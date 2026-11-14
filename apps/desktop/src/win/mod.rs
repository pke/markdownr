//! Windows shell: plain Win32 window, Direct2D/DirectWrite rendering.

mod dwrite;
mod images;

use crate::ast::Node;
use crate::settings::Settings;
use crate::theme::ThemeChoice;
use crate::view::{Effect, Key, Mods, Viewer};
use dwrite::{DwPara, Gfx};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
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

struct App {
    hwnd: HWND,
    gfx: Gfx,
    viewer: Viewer<DwPara>,
    path: Option<PathBuf>,
    modified: Option<SystemTime>,
    fullscreen: Option<(WINDOWPLACEMENT, i32)>,
    mouse: (f32, f32),
    tracking: bool,
    pan: Option<(i32, i32, i32, i32)>, // last x, y; start x, y (pixels)
    zoom: Option<u64>,
    chrome_timer: bool,
    /// While the window is being resized (until 150 ms after the last WM_SIZE or the
    /// end of the drag) the idle layout waits: each size step would discard it.
    sizing_until: Option<Instant>,
    snapshot: Option<PathBuf>,
    exit_after_snapshot: bool,
    snapshot_ready: bool,
    /// Title bar colour last given to DWM (COLORREF; u32::MAX: none yet).
    caption: u32,
    settings: Settings,
    settings_path: PathBuf,
    sample: &'static str,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` on the app unless it is already borrowed (re-entrant window messages).
fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| cell.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

fn parse(text: &str) -> Node {
    let mut doc = crate::ast::parse(&crate::transforms::preprocess_html(text));
    crate::transforms::compose(&mut doc, &crate::transforms::default_pipeline());
    doc
}

fn read_doc(path: &Path) -> (String, String, Node) {
    let text = match std::fs::read(path) {
        Ok(bytes) => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
            String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect())
        }
        Err(_) => ["# Datei kann nicht geöffnet werden\n\n`", &path.to_string_lossy(), "`"].concat(),
    };
    let doc = parse(&text);
    (path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), text, doc)
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
    result: Option<(String, String, Node)>,
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
            (*job).result = Some(read_doc(&(*job).path));
            SetEvent((*job).done);
        }
    }

    fn join(job: *mut BackgroundDoc) -> (String, String, Node) {
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
            job.result.take().unwrap_or_else(|| read_doc(&job.path))
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
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

fn settings_path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("Markdownr").join("settings.ini")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
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

impl App {
    fn invalidate(&self) {
        unsafe { InvalidateRect(Some(self.hwnd), None, false).ok().ok() };
    }

    /// Complete a partial layout before input that may need all of it (selection
    /// end, links and anchors, find, TOC, copy). Scrolling extends it as needed.
    fn settle(&mut self) {
        self.viewer.finish_layout(&mut self.gfx);
    }

    /// Arm the idle layout: at once, or once a resize has come to rest.
    fn arm_layout(&mut self) {
        let wait = self.sizing_until.map_or(0, |t| t.saturating_duration_since(Instant::now()).as_millis() as u32);
        if wait == 0 {
            self.sizing_until = None;
        }
        unsafe { SetTimer(Some(self.hwnd), TIMER_LAYOUT, wait.max(1), None) };
    }

    /// One idle slice of a partial layout; repaint once it is complete.
    fn continue_layout(&mut self) {
        if self.sizing_until.is_some_and(|t| t > Instant::now()) {
            return self.arm_layout();
        }
        if self.viewer.continue_layout(&mut self.gfx, Duration::from_millis(30)) || !self.viewer.layout_pending() {
            unsafe { KillTimer(Some(self.hwnd), TIMER_LAYOUT).ok() };
            self.invalidate();
        }
    }

    fn set_title(&self) {
        // "First heading | Markdownr", or just "Markdownr" when the document has no heading.
        let title = match self.viewer.first_heading() {
            Some(h) => [h.as_str(), " | Markdownr"].concat(),
            None => "Markdownr".to_string(),
        };
        unsafe { SetWindowTextW(self.hwnd, &HSTRING::from(title)).ok() };
    }

    fn apply_dark(&mut self) {
        let dark = system_dark();
        self.viewer.set_dark(dark);
    }

    /// Title bar in the page's background colour (as FileCushion): DWM caption colour, and its
    /// dark mode (white caption text and buttons) when that colour is dark.
    fn apply_caption(&mut self) {
        let [r, g, b, _] = self.viewer.theme.background.0;
        let color = u32::from_le_bytes([r, g, b, 0]);
        if color == self.caption {
            return;
        }
        self.caption = color;
        let dark = BOOL::from(299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b) < 128_000);
        unsafe {
            DwmSetWindowAttribute(self.hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const _ as _, 4).ok();
            DwmSetWindowAttribute(self.hwnd, DWMWA_CAPTION_COLOR, &color as *const _ as _, 4).ok();
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
            Err(_) => {
                return;
            }
        }
        self.viewer.ensure_layout(&mut self.gfx);
        if self.snapshot_ready && self.snapshot.is_some() {
            self.viewer.finish_layout(&mut self.gfx); // snapshots show the final layout
        }
        if self.viewer.layout_pending() {
            self.arm_layout();
        }
        self.apply_caption();
        let v = &self.viewer;
        let finds = v.find_highlights();
        let chrome = v.chrome_visible().then_some((&v.chrome, v.chrome_alpha));
        let sprites = v.sprites(Instant::now());
        self.gfx.now_ms = (Instant::now() - v.anim_start).as_secs_f32() * 1000.0;
        let particles = v.particles.as_ref().map(|p| (p, sprites.as_slice()));
        if self.gfx.draw(&v.layout, v.selection.as_ref(), &finds, &v.theme, v.scroll, v.view_h, chrome, particles, &v.tip_layout).is_err() {
            self.viewer.invalidate();
            self.invalidate();
        }
    }

    /// Drive the chrome fade: (re)arm or stop the animation timer.
    fn schedule(&mut self) {
        let (repaint, next) = self.viewer.tick(Instant::now());
        let next = if unsafe { IsIconic(self.hwnd) }.as_bool() { None } else { next };
        if repaint {
            self.invalidate();
        }
        unsafe {
            match next {
                Some(d) => {
                    SetTimer(Some(self.hwnd), TIMER_CHROME, d.as_millis().clamp(10, 60_000) as u32, None);
                    self.chrome_timer = true;
                }
                None if self.chrome_timer => {
                    KillTimer(Some(self.hwnd), TIMER_CHROME).ok();
                    self.chrome_timer = false;
                }
                None => {}
            }
        }
    }

    fn load(&mut self, path: PathBuf) {
        let (title, source, doc) = read_doc(&path);
        let same = self.path.as_ref() == Some(&path);
        self.gfx.images.set_base(path.parent().map(Path::to_path_buf));
        self.viewer.set_document(title, source, doc, same);
        self.modified = mtime(&path);
        self.settings.add_recent(&path);
        self.settings.save(&self.settings_path);
        self.path = Some(path);
        self.set_title();
        self.invalidate();
    }

    fn load_text(&mut self, title: &str, text: &str) {
        self.path = None;
        self.modified = None;
        self.gfx.images.set_base(std::env::current_dir().ok());
        self.viewer.set_document(title.to_string(), text.to_string(), parse(text), false);
        self.set_title();
        self.invalidate();
    }

    fn reload(&mut self) {
        match self.path.clone() {
            Some(p) => {
                let (title, source, doc) = read_doc(&p);
                self.viewer.set_document(title, source, doc, true);
                self.modified = mtime(&p);
                self.set_title();
            }
            None => {
                let start = self.settings.start_page();
                self.viewer.set_document("Markdownr".into(), start.clone(), parse(&start), true);
            }
        }
        self.invalidate();
    }

    /// Reload when the file changed on disk (polled once a second).
    fn watch(&mut self) {
        if let Some(p) = &self.path {
            let now = mtime(p);
            if now.is_some() && now != self.modified {
                self.reload();
            }
        }
    }

    fn open_dialog(&mut self) {
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
        if unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
            let n = buf.iter().position(|&c| c == 0).unwrap_or(0);
            self.load(PathBuf::from(String::from_utf16_lossy(&buf[..n])));
        }
    }

    fn open_url(&mut self, url: &str) {
        let lower = url.to_ascii_lowercase();
        match lower.as_str() {
            "markdownr:open" => return self.open_dialog(),
            "markdownr:sample" => {
                let sample = self.sample;
                return self.load_text("Beispiel", sample);
            }
            _ => {}
        }
        if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:") {
            unsafe { ShellExecuteW(Some(self.hwnd), w!("open"), &HSTRING::from(url), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
            return;
        }
        let file = percent_decode(url.split('#').next().unwrap_or(""));
        let file = file.strip_prefix("file:///").or_else(|| file.strip_prefix("file://")).unwrap_or(&file).to_string();
        let p = Path::new(&file);
        let target = if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(dir) = self.path.as_ref().and_then(|p| p.parent()) {
            dir.join(p)
        } else {
            return;
        };
        if !target.is_file() {
            return;
        }
        let ext = target.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        if matches!(ext.as_str(), "md" | "markdown" | "mdown" | "mkd" | "txt") {
            self.load(target);
        } else {
            unsafe { ShellExecuteW(Some(self.hwnd), w!("open"), &HSTRING::from(target.as_os_str()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        }
    }

    fn save_settings(&mut self) {
        self.settings.font_size = self.viewer.font_size;
        self.settings.theme = self.viewer.theme_choice.name().to_string();
        self.settings.remember_fullscreen = self.viewer.remember_fullscreen;
        self.settings.fullscreen = self.viewer.remember_fullscreen && self.fullscreen.is_some();
        self.settings.save(&self.settings_path);
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
                Ok(()) => crate::util::log(&["snapshot: ", &path.to_string_lossy()]),
                Err(_) => crate::util::log(&["snapshot failed"]),
            }
        }
        Effect::OpenUrl(url) => {
            with_app(|a| a.open_url(&url));
        }
        Effect::OpenDialog => {
            with_app(|a| a.open_dialog());
        }
        Effect::Reload => {
            with_app(|a| a.reload());
        }
        Effect::Quit => unsafe { DestroyWindow(hwnd).ok().unwrap_or_default() },
    }
    with_app(|a| {
        a.schedule();
        a.invalidate();
    });
}

fn toggle_fullscreen(hwnd: HWND, want: Option<bool>) {
    let saved = with_app(|a| a.fullscreen.take()).flatten();
    unsafe {
        if let Some((placement, style)) = saved {
            if want == Some(true) {
                with_app(|a| a.fullscreen = Some((placement, style)));
                return;
            }
            with_app(|a| a.viewer.set_fullscreen(false));
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
            with_app(|a| {
                a.fullscreen = Some((placement, style));
                a.viewer.set_fullscreen(true);
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
        VK_LEFT => Key::Left,
        VK_RIGHT => Key::Right,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_F1 => Key::F1,
        VK_F3 => Key::F3,
        VK_F5 => Key::F5,
        VK_F11 => Key::F11,
        VK_F12 => Key::F12,
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
    let handled = with_app(|a| {
        let s = dpi_scale(hwnd);
        if gi.dwID == GID_PAN.0 {
            if gi.dwFlags & GF_BEGIN != 0 || a.pan.is_none() {
                a.pan = Some((x, y, x, y));
            }
            let (_, ly, sx, sy) = a.pan.unwrap();
            a.viewer.scroll_by(-(y - ly) as f32 / s);
            a.pan = Some((x, y, sx, sy));
            if gi.dwFlags & GF_END != 0 {
                a.viewer.swipe((x - sx) as f32 / s, (y - sy) as f32 / s);
                a.pan = None;
            }
            a.invalidate();
            true
        } else if gi.dwID == GID_ZOOM.0 {
            if gi.dwFlags & GF_BEGIN != 0 {
                a.zoom = Some(gi.ullArguments);
            } else if let Some(start) = a.zoom {
                let ratio = gi.ullArguments as f32 / start.max(1) as f32;
                if !(0.85..=1.15).contains(&ratio) {
                    a.viewer.zoom_by(ratio);
                    a.zoom = Some(gi.ullArguments);
                    a.invalidate();
                }
            }
            if gi.dwFlags & GF_END != 0 {
                a.zoom = None;
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
                let snap = with_app(|a| {
                    a.paint();
                    if a.snapshot_ready { a.snapshot.take().map(|p| (p, a.exit_after_snapshot)) } else { None }
                })
                .flatten();
                let _ = EndPaint(hwnd, &ps);
                if let Some((path, exit)) = snap {
                    let r = if path.extension().is_some_and(|e| e == "png") { with_app(|a| a.gfx.save_png(&path).map_err(std::io::Error::other)).unwrap_or(Ok(())) } else { snapshot(hwnd, &path) };
                    match r {
                        Ok(()) => crate::util::log(&["snapshot: ", &path.to_string_lossy()]),
                        Err(_) => crate::util::log(&["snapshot failed"]),
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
                    a.sizing_until = Some(Instant::now() + Duration::from_millis(150));
                    a.update_size();
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_EXITSIZEMOVE => {
                with_app(|a| {
                    a.sizing_until = None;
                    if a.viewer.layout_pending() {
                        a.arm_layout();
                    }
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
            WM_TIMER => {
                with_app(|a| match wp.0 {
                    TIMER_CHROME => a.schedule(),
                    TIMER_WATCH => a.watch(),
                    TIMER_LAYOUT => a.continue_layout(),
                    3 => {
                        KillTimer(Some(hwnd), 3).ok();
                        a.snapshot_ready = true;
                        a.invalidate();
                    }
                    _ => {}
                });
                LRESULT(0)
            }
            images::WM_IMAGE_LOADED => {
                // SAFETY: lParam is the Job box posted by the image loader.
                let ready = with_app(|a| unsafe { a.gfx.images.collect(lp.0) });
                with_app(|a| {
                    if ready == Some(true) {
                        a.viewer.invalidate();
                        a.invalidate();
                    }
                });
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = ((wp.0 >> 16) & 0xffff) as i16 as f32 / 120.0;
                let mut pt = POINT { x: lparam_point(lp).0, y: lparam_point(lp).1 };
                ScreenToClient(hwnd, &mut pt).ok().ok();
                with_app(|a| {
                    let s = dpi_scale(hwnd);
                    let step = a.viewer.font_size * 3.0;
                    a.viewer.wheel(-delta * step, mods(), pt.x as f32 / s, pt.y as f32 / s);
                    a.invalidate();
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
                let tap = from_touch();
                let effect = with_app(|a| {
                    a.settle();
                    let s = dpi_scale(hwnd);
                    let e = a.viewer.release(x as f32 / s, y as f32 / s, tap, Instant::now());
                    a.schedule();
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
                let touch = from_touch();
                with_app(|a| {
                    if !a.tracking {
                        let mut tme = TRACKMOUSEEVENT { cbSize: size_of::<TRACKMOUSEEVENT>() as u32, dwFlags: TME_LEAVE, hwndTrack: hwnd, dwHoverTime: 0 };
                        a.tracking = TrackMouseEvent(&mut tme).is_ok();
                    }
                    let s = dpi_scale(hwnd);
                    a.mouse = (x as f32 / s, y as f32 / s);
                    if !touch && a.viewer.mouse_move(a.mouse.0, a.mouse.1, Instant::now()) {
                        a.invalidate();
                    }
                    a.schedule();
                });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                with_app(|a| {
                    a.tracking = false;
                    a.viewer.mouse_leave();
                    a.schedule();
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_SETCURSOR if (lp.0 & 0xffff) as u32 == HTCLIENT => {
                let cursor = with_app(|a| {
                    let (x, y) = a.mouse;
                    if a.viewer.over_button(x, y) || a.viewer.over_link(x, y) {
                        IDC_HAND
                    } else if a.viewer.over_chrome_area(x, y) {
                        IDC_ARROW
                    } else {
                        IDC_IBEAM
                    }
                })
                .unwrap_or(IDC_ARROW);
                if let Ok(c) = LoadCursorW(None, cursor) {
                    SetCursor(Some(c));
                }
                LRESULT(1)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let Some(key) = key_from_vk(wp.0 as u16) else { return DefWindowProcW(hwnd, msg, wp, lp) };
                let effect = with_app(|a| {
                    if !matches!(key, Key::PageDown | Key::PageUp | Key::Space | Key::Down | Key::Up | Key::Left | Key::Right | Key::Home | Key::End) {
                        a.settle();
                    }
                    let e = a.viewer.key(key, mods(), Instant::now());
                    a.schedule();
                    a.invalidate();
                    e
                })
                .flatten();
                if let Some(e) = effect {
                    apply(hwnd, e);
                }
                LRESULT(0)
            }
            WM_CHAR => {
                if let Some(c) = char::from_u32(wp.0 as u32) {
                    with_app(|a| {
                        a.settle();
                        if a.viewer.text_input(c) {
                            a.invalidate();
                        }
                    });
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
                with_app(|a| a.save_settings());
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

pub fn run(path: Option<PathBuf>, sample: &'static str) {
    unsafe {
        // PNG snapshots render into an offscreen bitmap (same software rasteriser as the window).
        let offscreen = std::env::var("MARKDOWNR_SNAPSHOT").is_ok_and(|p| p.ends_with(".png"));
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED);
        {
            let mut buf = [0u16; 85];
            let n = windows::Win32::Globalization::GetUserDefaultLocaleName(&mut buf);
            if n > 1 {
                crate::transforms::set_locale(&String::from_utf16_lossy(&buf[..n as usize - 1]));
            }
        }
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
        // Initial size: 980 × 820 DIPs.
        let s = dpi_scale(hwnd);
        SetWindowPos(hwnd, None, 0, 0, (980.0 * s) as i32, (820.0 * s) as i32, SWP_NOMOVE | SWP_NOZORDER).ok();
        DragAcceptFiles(hwnd, true);
        // Touch: one-finger pan in both directions (with inertia) and pinch zoom.
        let configs = [
            GESTURECONFIG { dwID: GID_PAN, dwWant: 0x2 | 0x4 | 0x10, dwBlock: 0x8 },
            GESTURECONFIG { dwID: GID_ZOOM, dwWant: 1, dwBlock: 0 },
        ];
        SetGestureConfig(hwnd, 0, &configs, size_of::<GESTURECONFIG>() as u32).ok();

        let settings_path = std::env::var_os("MARKDOWNR_SETTINGS").map(PathBuf::from).unwrap_or_else(settings_path);
        let mut settings = Settings::load(&settings_path);
        let mut gfx = Gfx::new(hwnd).expect("Direct2D/DirectWrite");
        gfx.offscreen_mode = offscreen;
        {
            // Create the render target now, in parallel with the parse.
            let (w, h) = client_size(hwnd);
            let _ = gfx.ensure_target(w, h, dpi_scale(hwnd) * 96.0);
        }
        let sample_mode = std::env::var_os("MARKDOWNR_SAMPLE").is_some();
        let (title, source, doc) = match background {
            Some(job) => BackgroundDoc::join(job),
            None if sample_mode => ("Beispiel".into(), sample.to_string(), parse(sample)),
            None => {
                let start = settings.start_page();
                ("Markdownr".into(), start.clone(), parse(&start))
            }
        };
        if let Some(p) = &path {
            settings.add_recent(p);
            settings.save(&settings_path);
        }
        let mut viewer = Viewer::new(title, source, doc);
        viewer.font_size = settings.font_size;
        viewer.theme_choice = ThemeChoice::from_name(&settings.theme);
        viewer.remember_fullscreen = settings.remember_fullscreen;
        let start_fullscreen = settings.remember_fullscreen && settings.fullscreen;
        gfx.images.set_base(path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)).or_else(|| std::env::current_dir().ok()));
        let mut app = App {
            hwnd,
            gfx,
            viewer,
            modified: path.as_deref().and_then(mtime),
            path,
            fullscreen: None,
            mouse: (0.0, 0.0),
            tracking: false,
            pan: None,
            zoom: None,
            chrome_timer: false,
            sizing_until: None,
            snapshot: std::env::var_os("MARKDOWNR_SNAPSHOT").map(PathBuf::from),
            exit_after_snapshot: std::env::var_os("MARKDOWNR_SNAPSHOT_EXIT").is_some(),
            snapshot_ready: false,
            caption: u32::MAX,
            settings,
            settings_path,
            sample,
        };
        // Test hooks for snapshots: open the TOC / run a search at start.
        if std::env::var_os("MARKDOWNR_TOC").is_some() {
            app.viewer.key(Key::Char('T'), Mods { ctrl: true, shift: false }, Instant::now());
        }
        if let Ok(q) = std::env::var("MARKDOWNR_FIND") {
            app.viewer.key(Key::Char('F'), Mods { ctrl: true, shift: false }, Instant::now());
            q.chars().for_each(|c| {
                app.viewer.text_input(c);
            });
        }
        if std::env::var_os("MARKDOWNR_HELP").is_some() {
            app.viewer.key(Key::F1, Mods::default(), Instant::now());
        }
        if std::env::var_os("MARKDOWNR_CHROME").is_some() {
            app.viewer.chrome_alpha = 1.0;
            app.viewer.show_chrome(Instant::now() + Duration::from_secs(3600));
        }
        {
            let st = windows::Win32::System::SystemInformation::GetLocalTime();
            app.viewer.today = (st.wYear as i32, st.wMonth as u32, st.wDay as u32);
            let mut anim = BOOL(1);
            let _ = SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut anim as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
            app.viewer.reduce_motion = !anim.as_bool();
            if let Some(ms) = std::env::var("MARKDOWNR_TIME_MS").ok().and_then(|s| s.parse::<u64>().ok()) {
                app.viewer.anim_start = Instant::now() - Duration::from_millis(ms);
            }
        }
        app.apply_dark();
        app.set_title();
        app.update_size();
        APP.with(|c| *c.borrow_mut() = Some(app));
        with_app(|a| a.schedule());
        if let Some(px) = std::env::var("MARKDOWNR_SCROLL").ok().and_then(|s| crate::util::parse_f32(&s)) {
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
        SetTimer(Some(hwnd), TIMER_WATCH, 1000, None);
        if std::env::var_os("MARKDOWNR_SNAPSHOT").is_some() {
            SetTimer(Some(hwnd), 3, 1500, None); // snapshot after 1.5 s (images, fades)
        }
        let _ = ShowWindow(hwnd, SW_SHOW);
        if start_fullscreen {
            toggle_fullscreen(hwnd, Some(true));
        }

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        APP.with(|c| c.borrow_mut().take());
    }
}
