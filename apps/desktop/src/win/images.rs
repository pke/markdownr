//! Image loading: WIC decodes local files and downloads (urlmon cache) on the
//! Windows thread pool (WIC itself is synchronous); each result travels back
//! boxed in the lParam of WM_IMAGE_LOADED, so no Rust threads or channels are
//! needed. The UI thread turns the pixels into Direct2D bitmaps on first draw.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

/// Work item handed to the thread pool and back to the UI thread.
struct Job {
    url: String,
    source: Option<Source>,
    hwnd: isize,
    decoded: Decoded,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn TrySubmitThreadpoolCallback(cb: unsafe extern "system" fn(*mut core::ffi::c_void, *mut core::ffi::c_void), ctx: *mut core::ffi::c_void, env: *const core::ffi::c_void) -> i32;
}

unsafe extern "system" fn run_job(_instance: *mut core::ffi::c_void, ctx: *mut core::ffi::c_void) {
    // SAFETY: ctx is the Box<Job> leaked in Images::size.
    let mut job = unsafe { Box::from_raw(ctx as *mut Job) };
    job.decoded = job.source.take().and_then(|s| unsafe { load(&s) });
    let hwnd = HWND(job.hwnd as *mut _);
    let ptr = Box::into_raw(job);
    if unsafe { PostMessageW(Some(hwnd), WM_IMAGE_LOADED, WPARAM(0), LPARAM(ptr as isize)) }.is_err() {
        drop(unsafe { Box::from_raw(ptr) });
    }
}

enum State {
    Loading,
    Failed,
    Ready { w: u32, h: u32, pixels: Vec<u8>, bitmap: Option<ID2D1Bitmap> },
}

pub struct Images {
    map: RefCell<HashMap<String, State>>,
    base: RefCell<Option<PathBuf>>,
    hwnd: isize,
}

impl Images {
    pub fn new(hwnd: HWND) -> Self {
        Images { map: RefCell::default(), base: RefCell::default(), hwnd: hwnd.0 as isize }
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
        let job = Box::into_raw(Box::new(Job { url: url.to_string(), source, hwnd: self.hwnd, decoded: None }));
        // SAFETY: run_job takes ownership of the box; reclaim it if submission fails.
        if unsafe { TrySubmitThreadpoolCallback(run_job, job as *mut _, core::ptr::null()) } == 0 {
            drop(unsafe { Box::from_raw(job) });
            self.map.borrow_mut().insert(url.to_string(), State::Failed);
        }
        None
    }

    /// Takes a finished load from WM_IMAGE_LOADED's lParam; true if an image became available.
    ///
    /// # Safety
    /// `lparam` must be the pointer posted by `run_job`, used exactly once.
    pub unsafe fn collect(&self, lparam: isize) -> bool {
        if lparam == 0 {
            return false;
        }
        let job = unsafe { Box::from_raw(lparam as *mut Job) };
        let mut map = self.map.borrow_mut();
        // A newer document may have cleared the map; drop stale results.
        if !map.contains_key(&job.url) {
            return false;
        }
        let (state, ok) = match job.decoded {
            Some((w, h, pixels)) => (State::Ready { w, h, pixels, bitmap: None }, true),
            None => (State::Failed, false),
        };
        map.insert(job.url, state);
        ok
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
