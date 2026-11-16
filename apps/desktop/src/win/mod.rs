//! Windows shell: plain Win32 window, Direct2D/DirectWrite rendering. The app logic is
//! `crate::app`; this file is the window, its messages and what only Windows has (the
//! title bar colours, cloaking until the first frame, touch gestures, the compositor
//! overlay for particles).

mod comp;
mod dwrite;
mod images;

use crate::app::{App, Cursor, Host, Timer};
use crate::settings::Settings;
use crate::view::{Effect, Key, Mods};
use dwrite::Gfx;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_CLOAK, DWMWA_TEXT_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Input::Touch::{CloseGestureInfoHandle, GESTURECONFIG, GESTUREINFO, GID_PAN, GID_ZOOM, GetGestureInfo, HGESTUREINFO, SetGestureConfig};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, HSTRING, PCWSTR, PWSTR, w};

const TIMER_CHROME: usize = 1;
const TIMER_WATCH: usize = 2;
/// Idle-time slices of a partial document layout.
const TIMER_LAYOUT: usize = 4;
const WM_MOUSELEAVE: u32 = 0x02A3;

/// What the shared app asks of the window.
struct WinHost {
    hwnd: HWND,
}

impl Host for WinHost {
    fn invalidate(&self) {
        unsafe { InvalidateRect(Some(self.hwnd), None, false).ok().ok() };
    }

    fn set_title(&self, title: &str) {
        unsafe { SetWindowTextW(self.hwnd, &HSTRING::from(title)).ok() };
    }

    fn set_timer(&mut self, timer: Timer, every: Option<Duration>) {
        let id = match timer {
            Timer::Chrome => TIMER_CHROME,
            Timer::Layout => TIMER_LAYOUT,
        };
        unsafe {
            match every {
                Some(d) => {
                    SetTimer(Some(self.hwnd), id, (d.as_millis() as u32).max(1), None);
                }
                None => KillTimer(Some(self.hwnd), id).ok().unwrap_or_default(),
            }
        }
    }

    fn open_dialog(&mut self) -> Option<PathBuf> {
        let mut buf = [0u16; 2048];
        let filter: Vec<u16> = "Markdown (*.md;*.markdown;*.txt)\0*.md;*.markdown;*.mdown;*.mkd;*.txt\0Alle Dateien\0*.*\0\0".encode_utf16().collect();
        let mut ofn = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: self.hwnd,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            lpstrFile: PWSTR(buf.as_mut_ptr()),
            nMaxFile: buf.len() as u32,
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
            return None;
        }
        let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
    }

    fn open_external(&mut self, target: &str) {
        unsafe { ShellExecuteW(Some(self.hwnd), w!("open"), &HSTRING::from(target), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
    }

    fn minimized(&self) -> bool {
        unsafe { IsIconic(self.hwnd) }.as_bool()
    }
}

/// The app plus the window state only Windows has.
struct Win {
    app: App<Gfx, WinHost>,
    hwnd: HWND,
    /// Placement and style to restore when leaving full screen.
    full: Option<(WINDOWPLACEMENT, i32)>,
    tracking: bool,
    pan: Option<(i32, i32, i32, i32)>, // last x, y; start x, y (pixels)
    zoom: Option<u64>,
    /// Particle overlay on the compositor (made after the first frame); failed: not again.
    overlay: Option<comp::Overlay>,
    overlay_failed: bool,
    painted: bool,
    /// Title bar colours set last.
    caption: Option<crate::theme::Caption>,
    /// Still hidden from the screen until the first frame (see `uncloak`).
    cloaked: bool,
}

/// The app, used only on the UI thread (window procedure and `run`; thread-pool work posts
/// messages instead). A plain static rather than `thread_local!`, whose destructor
/// registration brings std's TLS clean-up and its abort path; `run` drops the app itself.
struct AppCell(RefCell<Option<Win>>);
// SAFETY: only the UI thread touches it (see above).
unsafe impl Sync for AppCell {}
static APP: AppCell = AppCell(RefCell::new(None));

/// Run `f` on the app unless it is already borrowed (re-entrant window messages).
fn with_app<R>(f: impl FnOnce(&mut Win) -> R) -> Option<R> {
    APP.0.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f))
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *const core::ffi::c_void, manual_reset: i32, initial: i32, name: *const u16) -> isize;
    fn SetEvent(event: isize) -> i32;
    fn WaitForSingleObject(handle: isize, ms: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
}

/// The start document, read and parsed on the thread pool while the window and
/// Direct2D are set up (`join` waits for it).
struct BackgroundDoc {
    path: PathBuf,
    done: isize,
    /// Running on the pool: `result` belongs to the worker until `done` is set.
    pooled: bool,
    result: Option<(String, String, crate::ast::Node)>,
}

impl BackgroundDoc {
    fn start(path: &Path) -> *mut BackgroundDoc {
        // SAFETY: plain Win32 calls; `run_parse` only touches the job until it signals `done`.
        // The worker only reads `path` and writes `result` (through the raw pointer,
        // field by field); the main thread only writes `pooled` until it waits.
        unsafe {
            let job = Box::into_raw(Box::new(BackgroundDoc { path: path.to_path_buf(), done: CreateEventW(core::ptr::null(), 1, 0, core::ptr::null()), pooled: false, result: None }));
            let pooled = (*job).done != 0 && images::TrySubmitThreadpoolCallback(Self::run_parse, job as *mut _, core::ptr::null()) != 0;
            (*job).pooled = pooled;
            job
        }
    }

    unsafe extern "system" fn run_parse(_instance: *mut core::ffi::c_void, ctx: *mut core::ffi::c_void) {
        // SAFETY: ctx is the job from `start`; the main thread waits for `done` before using `result`.
        unsafe {
            let job = ctx as *mut BackgroundDoc;
            (*job).result = Some(crate::app::read_doc(&(*job).path));
            SetEvent((*job).done);
        }
    }

    fn join(job: *mut BackgroundDoc) -> (String, String, crate::ast::Node) {
        // SAFETY: `job` comes from `start` and is used once; after `done` is signalled
        // (or when it never ran on the pool) the worker no longer touches it.
        unsafe {
            let mut job = Box::from_raw(job);
            if job.pooled {
                WaitForSingleObject(job.done, u32::MAX);
            }
            if job.done != 0 {
                CloseHandle(job.done);
            }
            job.result.take().unwrap_or_else(|| crate::app::read_doc(&job.path))
        }
    }
}

/// DWMWA_CLOAK: the window stays out of the picture while it is still being drawn.
fn cloak(hwnd: HWND, on: bool) {
    let v = BOOL::from(on);
    unsafe { DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &v as *const _ as _, 4).ok() };
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
    unsafe { Mods { ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0, shift: GetKeyState(VK_SHIFT.0 as i32) < 0 } }
}

/// Mouse message synthesised from touch or pen input.
fn from_touch() -> bool {
    (unsafe { GetMessageExtraInfo() }.0 as u32 & 0xFFFF_FF00) == 0xFF51_5700
}

impl Win {
    /// The system's light or dark mode picks the themes' palettes; the title bar follows the
    /// theme with the next frame (`apply_caption`, set anew there), not the system.
    fn apply_dark(&mut self) {
        self.app.set_dark(system_dark());
        self.caption = None;
    }

    /// Title bar in the theme's colours after each layout, set again only when they change
    /// (`Theme::caption`; theme button, front matter, history, system mode, the toolbar fading
    /// in and out): background, title and dark caption buttons on a dark background. Windows 10
    /// knows only the last one.
    fn apply_caption(&mut self) {
        let v = &self.app.viewer;
        let c = v.theme.caption(if self.full.is_some() { 0.0 } else { v.chrome_alpha });
        if self.caption == Some(c) {
            return;
        }
        self.caption = Some(c);
        let bgr = |ink: crate::theme::Ink| u32::from_le_bytes([ink.0[0], ink.0[1], ink.0[2], 0]);
        let (background, text, dark) = (bgr(c.background), bgr(c.text), BOOL::from(c.dark));
        unsafe {
            let set = |attr, v: *const core::ffi::c_void| DwmSetWindowAttribute(self.hwnd, attr, v, 4).ok();
            set(DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as _);
            set(DWMWA_CAPTION_COLOR, &background as *const _ as _);
            set(DWMWA_BORDER_COLOR, &background as *const _ as _);
            set(DWMWA_TEXT_COLOR, &text as *const _ as _);
        }
    }

    /// Show the window once its first frame is drawn (it starts cloaked: shown, laid
    /// out and painted by DWM, but not on screen, so no empty white client area).
    fn uncloak(&mut self) {
        if self.cloaked {
            self.cloaked = false;
            cloak(self.hwnd, false);
        }
    }

    fn update_size(&mut self, live: bool) {
        let (w, h) = client_size(self.hwnd);
        let s = dpi_scale(self.hwnd);
        self.app.gfx.resize(w, h);
        self.app.viewer.pixel = s;
        self.app.resize(w as f32 / s, h as f32 / s, live);
    }

    fn paint(&mut self) {
        let (w, h) = client_size(self.hwnd);
        let dpi = dpi_scale(self.hwnd) * 96.0;
        match self.app.gfx.ensure_target(w, h, dpi) {
            Ok(true) => self.app.viewer.invalidate(),
            Ok(false) => {}
            Err(_) => {
                return;
            }
        }
        self.app.layout();
        self.apply_caption();
        self.particle_overlay();
        self.painted = true;
        self.app.draw();
    }

    /// From the second frame on, particles move on a compositor overlay (no frames of
    /// ours); a window whose overlay failed draws them into the frame.
    fn particle_overlay(&mut self) {
        let v = &self.app.viewer;
        let wanted = self.painted && !self.overlay_failed && v.particles.is_some() && !v.reduce_motion;
        if wanted && self.overlay.is_none() {
            match comp::Overlay::new(self.hwnd) {
                Ok(o) => self.overlay = Some(o),
                Err(_) => self.overlay_failed = true,
            }
        }
        if let (true, Some(o), Some(ps)) = (wanted, self.overlay.as_mut(), v.particles.as_ref()) {
            let ms = (Instant::now() - v.anim_start).as_secs_f32() * 1000.0;
            if o.show(&mut self.app.gfx, ps, v.particles_gen, &v.theme, v.view_h, dpi_scale(self.hwnd), ms).is_ok() {
                self.app.viewer.composited = true;
                return;
            }
            self.overlay_failed = true;
        }
        if let Some(o) = self.overlay.take() {
            o.clear();
        }
        self.app.viewer.composited = false;
    }
}

/// Effects that may re-enter the window procedure run outside the app borrow.
fn apply(hwnd: HWND, effect: Effect) {
    let rest = match effect {
        Effect::Copy(_) | Effect::ToggleFullscreen | Effect::ExitFullscreen | Effect::Quit => Some(effect),
        other => with_app(|w| w.app.effect(other)).flatten(),
    };
    match rest {
        Some(Effect::Copy(text)) => copy_to_clipboard(hwnd, &text),
        Some(Effect::ToggleFullscreen) => toggle_fullscreen(hwnd, None),
        Some(Effect::ExitFullscreen) => toggle_fullscreen(hwnd, Some(false)),
        Some(Effect::Quit) => unsafe { DestroyWindow(hwnd).ok().unwrap_or_default() },
        _ => {}
    }
    with_app(|w| {
        w.app.schedule();
        w.app.invalidate();
    });
}

fn toggle_fullscreen(hwnd: HWND, want: Option<bool>) {
    let saved = with_app(|w| w.full.take()).flatten();
    unsafe {
        if let Some((placement, style)) = saved {
            if want == Some(true) {
                with_app(|w| w.full = Some((placement, style)));
                return;
            }
            with_app(|w| w.app.set_fullscreen(false));
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
            with_app(|w| {
                w.full = Some((placement, style));
                w.app.set_fullscreen(true);
            });
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

fn key_from_vk(vk: u16) -> Option<Key> {
    Some(match VIRTUAL_KEY(vk) {
        VK_NEXT => Key::PageDown,
        VK_PRIOR => Key::PageUp,
        VK_SPACE => Key::Space,
        VK_DOWN => Key::Down,
        VK_UP => Key::Up,
        VK_LEFT => Key::Left,
        VK_RIGHT => Key::Right,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_F1 => Key::F1,
        VK_F3 => Key::F3,
        VK_F5 => Key::F5,
        VK_F11 => Key::F11,
        VK_ESCAPE => Key::Escape,
        VK_RETURN => Key::Enter,
        VK_BACK => Key::Backspace,
        VK_OEM_PLUS | VK_ADD => Key::Char('+'),
        VK_OEM_MINUS | VK_SUBTRACT => Key::Char('-'),
        VK_NUMPAD0 => Key::Char('0'),
        _ if (0x30..=0x39).contains(&vk) || (0x41..=0x5a).contains(&vk) => Key::Char(vk as u8 as char),
        _ => return None,
    })
}

fn gesture(hwnd: HWND, lp: LPARAM) -> bool {
    let handle = HGESTUREINFO(lp.0 as *mut _);
    let mut gi = GESTUREINFO { cbSize: size_of::<GESTUREINFO>() as u32, ..Default::default() };
    if unsafe { GetGestureInfo(handle, &mut gi) }.is_err() {
        return false;
    }
    const GF_BEGIN: u32 = 1;
    const GF_END: u32 = 4;
    let (x, y) = (gi.ptsLocation.x as i32, gi.ptsLocation.y as i32);
    let handled = with_app(|w| {
        let s = dpi_scale(hwnd);
        if gi.dwID == GID_PAN.0 {
            if gi.dwFlags & GF_BEGIN != 0 || w.pan.is_none() {
                w.pan = Some((x, y, x, y));
            }
            let (_, ly, sx, sy) = w.pan.unwrap();
            w.app.viewer.scroll_by(-(y - ly) as f32 / s);
            w.pan = Some((x, y, sx, sy));
            if gi.dwFlags & GF_END != 0 {
                w.app.viewer.swipe((x - sx) as f32 / s, (y - sy) as f32 / s);
                w.pan = None;
            }
            w.app.invalidate();
            true
        } else if gi.dwID == GID_ZOOM.0 {
            if gi.dwFlags & GF_BEGIN != 0 {
                w.zoom = Some(gi.ullArguments);
            } else if let Some(start) = w.zoom {
                let ratio = gi.ullArguments as f32 / start.max(1) as f32;
                if !(0.85..=1.15).contains(&ratio) {
                    w.app.viewer.zoom_by(ratio);
                    w.zoom = Some(gi.ullArguments);
                    w.app.invalidate();
                }
            }
            if gi.dwFlags & GF_END != 0 {
                w.zoom = None;
            }
            true
        } else {
            false
        }
    })
    .unwrap_or(false);
    if handled {
        unsafe { CloseGestureInfoHandle(handle).ok() };
    }
    handled
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                with_app(|w| {
                    w.paint();
                    w.uncloak();
                });
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => {
                with_app(|w| w.update_size(true));
                LRESULT(0)
            }
            WM_EXITSIZEMOVE => {
                with_app(|w| w.app.resize_done());
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let r = &*(lp.0 as *const RECT);
                SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE).ok();
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                with_app(|w| w.apply_dark());
                LRESULT(0)
            }
            WM_TIMER => {
                with_app(|w| match wp.0 {
                    TIMER_CHROME => w.app.schedule(),
                    TIMER_WATCH => {
                        w.uncloak(); // in case no frame could be drawn
                        w.app.watch()
                    }
                    TIMER_LAYOUT => w.app.continue_layout(),
                    _ => {}
                });
                LRESULT(0)
            }
            images::WM_IMAGE_LOADED => {
                // SAFETY: lParam is the Job box posted by the image loader.
                let ready = with_app(|w| w.app.gfx.images.collect(lp.0));
                with_app(|w| {
                    if ready == Some(true) {
                        w.app.viewer.invalidate();
                        w.app.invalidate();
                    }
                });
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                // Whole notches (a wheel) glide; a precision touchpad's small steps apply at once.
                let raw = ((wp.0 >> 16) & 0xffff) as i16;
                let delta = raw as f32 / 120.0;
                let mut pt = POINT { x: lparam_point(lp).0, y: lparam_point(lp).1 };
                ScreenToClient(hwnd, &mut pt).ok().ok();
                with_app(|w| {
                    let s = dpi_scale(hwnd);
                    w.app.wheel(-delta, raw % 120 == 0, mods(), pt.x as f32 / s, pt.y as f32 / s);
                });
                LRESULT(0)
            }
            WM_GESTURE => {
                if gesture(hwnd, lp) {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wp, lp)
                }
            }
            WM_LBUTTONDOWN => {
                SetCapture(hwnd);
                let (x, y) = lparam_point(lp);
                let dbl = Duration::from_millis(GetDoubleClickTime() as u64);
                with_app(|w| {
                    let s = dpi_scale(hwnd);
                    w.app.press(x as f32 / s, y as f32 / s, mods(), dbl);
                });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                ReleaseCapture().ok();
                let (x, y) = lparam_point(lp);
                let tap = from_touch();
                let effect = with_app(|w| {
                    let s = dpi_scale(hwnd);
                    w.app.release(x as f32 / s, y as f32 / s, tap)
                })
                .flatten();
                if let Some(e) = effect {
                    apply(hwnd, e);
                }
                LRESULT(0)
            }
            // The mouse's back and forward buttons (XBUTTON1, XBUTTON2) walk the history.
            WM_XBUTTONUP => {
                let back = (wp.0 >> 16) & 0xffff == 1;
                with_app(|w| if back { w.app.back() } else { w.app.forward() });
                LRESULT(1)
            }
            WM_MOUSEMOVE => {
                let (x, y) = lparam_point(lp);
                let touch = from_touch();
                with_app(|w| {
                    if !w.tracking {
                        let mut tme = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                        w.tracking = TrackMouseEvent(&mut tme).is_ok();
                    }
                    let s = dpi_scale(hwnd);
                    w.app.mouse_move(x as f32 / s, y as f32 / s, touch);
                });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with_app(|w| {
                    w.tracking = false;
                    w.app.mouse_leave();
                });
                LRESULT(0)
            }
            WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
                let cursor = match with_app(|w| w.app.cursor()) {
                    Some(Cursor::Hand) => IDC_HAND,
                    Some(Cursor::Text) => IDC_IBEAM,
                    _ => IDC_ARROW,
                };
                if let Ok(c) = LoadCursorW(None, cursor) {
                    SetCursor(Some(c));
                }
                LRESULT(1)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let Some(key) = key_from_vk(wp.0 as u16) else { return DefWindowProcW(hwnd, msg, wp, lp) };
                let effect = with_app(|w| w.app.key(key, mods())).flatten();
                if let Some(e) = effect {
                    apply(hwnd, e);
                }
                LRESULT(0)
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wp.0 as u32) {
                    with_app(|w| w.app.text(c));
                }
                LRESULT(0)
            }
            WM_DROPFILES => {
                let drop = HDROP(wp.0 as *mut _);
                let mut buf = [0u16; 1024];
                let n = DragQueryFileW(drop, 0, Some(&mut buf)) as usize;
                DragFinish(drop);
                let path = PathBuf::from(String::from_utf16_lossy(&buf[..n]));
                with_app(|w| w.app.load(path));
                LRESULT(0)
            }
            WM_DESTROY => {
                with_app(|w| w.app.save_settings());
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub fn run(path: Option<PathBuf>, sample: &'static str) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        crate::app::init_locale(ui_language, || {
            let mut buf = [0u16; 85];
            let n = windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buf);
            (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
        });
        crate::view::set_key_namer(key_name);
        // Parse the document while the window and Direct2D come up.
        let background = path.as_deref().map(BackgroundDoc::start);
        let instance = GetModuleHandleW(None).expect("module handle");
        let class = w!("MarkdownrWindow");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            // Icon resource 1 (data/app.res); the small one comes from the same resource.
            hIcon: LoadIconW(Some(instance.into()), PCWSTR(1 as _)).unwrap_or_default(),
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
        // Initial size: 980 × 820 DIPs. NOACTIVATE: whether the window takes the foreground is up to
        // ShowWindow below (the starter's wish), not to this resize.
        let s = dpi_scale(hwnd);
        SetWindowPos(hwnd, None, 0, 0, (980.0 * s) as i32, (820.0 * s) as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE).ok();
        DragAcceptFiles(hwnd, true);
        // Touch: one-finger pan in both directions (with inertia) and pinch zoom.
        let configs = [
            GESTURECONFIG { dwID: GID_PAN, dwWant: 0x2 | 0x4 | 0x10, dwBlock: 0x8 },
            GESTURECONFIG { dwID: GID_ZOOM, dwWant: 1, dwBlock: 0 },
        ];
        SetGestureConfig(hwnd, 0, &configs, size_of::<GESTURECONFIG>() as u32).ok();

        let settings_path = crate::app::settings_path();
        let settings = Settings::load(&settings_path);
        let mut gfx = Gfx::new(hwnd).expect("Direct2D/DirectWrite");
        {
            // Create the render target now, in parallel with the parse.
            let (w, h) = client_size(hwnd);
            let _ = gfx.ensure_target(w, h, dpi_scale(hwnd) * 96.0);
        }
        let doc = match background {
            Some(job) => BackgroundDoc::join(job),
            None => crate::app::start_doc(&settings, sample),
        };
        let mut app = App::new(WinHost { hwnd }, gfx, path, doc, settings, settings_path, sample);
        let start_fullscreen = app.start_fullscreen();
        {
            let st = windows::Win32::System::SystemInformation::GetLocalTime();
            app.viewer.today = (st.wYear as i32, st.wMonth as u32, st.wDay as u32);
            let mut anim = BOOL(1);
            let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut anim as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
            app.viewer.reduce_motion = !anim.as_bool();
        }
        let mut win = Win { app, hwnd, full: None, tracking: false, pan: None, zoom: None, overlay: None, overlay_failed: false, painted: false, caption: None, cloaked: true };
        win.apply_dark();
        win.app.set_title();
        win.update_size(false);
        *APP.0.borrow_mut() = Some(win);
        with_app(|w| w.app.schedule());
        SetTimer(Some(hwnd), TIMER_WATCH, 1000, None);
        // Cloaked until the first frame is drawn (Win::uncloak): no white client area.
        cloak(hwnd, true);
        // SW_SHOWDEFAULT: the show state the starter asked for (STARTUPINFO), e.g. a shortcut set to
        // minimized, or SW_SHOWNOACTIVATE from tools/mdr-shot.ps1 so the window never steals focus.
        let _ = ShowWindow(hwnd, SW_SHOWDEFAULT);
        if start_fullscreen {
            toggle_fullscreen(hwnd, Some(true));
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        drop(APP.0.borrow_mut().take());
    }
}

/// Windows' display language as a two-letter code, as FileCushion picks it: traditional
/// Chinese (zh-TW, zh-HK, zh-MO, zh-Hant) gets English, simplified is "zh".
fn ui_language() -> String {
    let mut buf = [0u16; 85];
    let n = unsafe { windows::Win32::Globalization::LCIDToLocaleName(windows::Win32::Globalization::GetUserDefaultUILanguage() as u32, Some(&mut buf), 0) };
    let name = String::from_utf16_lossy(&buf[..(n.max(1) - 1) as usize]);
    if ["zh-TW", "zh-HK", "zh-MO", "zh-Hant"].iter().any(|t| name.starts_with(t)) {
        return "en".into();
    }
    name.get(..2).unwrap_or("en").to_ascii_lowercase()
}

/// A key's name in the keyboard layout's language (GetKeyNameTextW: STRG, BILD-NACH-UNTEN on
/// a German keyboard, Ctrl, Page Down on an English one); all-capital names are written with
/// a capital first letter only (Strg, Bild-nach-unten).
fn key_name(vk: u16, extended: bool) -> Option<String> {
    let mut buf = [0u16; 64];
    let n = unsafe {
        let scan = MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC);
        GetKeyNameTextW(((scan << 16) | if extended { 1 << 24 } else { 0 }) as i32, &mut buf)
    };
    let name = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    if name.is_empty() {
        return None;
    }
    if name.chars().any(char::is_lowercase) {
        return Some(name);
    }
    let mut chars = name.chars();
    let first = chars.next()?;
    Some(std::iter::once(first).chain(chars.flat_map(char::to_lowercase)).collect())
}
