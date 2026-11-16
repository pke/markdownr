//! Linux shell: a plain X11 window (Xlib; XWayland on Wayland desktops), drawn with cairo
//! into a back pixmap and copied to the window, text by Pango. The app logic is
//! `crate::app`; this is the window, its event loop (one `poll` on the X connection and
//! the image wake socket, with the timers as its timeout) and what the X11 world offers:
//! the CLIPBOARD selection, EWMH full screen, a dark title bar hint, the window icon.

mod ffi;
mod gfx;
mod images;

use crate::app::{App, Cursor, Host, Timer};
use crate::settings::Settings;
use crate::view::{Effect, Key, Mods};
use core::ffi::{c_char, c_int, c_uint, c_ulong};
use ffi::*;
use gfx::Gfx;
use std::cell::Cell;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

/// The system's dark appearance as found by the background query: 0 unknown, 1 light, 2 dark.
static DARK: AtomicU8 = AtomicU8::new(0);

struct Atoms {
    delete: Atom,
    protocols: Atom,
    name: Atom,
    utf8: Atom,
    clipboard: Atom,
    targets: Atom,
    state: Atom,
    fullscreen: Atom,
    variant: Atom,
    icon: Atom,
    class: Atom,
}

/// What the shared app asks of the window.
struct XHost {
    dpy: *mut Display,
    win: Window,
    atoms: Atoms,
    /// A repaint is due (painted once the event queue is empty).
    dirty: Cell<bool>,
    /// Chrome and layout timers: (next firing, period).
    timers: [Option<(Instant, Duration)>; 2],
}

impl Host for XHost {
    fn invalidate(&self) {
        self.dirty.set(true);
    }

    fn set_title(&self, title: &str) {
        unsafe { XChangeProperty(self.dpy, self.win, self.atoms.name, self.atoms.utf8, 8, 0, title.as_ptr(), title.len() as c_int) };
    }

    fn set_timer(&mut self, timer: Timer, every: Option<Duration>) {
        self.timers[timer as usize] = every.map(|d| (Instant::now() + d, d));
    }

    fn open_dialog(&mut self) -> Option<PathBuf> {
        // No toolkit, so the desktop's own dialog tools: zenity (GNOME and most others), kdialog (KDE).
        let run = |cmd: &str, args: &[&str]| std::process::Command::new(cmd).args(args).output().ok().filter(|o| o.status.success());
        let out = run("zenity", &["--file-selection", "--title=Markdownr", "--file-filter=Markdown | *.md *.markdown *.mdown *.mkd *.txt", "--file-filter=* | *"])
            .or_else(|| run("kdialog", &["--getopenfilename", ".", "*.md *.markdown *.mdown *.mkd *.txt"]))?;
        let path = String::from_utf8(out.stdout).ok()?;
        let path = path.trim_end_matches('\n');
        (!path.is_empty()).then(|| PathBuf::from(path))
    }

    fn open_external(&mut self, target: &str) {
        if let Ok(mut child) = std::process::Command::new("xdg-open").arg(target).spawn() {
            // Reap it when it exits (no zombie).
            std::thread::spawn(move || child.wait());
        }
    }
}

/// The app plus the X11 state.
struct Lx {
    app: App<Gfx, XHost>,
    ic: XIC,
    gc: GC,
    visual: *mut Visual,
    depth: c_int,
    /// Back buffer: (pixmap, its width and height in pixels).
    back: Option<(Pixmap, u32, u32)>,
    /// Window size in pixels.
    size: (u32, u32),
    scale: f32,
    cursors: [ffi::Cursor; 3],
    cursor: Option<Cursor>,
    /// Text we offer on the CLIPBOARD selection.
    clipboard: Option<String>,
    /// Theme brightness last told the window manager (_GTK_THEME_VARIANT).
    variant: Option<bool>,
    quit: bool,
}

fn mods(state: c_uint) -> Mods {
    Mods { ctrl: state & CONTROL_MASK != 0, shift: state & SHIFT_MASK != 0 }
}

fn key_from_keysym(k: KeySym) -> Option<Key> {
    Some(match k {
        0xff56 | 0xff9b => Key::PageDown,
        0xff55 | 0xff9a => Key::PageUp,
        0x20 => Key::Space,
        0xff54 | 0xff99 => Key::Down,
        0xff52 | 0xff97 => Key::Up,
        0xff51 | 0xff96 => Key::Left,
        0xff53 | 0xff98 => Key::Right,
        0xff50 | 0xff95 => Key::Home,
        0xff57 | 0xff9c => Key::End,
        0xffbe => Key::F1,
        0xffc0 => Key::F3,
        0xffc2 => Key::F5,
        0xffc8 => Key::F11,
        0xff1b => Key::Escape,
        0xff0d | 0xff8d => Key::Enter,
        0xff08 => Key::Backspace,
        0x2b | 0xffab => Key::Char('+'),
        0x3d => Key::Char('='),
        0x2d | 0xffad => Key::Char('-'),
        0xffb0 => Key::Char('0'),
        0x30..=0x39 => Key::Char(k as u8 as char),
        0x61..=0x7a => Key::Char((k as u8 - 32) as char),
        0x41..=0x5a => Key::Char(k as u8 as char),
        _ => return None,
    })
}

fn atom(dpy: *mut Display, name: &core::ffi::CStr) -> Atom {
    unsafe { XInternAtom(dpy, name.as_ptr(), 0) }
}

/// Display scale: GDK_SCALE (whole factors, as GTK) times Xft.dpi / 96.
fn display_scale(dpy: *mut Display) -> f32 {
    let mut dpi = 96.0;
    unsafe {
        let rm = XResourceManagerString(dpy);
        if !rm.is_null() {
            let text = std::ffi::CStr::from_ptr(rm).to_string_lossy();
            if let Some(v) = text.lines().find_map(|l| l.strip_prefix("Xft.dpi:")) {
                dpi = crate::util::parse_f32(v.trim()).filter(|&d| d > 0.0).unwrap_or(96.0);
            }
        }
    }
    let gdk = crate::util::env::string("GDK_SCALE").and_then(|s| crate::util::parse_f32(&s)).filter(|&s| s >= 1.0).unwrap_or(1.0);
    (dpi / 96.0 * gdk).max(0.5)
}

/// Language and locale from LC_ALL / LC_MESSAGES / LANG ("de_DE.UTF-8" → "de", "de-DE").
fn posix_locale() -> Option<String> {
    let v = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|n| crate::util::env::string(n).filter(|v| !v.is_empty() && v != "C" && v != "POSIX"))?;
    Some(v.split(['.', '@']).next().unwrap_or("").replace('_', "-"))
}

/// The desktop's dark preference (GNOME's color-scheme, or a GTK_THEME ending in :dark),
/// asked off the UI thread; `wake` gets a byte when known.
fn query_dark(mut wake: UnixStream) {
    std::thread::spawn(move || {
        let theme_dark = crate::util::env::string("GTK_THEME").is_some_and(|t| t.ends_with(":dark"));
        let scheme_dark = std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("dark"));
        DARK.store(if theme_dark || scheme_dark { 2 } else { 1 }, Ordering::Relaxed);
        let _ = std::io::Write::write(&mut wake, &[2]);
    });
}

/// The PNG images of an .ico file (Markdownr's icon has PNG entries) as _NET_WM_ICON data:
/// per size width, height and the ARGB pixels, each a CARDINAL (a C long).
fn icon_data() -> Vec<c_ulong> {
    static ICO: &[u8] = include_bytes!("../../assets/markdownr.ico");
    let u16_at = |o: usize| ICO.get(o..o + 2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let u32_at = |o: usize| ICO.get(o..o + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut out = Vec::new();
    for i in 0..u16_at(4) {
        let e = 6 + 16 * i;
        let size = match ICO.get(e) {
            Some(0) => 256,
            Some(&s) => s as usize,
            None => continue,
        };
        if size != 48 && size != 256 {
            continue;
        }
        let Some(png) = ICO.get(u32_at(e + 12)..u32_at(e + 12) + u32_at(e + 8)) else { continue };
        unsafe extern "C" fn read(closure: *mut core::ffi::c_void, data: *mut u8, n: c_uint) -> c_int {
            // SAFETY: closure is the &mut &[u8] below, alive for the call.
            let rest = unsafe { &mut *(closure as *mut &[u8]) };
            if rest.len() < n as usize {
                return 11; // CAIRO_STATUS_READ_ERROR
            }
            unsafe { core::ptr::copy_nonoverlapping(rest.as_ptr(), data, n as usize) };
            *rest = &rest[n as usize..];
            0
        }
        unsafe {
            let mut cursor: &[u8] = png;
            let s = cairo_image_surface_create_from_png_stream(read, &mut cursor as *mut _ as *mut _);
            if cairo_surface_status(s) == 0 {
                let (w, h, stride) = (cairo_image_surface_get_width(s) as usize, cairo_image_surface_get_height(s) as usize, cairo_image_surface_get_stride(s) as usize);
                let data = cairo_image_surface_get_data(s);
                out.extend([w as c_ulong, h as c_ulong]);
                for y in 0..h {
                    for x in 0..w {
                        let p = data.add(y * stride + x * 4) as *const u32;
                        let argb = *p;
                        // Cairo's premultiplied ARGB back to plain ARGB.
                        let a = argb >> 24;
                        let un = |c: u32| if a == 0 { 0 } else { ((c * 255 + a / 2) / a).min(255) };
                        out.push(((a << 24) | (un((argb >> 16) & 255) << 16) | (un((argb >> 8) & 255) << 8) | un(argb & 255)) as c_ulong);
                    }
                }
            }
            cairo_surface_destroy(s);
        }
    }
    out
}

impl Lx {
    fn dpy(&self) -> *mut Display {
        self.app.host.dpy
    }

    fn win(&self) -> Window {
        self.app.host.win
    }

    fn paint(&mut self) {
        self.app.host.dirty.set(false);
        let (w, h) = (self.size.0.max(1), self.size.1.max(1));
        if self.back.is_none_or(|b| (b.1, b.2) != (w, h)) {
            unsafe {
                if let Some((p, ..)) = self.back.take() {
                    XFreePixmap(self.dpy(), p);
                }
                if !self.app.gfx.target.is_null() {
                    cairo_surface_destroy(self.app.gfx.target);
                }
                let p = XCreatePixmap(self.dpy(), self.win(), w, h, self.depth as c_uint);
                let s = cairo_xlib_surface_create(self.dpy(), p, self.visual, w as c_int, h as c_int);
                cairo_surface_set_device_scale(s, self.scale as f64, self.scale as f64);
                self.app.gfx.target = s;
                self.back = Some((p, w, h));
            }
        }
        self.app.layout();
        self.theme_variant();
        self.app.draw();
        if let Some((p, w, h)) = self.back {
            unsafe { XCopyArea(self.dpy(), p, self.win(), self.gc, 0, 0, w, h, 0, 0) };
        }
    }

    /// _GTK_THEME_VARIANT: window managers draw a dark title bar for a dark theme; dark as
    /// Windows' title bar (`Theme::caption`: the page, or the toolbar while it shows).
    fn theme_variant(&mut self) {
        let v = &self.app.viewer;
        let dark = v.theme.caption(if self.app.fullscreen { 0.0 } else { v.chrome_alpha }).dark;
        if self.variant == Some(dark) {
            return;
        }
        self.variant = Some(dark);
        let v: &[u8] = if dark { b"dark" } else { b"light" };
        let a = &self.app.host.atoms;
        unsafe { XChangeProperty(self.dpy(), self.win(), a.variant, a.utf8, 8, 0, v.as_ptr(), v.len() as c_int) };
    }

    fn set_fullscreen(&mut self, on: bool) {
        let a = &self.app.host.atoms;
        let mut ev = XEvent::zeroed();
        ev.set_i32(0, CLIENT_MESSAGE);
        ev.set_ulong(32, self.win());
        ev.set_ulong(40, a.state);
        ev.set_i32(48, 32);
        ev.set_ulong(56, on as c_ulong); // _NET_WM_STATE_REMOVE 0 / _ADD 1
        ev.set_ulong(64, a.fullscreen);
        ev.set_ulong(80, 1); // source: application
        unsafe {
            let root = XRootWindow(self.dpy(), XDefaultScreen(self.dpy()));
            XSendEvent(self.dpy(), root, 0, SUBSTRUCTURE_REDIRECT_MASK | SUBSTRUCTURE_NOTIFY_MASK, &mut ev);
        }
        self.app.set_fullscreen(on);
    }

    fn apply(&mut self, effect: Effect) {
        match self.app.effect(effect) {
            Some(Effect::Copy(text)) => {
                self.clipboard = Some(text);
                unsafe { XSetSelectionOwner(self.dpy(), self.app.host.atoms.clipboard, self.win(), 0) };
            }
            Some(Effect::ToggleFullscreen) => self.set_fullscreen(!self.app.fullscreen),
            Some(Effect::ExitFullscreen) => self.set_fullscreen(false),
            Some(Effect::Quit) => self.quit = true,
            _ => {}
        }
    }

    fn update_cursor(&mut self) {
        let c = self.app.cursor();
        if self.cursor != Some(c) {
            self.cursor = Some(c);
            unsafe { XDefineCursor(self.dpy(), self.win(), self.cursors[c as usize]) };
        }
    }

    /// Answer another client asking for our CLIPBOARD text.
    fn selection_request(&self, ev: &XEvent) {
        let (requestor, selection, target, property, time) = (ev.ulong(40), ev.ulong(48), ev.ulong(56), ev.ulong(64), ev.ulong(72));
        let a = &self.app.host.atoms;
        let mut answered = property;
        unsafe {
            if target == a.targets {
                let list: [Atom; 3] = [a.targets, a.utf8, XA_STRING];
                XChangeProperty(self.dpy(), requestor, property, XA_ATOM, 32, 0, list.as_ptr() as *const u8, 3);
            } else if let (true, Some(text)) = (target == a.utf8 || target == XA_STRING, &self.clipboard) {
                XChangeProperty(self.dpy(), requestor, property, target, 8, 0, text.as_ptr(), text.len() as c_int);
            } else {
                answered = 0;
            }
            let mut reply = XEvent::zeroed();
            reply.set_i32(0, SELECTION_NOTIFY);
            reply.set_i32(16, 1);
            reply.set_ulong(32, requestor);
            reply.set_ulong(40, selection);
            reply.set_ulong(48, target);
            reply.set_ulong(56, answered);
            reply.set_ulong(64, time);
            XSendEvent(self.dpy(), requestor, 0, 0, &mut reply);
        }
    }

    fn event(&mut self, ev: &mut XEvent) {
        let s = self.scale;
        match ev.kind() {
            EXPOSE => self.app.invalidate(),
            CONFIGURE_NOTIFY => {
                let size = (ev.i32(56).max(1) as u32, ev.i32(60).max(1) as u32);
                if size != self.size {
                    self.size = size;
                    self.app.resize(size.0 as f32 / s, size.1 as f32 / s, true);
                }
            }
            KEY_PRESS => {
                let mut buf = [0 as c_char; 64];
                let mut keysym: KeySym = 0;
                let mut status = 0;
                let n = unsafe {
                    if self.ic.is_null() {
                        XLookupString(ev, buf.as_mut_ptr(), buf.len() as c_int, &mut keysym, core::ptr::null_mut())
                    } else {
                        Xutf8LookupString(self.ic, ev, buf.as_mut_ptr(), buf.len() as c_int, &mut keysym, &mut status)
                    }
                };
                let m = mods(ev.u32(80));
                if let Some(key) = key_from_keysym(keysym) {
                    if let Some(e) = self.app.key(key, m) {
                        self.apply(e);
                    }
                }
                if n > 0 && !m.ctrl {
                    let bytes = unsafe { core::slice::from_raw_parts(buf.as_ptr() as *const u8, n as usize) };
                    for c in String::from_utf8_lossy(bytes).chars().filter(|c| !c.is_control()) {
                        self.app.text(c);
                    }
                }
            }
            BUTTON_PRESS => {
                let (x, y, m) = (ev.i32(64) as f32 / s, ev.i32(68) as f32 / s, mods(ev.u32(80)));
                match ev.u32(84) {
                    1 => self.app.press(x, y, m, Duration::from_millis(400)),
                    4 => self.app.wheel(-1.0, true, m, x, y),
                    5 => self.app.wheel(1.0, true, m, x, y),
                    // The mouse's back and forward buttons walk the history.
                    8 => _ = self.app.back(),
                    9 => _ = self.app.forward(),
                    _ => {}
                }
            }
            BUTTON_RELEASE if ev.u32(84) == 1 => {
                if let Some(e) = self.app.release(ev.i32(64) as f32 / s, ev.i32(68) as f32 / s, false) {
                    self.apply(e);
                }
            }
            MOTION_NOTIFY => {
                self.app.mouse_move(ev.i32(64) as f32 / s, ev.i32(68) as f32 / s, false);
                self.update_cursor();
            }
            LEAVE_NOTIFY => self.app.mouse_leave(),
            FOCUS_IN if !self.ic.is_null() => unsafe { XSetICFocus(self.ic) },
            FOCUS_OUT if !self.ic.is_null() => unsafe { XUnsetICFocus(self.ic) },
            SELECTION_REQUEST => self.selection_request(ev),
            CLIENT_MESSAGE if ev.ulong(40) == self.app.host.atoms.protocols && ev.ulong(56) == self.app.host.atoms.delete => self.quit = true,
            _ => {}
        }
    }
}

pub fn run(path: Option<PathBuf>, sample: &'static str) {
    unsafe {
        setlocale(LC_CTYPE, c"".as_ptr());
        XSetLocaleModifiers(c"".as_ptr());
    }
    let locale = posix_locale();
    crate::app::init_locale(|| locale.as_deref().and_then(|l| l.get(..2)).unwrap_or("en").to_ascii_lowercase(), || locale.clone());
    let dpy = unsafe { XOpenDisplay(core::ptr::null()) };
    if dpy.is_null() {
        let _ = std::io::Write::write_all(&mut std::io::stderr(), b"Markdownr: cannot open the X display (DISPLAY not set?)\n");
        return;
    }
    let Ok((mut wake_rx, wake_tx)) = UnixStream::pair() else { return };
    let _ = wake_rx.set_nonblocking(true);
    let scale = display_scale(dpy);
    unsafe {
        let screen = XDefaultScreen(dpy);
        let root = XRootWindow(dpy, screen);
        let (w, h) = ((980.0 * scale) as u32, (820.0 * scale) as u32);
        let win = XCreateSimpleWindow(dpy, root, 0, 0, w, h, 0, 0, 0);
        // No background: the server does not clear the window before our frame.
        XSetWindowBackgroundPixmap(dpy, win, 0);
        let atoms = Atoms {
            delete: atom(dpy, c"WM_DELETE_WINDOW"),
            protocols: atom(dpy, c"WM_PROTOCOLS"),
            name: atom(dpy, c"_NET_WM_NAME"),
            utf8: atom(dpy, c"UTF8_STRING"),
            clipboard: atom(dpy, c"CLIPBOARD"),
            targets: atom(dpy, c"TARGETS"),
            state: atom(dpy, c"_NET_WM_STATE"),
            fullscreen: atom(dpy, c"_NET_WM_STATE_FULLSCREEN"),
            variant: atom(dpy, c"_GTK_THEME_VARIANT"),
            icon: atom(dpy, c"_NET_WM_ICON"),
            class: atom(dpy, c"WM_CLASS"),
        };
        XSetWMProtocols(dpy, win, &atoms.delete, 1);
        let class = b"markdownr\0Markdownr\0";
        XChangeProperty(dpy, win, atoms.class, XA_STRING, 8, 0, class.as_ptr(), class.len() as c_int);
        let icon = icon_data();
        if !icon.is_empty() {
            XChangeProperty(dpy, win, atoms.icon, XA_CARDINAL, 32, 0, icon.as_ptr() as *const u8, icon.len() as c_int);
        }
        XSelectInput(dpy, win, KEY_PRESS_MASK | BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK | LEAVE_WINDOW_MASK | POINTER_MOTION_MASK | EXPOSURE_MASK | STRUCTURE_NOTIFY_MASK | FOCUS_CHANGE_MASK);
        // Input method: composed and dead-key characters in the find field.
        let im = XOpenIM(dpy, core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut());
        let ic = if im.is_null() {
            core::ptr::null_mut()
        } else {
            const XIM_PREEDIT_NOTHING: c_ulong = 0x0008;
            const XIM_STATUS_NOTHING: c_ulong = 0x0400;
            XCreateIC(im, c"inputStyle".as_ptr(), XIM_PREEDIT_NOTHING | XIM_STATUS_NOTHING, c"clientWindow".as_ptr(), win, c"focusWindow".as_ptr(), win, core::ptr::null::<c_char>())
        };
        query_dark(wake_tx.try_clone().expect("socket"));

        let settings_path = crate::app::settings_path();
        let settings = Settings::load(&settings_path);
        let gfx = Gfx::new(images::Images::new(wake_tx));
        let doc = match &path {
            Some(p) => crate::app::read_doc(p),
            None => crate::app::start_doc(&settings, sample),
        };
        let host = XHost { dpy, win, atoms, dirty: Cell::new(true), timers: [None, None] };
        let mut app = App::new(host, gfx, path, doc, settings, settings_path, sample);
        let start_fullscreen = app.start_fullscreen();
        let mut tm = Tm::default();
        let now = time(core::ptr::null_mut());
        if !localtime_r(&now, &mut tm).is_null() {
            app.viewer.today = (tm.year + 1900, (tm.mon + 1) as u32, tm.mday as u32);
        }
        app.viewer.pixel = scale;
        app.set_title();
        app.resize(w as f32 / scale, h as f32 / scale, false);
        let mut lx = Lx {
            app,
            ic,
            gc: XCreateGC(dpy, win, 0, core::ptr::null_mut()),
            visual: XDefaultVisual(dpy, screen),
            depth: XDefaultDepth(dpy, screen),
            back: None,
            size: (w, h),
            scale,
            // XC_left_ptr, XC_hand2, XC_xterm in the order of app::Cursor.
            cursors: [XCreateFontCursor(dpy, 68), XCreateFontCursor(dpy, 60), XCreateFontCursor(dpy, 152)],
            cursor: None,
            clipboard: None,
            variant: None,
            quit: false,
        };
        lx.app.schedule();
        XMapWindow(dpy, win);
        if start_fullscreen {
            lx.set_fullscreen(true);
        }
        let mut watch = Instant::now() + Duration::from_secs(1);
        let xfd = XConnectionNumber(dpy);
        let mut ev = XEvent::zeroed();
        while !lx.quit {
            while XPending(dpy) > 0 && !lx.quit {
                XNextEvent(dpy, &mut ev);
                if XFilterEvent(&mut ev, 0) == 0 {
                    lx.event(&mut ev);
                }
            }
            if lx.quit {
                break;
            }
            // Wake-ups: decoded images, the dark-mode answer.
            let mut buf = [0u8; 64];
            let mut woke = false;
            while matches!(wake_rx.read(&mut buf), Ok(n) if n > 0) {
                woke = true;
            }
            if woke {
                if lx.app.gfx.images.collect() {
                    lx.app.viewer.invalidate();
                    lx.app.invalidate();
                }
                match DARK.swap(0, Ordering::Relaxed) {
                    0 => {}
                    d => lx.app.set_dark(d == 2),
                }
            }
            // Timers.
            let now = Instant::now();
            for t in [Timer::Chrome, Timer::Layout] {
                if let Some((due, every)) = lx.app.host.timers[t as usize] {
                    if due <= now {
                        lx.app.host.timers[t as usize] = Some((now + every, every));
                        match t {
                            Timer::Chrome => lx.app.schedule(),
                            Timer::Layout => lx.app.continue_layout(),
                        }
                    }
                }
            }
            if watch <= now {
                watch = now + Duration::from_secs(1);
                lx.app.watch();
            }
            if lx.app.host.dirty.get() {
                lx.paint();
            }
            XFlush(dpy);
            if XPending(dpy) > 0 {
                continue;
            }
            let next = lx.app.host.timers.iter().flatten().map(|t| t.0).chain([watch]).min().unwrap_or(watch);
            let timeout = next.saturating_duration_since(Instant::now()).as_millis().min(60_000) as c_int;
            let mut fds = [PollFd { fd: xfd, events: 1, revents: 0 }, PollFd { fd: wake_rx.as_raw_fd(), events: 1, revents: 0 }];
            poll(fds.as_mut_ptr(), 2, timeout.max(if lx.app.host.dirty.get() { 0 } else { 1 }));
        }
        lx.app.save_settings();
        XDestroyWindow(dpy, win);
        XCloseDisplay(dpy);
    }
}
