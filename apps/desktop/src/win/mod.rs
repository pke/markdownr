//! Windows shell: plain Win32 window, Direct2D/DirectWrite rendering.

mod dwrite;

use crate::ast::Node;
use crate::view::{Effect, Key, Mods, Viewer};
use dwrite::{DwPara, Gfx};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::Duration;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCWSTR, w};

struct App {
    hwnd: HWND,
    gfx: Gfx,
    viewer: Viewer<DwPara>,
    path: Option<PathBuf>,
    fullscreen: Option<(WINDOWPLACEMENT, i32)>,
    mouse: (f32, f32),
    snapshot: Option<PathBuf>,
    exit_after_snapshot: bool,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` on the app unless it is already borrowed (re-entrant window messages).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn parse(text: &str) -> Node {
    let mut doc = crate::ast::parse(text);
    crate::transforms::compose(&mut doc, &crate::transforms::default_pipeline());
    doc
}

fn read_doc(path: &Path) -> (String, Node) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| format!("# Cannot open file\n\n`{}`: {e}", path.display()));
    (path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), parse(&text))
}

fn system_dark() -> bool {
    let mut value = 1u32;
    let mut size = 4u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    r.is_ok() && value == 0
}

fn dpi_scale(hwnd: HWND) -> f32 {
    unsafe { GetDpiForWindow(hwnd) as f32 / 96.0 }
}

fn client_size(hwnd: HWND) -> (u32, u32) {
    let mut rc = RECT::default();
    unsafe { GetClientRect(hwnd, &mut rc).ok() };
    ((rc.right - rc.left) as u32, (rc.bottom - rc.top) as u32)
}

fn lparam_point(lp: LPARAM) -> (i32, i32) {
    ((lp.0 & 0xffff) as i16 as i32, ((lp.0 >> 16) & 0xffff) as i16 as i32)
}

fn mods() -> Mods {
    unsafe {
        Mods { ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0, shift: GetKeyState(VK_SHIFT.0 as i32) < 0 }
    }
}

impl App {
    fn invalidate(&self) {
        unsafe { InvalidateRect(Some(self.hwnd), None, false).ok().ok() };
    }

    fn set_title(&self) {
        unsafe { SetWindowTextW(self.hwnd, &HSTRING::from(format!("{} – Markdownr", self.viewer.title))).ok() };
    }

    fn apply_dark(&mut self) {
        let dark = system_dark();
        self.viewer.set_dark(dark);
        let v = BOOL::from(dark);
        unsafe {
            DwmSetWindowAttribute(self.hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &v as *const _ as _, 4).ok();
        }
    }

    fn update_size(&mut self) {
        let (w, h) = client_size(self.hwnd);
        let s = dpi_scale(self.hwnd);
        self.gfx.resize(w, h);
        self.viewer.resize(w as f32 / s, h as f32 / s);
    }

    fn paint(&mut self) {
        let (w, h) = client_size(self.hwnd);
        let dpi = dpi_scale(self.hwnd) * 96.0;
        match self.gfx.ensure_target(w, h, dpi) {
            Ok(true) => self.viewer.invalidate(),
            Ok(false) => {}
            Err(e) => {
                eprintln!("render target: {e}");
                return;
            }
        }
        let before = self.viewer.last_layout_time;
        self.viewer.ensure_layout(&mut self.gfx);
        if self.viewer.last_layout_time != before {
            eprintln!("layout: {} blocks in {:?}", self.viewer.layout.blocks.len(), self.viewer.last_layout_time);
        }
        let v = &self.viewer;
        if let Err(e) = self.gfx.draw(&v.layout, v.selection.as_ref(), &v.theme, v.scroll, v.view_h) {
            eprintln!("draw: {e}");
            self.viewer.invalidate();
            self.invalidate();
        }
    }

    fn load(&mut self, path: PathBuf) {
        let (title, doc) = read_doc(&path);
        self.viewer.set_document(title, doc);
        self.path = Some(path);
        self.set_title();
        self.invalidate();
    }

    fn open_url(&mut self, url: &str) {
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
            unsafe { ShellExecuteW(Some(self.hwnd), w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
            return;
        }
        // Relative Markdown link: open next to the current file.
        let file = url.split('#').next().unwrap_or("");
        if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            let target = dir.join(file);
            if target.is_file() {
                self.load(target);
            }
        }
    }
}

/// Effects that may re-enter the window procedure run outside the app borrow.
fn apply(hwnd: HWND, effect: Effect) {
    match effect {
        Effect::Copy(text) => copy_to_clipboard(hwnd, &text),
        Effect::ToggleFullscreen => toggle_fullscreen(hwnd, None),
        Effect::ExitFullscreen => toggle_fullscreen(hwnd, Some(false)),
        Effect::Snapshot => {
            let path = std::env::temp_dir().join("markdownr-snapshot.bmp");
            match snapshot(hwnd, &path) {
                Ok(()) => eprintln!("snapshot: {}", path.display()),
                Err(e) => eprintln!("snapshot failed: {e}"),
            }
        }
        Effect::OpenUrl(url) => {
            with_app(|a| a.open_url(&url));
        }
        Effect::Quit => unsafe { DestroyWindow(hwnd).ok().unwrap_or_default() },
    }
}

fn toggle_fullscreen(hwnd: HWND, want: Option<bool>) {
    let saved = with_app(|a| a.fullscreen.take()).flatten();
    unsafe {
        if let Some((placement, style)) = saved {
            if want == Some(true) {
                with_app(|a| a.fullscreen = Some((placement, style)));
                return;
            }
            SetWindowLongW(hwnd, GWL_STYLE, style);
            SetWindowPlacement(hwnd, &placement).ok();
            SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED).ok();
        } else {
            if want == Some(false) {
                return;
            }
            let mut placement = WINDOWPLACEMENT { length: size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            GetWindowPlacement(hwnd, &mut placement).ok();
            let style = GetWindowLongW(hwnd, GWL_STYLE);
            let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
            GetMonitorInfoW(monitor, &mut mi).ok().ok();
            with_app(|a| a.fullscreen = Some((placement, style)));
            // Borderless over the whole monitor, covering the taskbar.
            SetWindowLongW(hwnd, GWL_STYLE, style & !(WS_OVERLAPPEDWINDOW.0 as i32));
            let r = mi.rcMonitor;
            SetWindowPos(hwnd, Some(HWND_TOP), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED).ok();
        }
    }
}

fn copy_to_clipboard(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.replace('\n', "\r\n").encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }
        EmptyClipboard().ok();
        if let Ok(mem) = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2) {
            let p = GlobalLock(mem) as *mut u16;
            if !p.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                GlobalUnlock(mem).ok();
                SetClipboardData(13 /* CF_UNICODETEXT */, Some(HANDLE(mem.0))).ok();
            }
        }
        CloseClipboard().ok();
    }
}

/// Window-only screenshot (PrintWindow of our own window) written as a 32-bit BMP.
fn snapshot(hwnd: HWND, path: &Path) -> std::io::Result<()> {
    let (w, h) = client_size(hwnd);
    let (w, h) = (w as i32, h as i32);
    let mut pixels = vec![0u8; (w * h * 4) as usize];
    unsafe {
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        // PW_CLIENTONLY | PW_RENDERFULLCONTENT
        let _ = windows::Win32::Storage::Xps::PrintWindow(hwnd, mem, windows::Win32::Storage::Xps::PRINT_WINDOW_FLAGS(1 | 2));
        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        GetDIBits(mem, bmp, 0, h as u32, Some(pixels.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
        SelectObject(mem, old);
        DeleteObject(bmp.into()).ok().ok();
        DeleteDC(mem).ok().ok();
        ReleaseDC(None, screen);
    }
    let mut file = Vec::with_capacity(54 + pixels.len());
    let size = 54 + pixels.len() as u32;
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&size.to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes());
    file.extend_from_slice(&54u32.to_le_bytes());
    file.extend_from_slice(&40u32.to_le_bytes());
    file.extend_from_slice(&w.to_le_bytes());
    file.extend_from_slice(&(-h).to_le_bytes());
    file.extend_from_slice(&1u16.to_le_bytes());
    file.extend_from_slice(&32u16.to_le_bytes());
    file.extend_from_slice(&[0u8; 24]);
    file.extend_from_slice(&pixels);
    std::fs::write(path, file)
}

fn key_from_vk(vk: u16) -> Option<Key> {
    Some(match VIRTUAL_KEY(vk) {
        VK_NEXT => Key::PageDown,
        VK_PRIOR => Key::PageUp,
        VK_SPACE => Key::Space,
        VK_DOWN => Key::Down,
        VK_UP => Key::Up,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_F11 => Key::F11,
        VK_F12 => Key::F12,
        VK_ESCAPE => Key::Escape,
        VK_OEM_PLUS | VK_ADD => Key::Char('+'),
        VK_OEM_MINUS | VK_SUBTRACT => Key::Char('-'),
        VK_NUMPAD0 => Key::Char('0'),
        _ if (0x30..=0x39).contains(&vk) || (0x41..=0x5a).contains(&vk) => Key::Char(vk as u8 as char),
        _ => return None,
    })
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                let snap = with_app(|a| {
                    a.paint();
                    a.snapshot.take().map(|p| (p, a.exit_after_snapshot))
                })
                .flatten();
                let _ = EndPaint(hwnd, &ps);
                if let Some((path, exit)) = snap {
                    match snapshot(hwnd, &path) {
                        Ok(()) => eprintln!("snapshot: {}", path.display()),
                        Err(e) => eprintln!("snapshot failed: {e}"),
                    }
                    if exit {
                        DestroyWindow(hwnd).ok();
                    }
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => {
                with_app(|a| {
                    a.update_size();
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let r = &*(lp.0 as *const RECT);
                SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE).ok();
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                with_app(|a| {
                    a.apply_dark();
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp.0 >> 16) & 0xffff) as i16 as f32 / 120.0;
                with_app(|a| {
                    let step = a.viewer.font_size * 3.0;
                    a.viewer.wheel(-delta * step, mods());
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                let (x, y) = lparam_point(lp);
                let dbl = Duration::from_millis(GetDoubleClickTime() as u64);
                with_app(|a| {
                    let s = dpi_scale(hwnd);
                    a.viewer.press(x as f32 / s, y as f32 / s, mods(), dbl);
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                ReleaseCapture().ok();
                let (x, y) = lparam_point(lp);
                let effect = with_app(|a| {
                    let s = dpi_scale(hwnd);
                    let e = a.viewer.release(x as f32 / s, y as f32 / s);
                    a.invalidate();
                    e
                })
                .flatten();
                if let Some(e) = effect {
                    apply(hwnd, e);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let (x, y) = lparam_point(lp);
                with_app(|a| {
                    let s = dpi_scale(hwnd);
                    a.mouse = (x as f32 / s, y as f32 / s);
                    if a.viewer.mouse_move(a.mouse.0, a.mouse.1) {
                        a.invalidate();
                    }
                });
                LRESULT(0)
            }
            WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
                let link = with_app(|a| a.viewer.over_link(a.mouse.0, a.mouse.1)).unwrap_or(false);
                if let Ok(c) = LoadCursorW(None, if link { IDC_HAND } else { IDC_IBEAM }) {
                    SetCursor(Some(c));
                }
                LRESULT(1)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let Some(key) = key_from_vk(wp.0 as u16) else { return DefWindowProcW(hwnd, msg, wp, lp) };
                let effect = with_app(|a| {
                    let e = a.viewer.key(key, mods(), a.fullscreen.is_some());
                    a.invalidate();
                    e
                })
                .flatten();
                if let Some(e) = effect {
                    apply(hwnd, e);
                }
                LRESULT(0)
            }
            WM_DROPFILES => {
                let drop = HDROP(wp.0 as *mut _);
                let mut buf = [0u16; 1024];
                let n = DragQueryFileW(drop, 0, Some(&mut buf)) as usize;
                DragFinish(drop);
                let path = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
                with_app(|a| a.load(path));
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub fn run(path: Option<PathBuf>, fallback_title: &str, fallback_text: &str) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).expect("module handle");
        let class = w!("MarkdownrWindow");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            w!("Markdownr"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .expect("create window");
        // Initial size: 980 × 820 DIPs.
        let s = dpi_scale(hwnd);
        SetWindowPos(hwnd, None, 0, 0, (980.0 * s) as i32, (820.0 * s) as i32, SWP_NOMOVE | SWP_NOZORDER).ok();
        DragAcceptFiles(hwnd, true);

        let (title, doc) = match &path {
            Some(p) => read_doc(p),
            None => (fallback_title.to_string(), parse(fallback_text)),
        };
        let mut app = App {
            hwnd,
            gfx: Gfx::new(hwnd).expect("Direct2D/DirectWrite"),
            viewer: Viewer::new(title, doc),
            path,
            fullscreen: None,
            mouse: (0.0, 0.0),
            snapshot: std::env::var_os("MARKDOWNR_SNAPSHOT").map(PathBuf::from),
            exit_after_snapshot: std::env::var_os("MARKDOWNR_SNAPSHOT_EXIT").is_some(),
        };
        if let Some(px) = std::env::var("MARKDOWNR_SCROLL").ok().and_then(|s| s.parse::<f32>().ok()) {
            app.viewer.scroll = px;
        }
        app.apply_dark();
        app.set_title();
        app.update_size();
        APP.with(|c| *c.borrow_mut() = Some(app));
        if let Some(px) = std::env::var("MARKDOWNR_SCROLL").ok().and_then(|s| s.parse::<f32>().ok()) {
            // Layout must exist before scrolling can be clamped: lay out once now.
            with_app(|a| {
                let (w, h) = client_size(a.hwnd);
                let dpi = dpi_scale(a.hwnd) * 96.0;
                if a.gfx.ensure_target(w, h, dpi).is_ok() {
                    a.viewer.ensure_layout(&mut a.gfx);
                    a.viewer.scroll_by(px);
                }
            });
        }
        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        APP.with(|c| c.borrow_mut().take());
    }
}
