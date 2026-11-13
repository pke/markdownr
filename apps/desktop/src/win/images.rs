//! Image loading: WIC decodes local files and downloads (urlmon cache) on worker
//! threads; the UI thread turns the pixels into Direct2D bitmaps on first draw.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use windows::Win32::Foundation::{GENERIC_READ, HWND, LPARAM, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::{D2D1_BITMAP_PROPERTIES, ID2D1Bitmap, ID2D1RenderTarget};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_APP};
use windows::core::{HSTRING, Interface, PCWSTR};

/// Posted to the window when an image finished loading.
pub const WM_IMAGE_LOADED: u32 = WM_APP + 1;

type Decoded = Option<(u32, u32, Vec<u8>)>;

enum State {
    Loading,
    Failed,
    Ready { w: u32, h: u32, pixels: Vec<u8>, bitmap: Option<ID2D1Bitmap> },
}

pub struct Images {
    map: RefCell<HashMap<String, State>>,
    base: RefCell<Option<PathBuf>>,
    hwnd: isize,
    tx: Sender<(String, Decoded)>,
    rx: Receiver<(String, Decoded)>,
}

impl Images {
    pub fn new(hwnd: HWND) -> Self {
        let (tx, rx) = channel();
        Images { map: RefCell::default(), base: RefCell::default(), hwnd: hwnd.0 as isize, tx, rx }
    }

    /// Directory that relative image paths are resolved against.
    pub fn set_base(&self, dir: Option<PathBuf>) {
        *self.base.borrow_mut() = dir;
        self.map.borrow_mut().clear();
    }

    /// Size in pixels if decoded; otherwise starts loading (once) and returns None.
    pub fn size(&self, url: &str) -> Option<(f32, f32)> {
        if let Some(state) = self.map.borrow().get(url) {
            return match state {
                State::Ready { w, h, .. } => Some((*w as f32, *h as f32)),
                _ => None,
            };
        }
        self.map.borrow_mut().insert(url.to_string(), State::Loading);
        let source = resolve(url, self.base.borrow().as_deref());
        let (tx, hwnd, url) = (self.tx.clone(), self.hwnd, url.to_string());
        std::thread::spawn(move || {
            let decoded = source.and_then(|s| unsafe { load(&s) });
            let _ = tx.send((url, decoded));
            unsafe { PostMessageW(Some(HWND(hwnd as *mut _)), WM_IMAGE_LOADED, WPARAM(0), LPARAM(0)).ok() };
        });
        None
    }

    /// Take finished loads; returns true if any image became available.
    pub fn collect(&self) -> bool {
        let mut any = false;
        let mut map = self.map.borrow_mut();
        while let Ok((url, decoded)) = self.rx.try_recv() {
            let state = match decoded {
                Some((w, h, pixels)) => {
                    any = true;
                    State::Ready { w, h, pixels, bitmap: None }
                }
                None => State::Failed,
            };
            map.insert(url, state);
        }
        any
    }

    /// Drop device-dependent bitmaps (render target was recreated).
    pub fn reset_device(&self) {
        for s in self.map.borrow_mut().values_mut() {
            if let State::Ready { bitmap, .. } = s {
                *bitmap = None;
            }
        }
    }

    pub fn bitmap(&self, url: &str, rt: &ID2D1RenderTarget) -> Option<ID2D1Bitmap> {
        let mut map = self.map.borrow_mut();
        let State::Ready { w, h, pixels, bitmap } = map.get_mut(url)? else { return None };
        if bitmap.is_none() {
            let props = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
                dpiX: 96.0,
                dpiY: 96.0,
            };
            *bitmap = unsafe { rt.CreateBitmap(D2D_SIZE_U { width: *w, height: *h }, Some(pixels.as_ptr() as *const _), *w * 4, &props) }.ok();
        }
        bitmap.clone()
    }
}

enum Source {
    File(PathBuf),
    Url(String),
}

fn resolve(url: &str, base: Option<&Path>) -> Option<Source> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Some(Source::Url(url.to_string()));
    }
    let path = url.strip_prefix("file:///").or_else(|| url.strip_prefix("file://")).unwrap_or(url);
    let path = percent_decode(path.split(['?', '#']).next().unwrap_or(path));
    let p = Path::new(&path);
    Some(Source::File(if p.is_absolute() { p.to_path_buf() } else { base?.join(p) }))
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() && s.is_char_boundary(i + 3) {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

unsafe fn load(source: &Source) -> Decoded {
    unsafe {
        let path = match source {
            Source::File(p) => p.clone(),
            Source::Url(u) => {
                let mut buf = [0u16; 1024];
                windows::Win32::System::Com::Urlmon::URLDownloadToCacheFileW(None, &HSTRING::from(u.as_str()), &mut buf, 0, None).ok()?;
                let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
                PathBuf::from(String::from_utf16_lossy(&buf[..n]))
            }
        };
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let result = decode(&path);
        CoUninitialize();
        result
    }
}

unsafe fn decode(path: &Path) -> Decoded {
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).ok()?;
        let name = HSTRING::from(path.as_os_str());
        let decoder = factory.CreateDecoderFromFilename(PCWSTR(name.as_ptr()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand).ok()?;
        let frame = decoder.GetFrame(0).ok()?;
        let source: IWICBitmapSource = WICConvertBitmapSource(&GUID_WICPixelFormat32bppPBGRA, &frame.cast::<IWICBitmapSource>().ok()?).ok()?;
        let (mut w, mut h) = (0u32, 0u32);
        source.GetSize(&mut w, &mut h).ok()?;
        if w == 0 || h == 0 || (w as u64 * h as u64) > 64 * 1024 * 1024 {
            return None;
        }
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        source.CopyPixels(std::ptr::null(), w * 4, &mut pixels).ok()?;
        Some((w, h, pixels))
    }
}
