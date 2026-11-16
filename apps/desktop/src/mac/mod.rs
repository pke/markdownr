//! macOS shell: an AppKit window whose view (a class made at run time through the
//! Objective-C runtime, no binding crate) draws with Core Graphics and Core Text. The app
//! logic is `crate::app`; this is the window, its view's events, NSTimers for the app's
//! timers, and what macOS offers: the pasteboard, native full screen, the open panel,
//! opening files from Finder.

pub mod ffi;
mod gfx;
mod images;

use crate::app::{App, Cursor, Host, Timer};
use crate::msg;
use crate::settings::Settings;
use crate::view::{Effect, Key, Mods};
use core::ffi::c_void;
use ffi::*;
use gfx::Gfx;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Duration;

struct MacHost {
    window: Id,
    view: Id,
    /// The NSTimers of Timer::Chrome, Timer::Layout and Timer::Poll.
    timers: [Id; 3],
    /// FSEvents stream of what is watched (`Host::watch`), on the main queue; null for none.
    events: *mut c_void,
    /// The view's display link (macOS 14+) for animation frames: they come in step with
    /// the display (120 Hz on ProMotion) instead of from a 16 ms timer. Null until needed
    /// or where there is none.
    link: Id,
}

impl MacHost {
    /// Run animation frames from the display link; false where there is none.
    fn frames(&mut self, on: bool) -> bool {
        if self.link.is_null() {
            if !on || !msg!(self.view, c"respondsToSelector:", sel(c"displayLinkWithTarget:selector:") => bool) {
                return false;
            }
            self.link = msg!(msg!(self.view, c"displayLinkWithTarget:selector:", self.view, sel(c"frameTick:") => Id), c"retain" => Id);
            if self.link.is_null() {
                return false;
            }
            let rl = msg!(class(c"NSRunLoop"), c"mainRunLoop" => Id);
            msg!(self.link, c"addToRunLoop:forMode:", rl, unsafe { kCFRunLoopCommonModes } as Id);
            // CAFrameRateRange (minimum, maximum, preferred): the display's full rate, not
            // the 60 Hz a link gets by default.
            #[repr(C)]
            struct FrameRateRange(f32, f32, f32);
            msg!(self.link, c"setPreferredFrameRateRange:", FrameRateRange(60.0, 120.0, 120.0));
        }
        msg!(self.link, c"setPaused:", !on);
        true
    }
}

impl Host for MacHost {
    fn invalidate(&self) {
        msg!(self.view, c"setNeedsDisplay:", true);
    }

    fn set_title(&self, title: &str) {
        msg!(self.window, c"setTitle:", nsstring(title));
    }

    fn set_timer(&mut self, timer: Timer, every: Option<Duration>) {
        // Animation frames (FRAME) from the display link, longer waits from a timer.
        let frames = timer == Timer::Chrome && every.is_some_and(|d| d <= crate::view::FRAME) && self.frames(true);
        if timer == Timer::Chrome && !frames {
            self.frames(false);
        }
        let every = if frames { None } else { every };
        let slot = &mut self.timers[timer as usize];
        if !slot.is_null() {
            msg!(*slot, c"invalidate");
            *slot = core::ptr::null_mut();
        }
        if let Some(d) = every {
            let selector = match timer {
                Timer::Chrome => sel(c"chromeTick:"),
                Timer::Layout => sel(c"layoutTick:"),
                Timer::Poll => sel(c"watchTick:"),
            };
            *slot = msg!(class(c"NSTimer"), c"scheduledTimerWithTimeInterval:target:selector:userInfo:repeats:", d.as_secs_f64(), self.view, selector, core::ptr::null_mut::<c_void>(), true => Id);
        }
    }

    fn open_dialog(&mut self, folder: bool) -> Option<PathBuf> {
        let panel = msg!(class(c"NSOpenPanel"), c"openPanel" => Id);
        msg!(panel, c"setCanChooseDirectories:", folder);
        msg!(panel, c"setCanChooseFiles:", !folder);
        msg!(panel, c"setAllowsMultipleSelection:", false);
        if msg!(panel, c"runModal" => isize) != 1 {
            return None;
        }
        let url = msg!(panel, c"URL" => Id);
        let path = string(msg!(url, c"path" => Id));
        (!path.is_empty()).then(|| PathBuf::from(path))
    }

    fn open_external(&mut self, target: &str) {
        let ws = msg!(class(c"NSWorkspace"), c"sharedWorkspace" => Id);
        let lower = target.to_ascii_lowercase();
        let url = if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("mailto:") {
            msg!(class(c"NSURL"), c"URLWithString:", nsstring(target) => Id)
        } else {
            msg!(class(c"NSURL"), c"fileURLWithPath:", nsstring(target) => Id)
        };
        if !url.is_null() {
            msg!(ws, c"openURL:", url => bool);
        }
    }

    fn minimized(&self) -> bool {
        msg!(self.window, c"isMiniaturized" => bool)
    }

    fn watch(&mut self, dirs: &[PathBuf], _tree: bool) -> bool {
        // The stream may be the one whose callback runs now (App::changed watches anew):
        // it ends once that is over.
        unsafe extern "C" fn end(stream: *mut c_void) {
            unsafe {
                FSEventStreamStop(stream);
                FSEventStreamInvalidate(stream);
                FSEventStreamRelease(stream);
            }
        }
        let main = unsafe { &_dispatch_main_q as *const _ as *const c_void };
        if !self.events.is_null() {
            unsafe { dispatch_async_f(main, self.events, end) };
            self.events = core::ptr::null_mut();
        }
        let Some(dir) = dirs.first() else { return true };
        // FSEvents watches the whole tree below a directory, so the first is all it needs;
        // its events name directories only, which is all App::changed asks.
        let paths = msg!(class(c"NSArray"), c"arrayWithObject:", nsstring(&dir.to_string_lossy()) => Id);
        let context = FSEventStreamContext { version: 0, info: core::ptr::null_mut(), retain: core::ptr::null(), release: core::ptr::null(), copy_description: core::ptr::null() };
        unsafe {
            let stream = FSEventStreamCreate(core::ptr::null(), fs_changed, &context, paths as CFTypeRef, kFSEventStreamEventIdSinceNow, 0.1, kFSEventStreamCreateFlagNoDefer);
            if stream.is_null() {
                return false;
            }
            FSEventStreamSetDispatchQueue(stream, main);
            if !FSEventStreamStart(stream) {
                end(stream);
                return false;
            }
            self.events = stream;
        }
        true
    }
}

/// Something changed where the app watches (FSEvents, on the main queue).
unsafe extern "C" fn fs_changed(_: *mut c_void, _: *mut c_void, _: usize, _: *mut c_void, _: *const u32, _: *const u64) {
    with_app(|a| a.app.changed());
}

struct Mac {
    app: App<Gfx, MacHost>,
    /// Appearance name seen last (light / dark switches).
    appearance: String,
}

/// The app, used only on the main thread.
struct AppCell(RefCell<Option<Mac>>);
// SAFETY: only the main thread touches it (AppKit calls the view there; image results come
// back on the main queue).
unsafe impl Sync for AppCell {}
static APP: AppCell = AppCell(RefCell::new(None));

fn with_app<R>(f: impl FnOnce(&mut Mac) -> R) -> Option<R> {
    APP.0.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f))
}

const CMD: usize = 1 << 20;
const SHIFT: usize = 1 << 17;
const CTRL: usize = 1 << 18;

fn mods(event: Id) -> Mods {
    let flags = msg!(event, c"modifierFlags" => usize);
    Mods { ctrl: flags & CMD != 0, shift: flags & SHIFT != 0 }
}

/// The event's position in the (flipped) view, in points.
fn point(view: Id, event: Id) -> (f32, f32) {
    let p = msg!(event, c"locationInWindow" => CGPoint);
    let v = msg!(view, c"convertPoint:fromView:", p, core::ptr::null_mut::<c_void>() => CGPoint);
    (v.x as f32, v.y as f32)
}

fn key_from_event(event: Id) -> Option<Key> {
    let code = msg!(event, c"keyCode" => u16);
    let flags = msg!(event, c"modifierFlags" => usize);
    Some(match code {
        3 if flags & (CTRL | CMD) == CTRL | CMD => Key::F11, // ⌃⌘F, the Mac's full screen key
        121 => Key::PageDown,
        116 => Key::PageUp,
        49 => Key::Space,
        125 => Key::Down,
        126 => Key::Up,
        123 => Key::Left,
        124 => Key::Right,
        115 => Key::Home,
        119 => Key::End,
        122 => Key::F1,
        99 => Key::F3,
        96 => Key::F5,
        103 => Key::F11,
        53 => Key::Escape,
        36 | 76 => Key::Enter,
        51 => Key::Backspace,
        _ => {
            let c = string(msg!(event, c"charactersIgnoringModifiers" => Id)).chars().next()?;
            match c {
                '+' | '=' | '-' | '0'..='9' => Key::Char(c),
                'a'..='z' | 'A'..='Z' => Key::Char(c.to_ascii_uppercase()),
                _ => return None,
            }
        }
    })
}

fn appearance_name() -> String {
    let app = msg!(class(c"NSApplication"), c"sharedApplication" => Id);
    let a = msg!(app, c"effectiveAppearance" => Id);
    if a.is_null() { String::new() } else { string(msg!(a, c"name" => Id)) }
}

fn apply(effect: Effect) {
    let rest = with_app(|m| m.app.effect(effect)).flatten();
    let window = with_app(|m| m.app.host.window).unwrap_or(core::ptr::null_mut());
    match rest {
        Some(Effect::Copy(text)) => {
            let pb = msg!(class(c"NSPasteboard"), c"generalPasteboard" => Id);
            msg!(pb, c"clearContents" => isize);
            msg!(pb, c"setString:forType:", nsstring(&text), unsafe { NSPasteboardTypeString } => bool);
        }
        Some(Effect::ToggleFullscreen) => msg!(window, c"toggleFullScreen:", core::ptr::null_mut::<c_void>()),
        Some(Effect::ExitFullscreen) => {
            if with_app(|m| m.app.fullscreen) == Some(true) {
                msg!(window, c"toggleFullScreen:", core::ptr::null_mut::<c_void>());
            }
        }
        Some(Effect::Quit) => msg!(window, c"performClose:", core::ptr::null_mut::<c_void>()),
        _ => {}
    }
}

// ------------------------------------------------------------ view methods

extern "C" fn yes(_: Id, _: Sel) -> bool {
    true
}

extern "C" fn draw_rect(view: Id, _: Sel, _dirty: CGRect) {
    let gctx = msg!(class(c"NSGraphicsContext"), c"currentContext" => Id);
    let ctx = msg!(gctx, c"CGContext" => CGContextRef);
    with_app(|m| {
        let name = appearance_name();
        if name != m.appearance {
            m.app.set_dark(name.contains("Dark"));
            m.appearance = name;
        }
        let frame = bounds(view);
        if (frame.size.width as f32, frame.size.height as f32) != (m.app.viewer.view_w, m.app.viewer.view_h) {
            m.app.resize(frame.size.width as f32, frame.size.height as f32, true);
        }
        m.app.viewer.pixel = msg!(m.app.host.window, c"backingScaleFactor" => f64).max(1.0) as f32;
        m.app.gfx.ctx = ctx;
        m.app.layout();
        m.app.draw();
        m.app.gfx.ctx = core::ptr::null_mut();
        if !m.app.fullscreen {
            place_window_buttons(m.app.host.window, m.app.viewer.chrome_alpha);
        }
    });
}

extern "C" fn key_down(view: Id, _: Sel, event: Id) {
    let m = mods(event);
    if let Some(key) = key_from_event(event) {
        if let Some(e) = with_app(|a| a.app.key(key, m)).flatten() {
            apply(e);
        }
    }
    if !m.ctrl {
        for c in string(msg!(event, c"characters" => Id)).chars().filter(|c| !c.is_control() && !('\u{F700}'..='\u{F8FF}').contains(c)) {
            with_app(|a| a.app.text(c));
        }
    }
    let _ = view;
}

extern "C" fn mouse_down(view: Id, _: Sel, event: Id) {
    let (x, y) = point(view, event);
    // The toolbar stands in for the hidden title bar: its empty parts move the window, a double
    // click zooms it.
    let window = msg!(view, c"window" => Id);
    let on_bar = with_app(|a| !a.app.fullscreen && y < crate::view::BAR_H && a.app.viewer.over_chrome_area(x, y) && !a.app.viewer.over_button(x, y));
    if on_bar == Some(true) {
        if msg!(event, c"clickCount" => isize) == 2 {
            msg!(window, c"performZoom:", core::ptr::null_mut::<c_void>());
        } else {
            msg!(window, c"performWindowDragWithEvent:", event);
        }
        return;
    }
    let dbl = msg!(class(c"NSEvent"), c"doubleClickInterval" => f64);
    with_app(|a| a.app.press(x, y, mods(event), Duration::from_secs_f64(dbl)));
}

extern "C" fn mouse_up(view: Id, _: Sel, event: Id) {
    let (x, y) = point(view, event);
    if let Some(e) = with_app(|a| a.app.release(x, y, false)).flatten() {
        apply(e);
    }
}

/// The mouse's back and forward buttons (numbers 3 and 4) walk the history.
extern "C" fn other_mouse_up(_: Id, _: Sel, event: Id) {
    match msg!(event, c"buttonNumber" => isize) {
        3 => _ = with_app(|a| a.app.back()),
        4 => _ = with_app(|a| a.app.forward()),
        _ => {}
    }
}

extern "C" fn mouse_moved(view: Id, _: Sel, event: Id) {
    let (x, y) = point(view, event);
    let cursor = with_app(|a| {
        a.app.mouse_move(x, y, false);
        a.app.cursor()
    });
    let c = match cursor {
        Some(Cursor::Hand) => msg!(class(c"NSCursor"), c"pointingHandCursor" => Id),
        Some(Cursor::Text) => msg!(class(c"NSCursor"), c"IBeamCursor" => Id),
        _ => msg!(class(c"NSCursor"), c"arrowCursor" => Id),
    };
    msg!(c, c"set");
}

extern "C" fn mouse_exited(_: Id, _: Sel, _event: Id) {
    with_app(|a| a.app.mouse_leave());
}

extern "C" fn scroll_wheel(view: Id, _: Sel, event: Id) {
    let (x, y) = point(view, event);
    let dy = msg!(event, c"scrollingDeltaY" => f64) as f32;
    // Trackpad and Magic Mouse: points as they come (momentum included; the system eases
    // them out). A wheel: its (accelerated) lines glide.
    let precise = msg!(event, c"hasPreciseScrollingDeltas" => bool);
    let m = mods(event);
    with_app(|a| if precise { a.app.scroll(-dy, m, x, y) } else { a.app.wheel(-dy, true, m, x, y) });
}

extern "C" fn magnify(_: Id, _: Sel, event: Id) {
    let k = 1.0 + msg!(event, c"magnification" => f64) as f32;
    with_app(|a| {
        a.app.viewer.zoom_by(k);
        a.app.invalidate();
    });
}

extern "C" fn chrome_tick(_: Id, _: Sel, _timer: Id) {
    with_app(|a| a.app.schedule());
}

extern "C" fn layout_tick(_: Id, _: Sel, _timer: Id) {
    with_app(|a| a.app.continue_layout());
}

extern "C" fn watch_tick(_: Id, _: Sel, _timer: Id) {
    with_app(|a| a.app.changed());
}

extern "C" fn appearance_changed(view: Id, _: Sel) {
    msg!(view, c"setNeedsDisplay:", true);
}

// -------------------------------------------------------- delegate methods

extern "C" fn will_close(_: Id, _: Sel, _note: Id) {
    with_app(|a| a.app.save_settings());
    let app = msg!(class(c"NSApplication"), c"sharedApplication" => Id);
    msg!(app, c"terminate:", core::ptr::null_mut::<c_void>());
}

extern "C" fn did_resize(_: Id, _: Sel, _note: Id) {
    with_app(|a| a.app.resize_done());
}

extern "C" fn did_enter_full(_: Id, _: Sel, _note: Id) {
    with_app(|a| {
        a.app.viewer.bar_inset = 0.0; // the window buttons are in the menu bar's strip now
        a.app.set_fullscreen(true);
    });
}

extern "C" fn did_exit_full(_: Id, _: Sel, _note: Id) {
    with_app(|a| {
        a.app.viewer.bar_inset = LIGHTS_INSET;
        a.app.set_fullscreen(false);
    });
}

/// Finder opened a document with us (double click, drop on the Dock icon).
extern "C" fn open_file(_: Id, _: Sel, _app: Id, file: Id) -> bool {
    let path = PathBuf::from(string(file));
    with_app(|a| a.app.open(path)).is_some()
}

/// A file dragged over the window: offer to copy (open) it.
extern "C" fn drag_entered(_: Id, _: Sel, _info: Id) -> usize {
    1 // NSDragOperationCopy
}

/// A file dropped on the window opens.
extern "C" fn perform_drag(_: Id, _: Sel, info: Id) -> bool {
    let pb = msg!(info, c"draggingPasteboard" => Id);
    let classes = msg!(class(c"NSArray"), c"arrayWithObject:", class(c"NSURL") => Id);
    let urls = msg!(pb, c"readObjectsForClasses:options:", classes, core::ptr::null_mut::<c_void>() => Id);
    if urls.is_null() || msg!(urls, c"count" => usize) == 0 {
        return false;
    }
    let path = string(msg!(msg!(urls, c"objectAtIndex:", 0usize => Id), c"path" => Id));
    !path.is_empty() && with_app(|a| a.app.open(PathBuf::from(path))).is_some()
}

/// Image results arrived (main queue, from images.rs).
unsafe extern "C" fn images_ready(_: *mut c_void) {
    with_app(|a| {
        if a.app.gfx.images.collect() {
            a.app.viewer.invalidate();
            a.app.invalidate();
        }
    });
}

/// The view class: an NSView subclass with our methods.
fn view_class() -> Class {
    unsafe {
        let cls = objc_allocateClassPair(class(c"NSView"), c"MarkdownrView".as_ptr(), 0);
        let add = |name: &core::ffi::CStr, imp: *const c_void, types: &core::ffi::CStr| {
            class_addMethod(cls, sel(name), imp, types.as_ptr());
        };
        add(c"isFlipped", yes as *const c_void, c"c@:");
        add(c"acceptsFirstResponder", yes as *const c_void, c"c@:");
        add(c"drawRect:", draw_rect as *const c_void, c"v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add(c"keyDown:", key_down as *const c_void, c"v@:@");
        add(c"mouseDown:", mouse_down as *const c_void, c"v@:@");
        add(c"mouseUp:", mouse_up as *const c_void, c"v@:@");
        add(c"otherMouseUp:", other_mouse_up as *const c_void, c"v@:@");
        add(c"mouseMoved:", mouse_moved as *const c_void, c"v@:@");
        add(c"mouseDragged:", mouse_moved as *const c_void, c"v@:@");
        add(c"mouseExited:", mouse_exited as *const c_void, c"v@:@");
        add(c"scrollWheel:", scroll_wheel as *const c_void, c"v@:@");
        add(c"magnifyWithEvent:", magnify as *const c_void, c"v@:@");
        add(c"chromeTick:", chrome_tick as *const c_void, c"v@:@");
        add(c"frameTick:", chrome_tick as *const c_void, c"v@:@");
        add(c"layoutTick:", layout_tick as *const c_void, c"v@:@");
        add(c"watchTick:", watch_tick as *const c_void, c"v@:@");
        add(c"viewDidChangeEffectiveAppearance", appearance_changed as *const c_void, c"v@:");
        // Window and application delegate in one (the view is both).
        add(c"windowWillClose:", will_close as *const c_void, c"v@:@");
        add(c"windowDidEndLiveResize:", did_resize as *const c_void, c"v@:@");
        add(c"windowDidEnterFullScreen:", did_enter_full as *const c_void, c"v@:@");
        add(c"windowDidExitFullScreen:", did_exit_full as *const c_void, c"v@:@");
        add(c"application:openFile:", open_file as *const c_void, c"c@:@@");
        add(c"draggingEntered:", drag_entered as *const c_void, c"Q@:@");
        add(c"performDragOperation:", perform_drag as *const c_void, c"c@:@");
        objc_registerClassPair(cls);
        cls
    }
}

fn preferred_language() -> String {
    let langs = msg!(class(c"NSLocale"), c"preferredLanguages" => Id);
    let first = if langs.is_null() || msg!(langs, c"count" => usize) == 0 { core::ptr::null_mut() } else { msg!(langs, c"objectAtIndex:", 0usize => Id) };
    let s = string(first);
    if s.starts_with("zh-Hant") || s.starts_with("zh-TW") || s.starts_with("zh-HK") {
        return "en".into();
    }
    s.get(..2).unwrap_or("en").to_ascii_lowercase()
}

fn locale_id() -> Option<String> {
    let l = msg!(class(c"NSLocale"), c"currentLocale" => Id);
    let s = string(msg!(l, c"localeIdentifier" => Id));
    (!s.is_empty()).then(|| s.split('@').next().unwrap_or("").replace('_', "-"))
}

/// ⌘ for Ctrl in the help (the shortcuts use Command on a Mac).
fn key_name(vk: u16, _extended: bool) -> Option<String> {
    match vk {
        0x11 => Some("⌘".into()),
        0x7A => Some("⌃⌘F".into()), // full screen (F11 elsewhere)
        _ => None,
    }
}

pub fn run(path: Option<PathBuf>, sample: &'static str) {
    crate::app::init_locale(preferred_language, locale_id);
    crate::view::set_key_namer(key_name);
    let app = msg!(class(c"NSApplication"), c"sharedApplication" => Id);
    msg!(app, c"setActivationPolicy:", 0isize => bool); // regular app (Dock, menu bar)
    let cls = view_class();
    let rect = CGRect::new(0.0, 0.0, 980.0, 820.0);
    // titled | closable | miniaturizable | resizable | full-size content (the title bar is hidden)
    let window = msg!(msg!(class(c"NSWindow"), c"alloc" => Id), c"initWithContentRect:styleMask:backing:defer:", rect, 15usize | 1 << 15, 2usize, false => Id);
    msg!(window, c"setTitlebarAppearsTransparent:", true);
    msg!(window, c"setTitleVisibility:", 1isize); // hidden
    let view = msg!(msg!(cls, c"alloc" => Id), c"initWithFrame:", rect => Id);
    msg!(window, c"setContentView:", view);
    msg!(window, c"setDelegate:", view);
    msg!(window, c"setAcceptsMouseMovedEvents:", true);
    msg!(window, c"setCollectionBehavior:", 1usize << 7); // full screen primary
    msg!(window, c"center");
    msg!(app, c"setDelegate:", view);
    // Mouse moved / exited inside the view.
    let area = msg!(msg!(class(c"NSTrackingArea"), c"alloc" => Id), c"initWithRect:options:owner:userInfo:", CGRect::default(), 0x01usize | 0x02 | 0x80 | 0x200, view, core::ptr::null_mut::<c_void>() => Id);
    msg!(view, c"addTrackingArea:", area);
    // Files dropped on the window open.
    msg!(view, c"registerForDraggedTypes:", msg!(class(c"NSArray"), c"arrayWithObject:", unsafe { NSPasteboardTypeFileURL } => Id));

    let settings_path = crate::app::settings_path();
    let settings = Settings::load(&settings_path);
    let doc = match &path {
        Some(p) => crate::app::read_doc(p),
        None => crate::app::start_doc(&settings, sample),
    };
    let gfx = Gfx::new(images::Images::new(images_ready));
    let host = MacHost { window, view, timers: [core::ptr::null_mut(); 3], events: core::ptr::null_mut(), link: core::ptr::null_mut() };
    let mut a = App::new(host, gfx, path, doc, settings, settings_path, sample);
    a.viewer.bar_inset = LIGHTS_INSET;
    let start_fullscreen = a.start_fullscreen();
    {
        let cal = msg!(class(c"NSCalendar"), c"currentCalendar" => Id);
        let now = msg!(class(c"NSDate"), c"date" => Id);
        let part = |unit: usize| msg!(cal, c"component:fromDate:", unit, now => isize);
        a.viewer.today = (part(4) as i32, part(8) as u32, part(16) as u32); // year, month, day
        let ws = msg!(class(c"NSWorkspace"), c"sharedWorkspace" => Id);
        a.viewer.reduce_motion = msg!(ws, c"accessibilityDisplayShouldReduceMotion" => bool);
    }
    a.set_title();
    a.resize(980.0, 820.0, false);
    let appearance = appearance_name();
    a.set_dark(appearance.contains("Dark"));
    *APP.0.borrow_mut() = Some(Mac { app: a, appearance });
    with_app(|m| m.app.schedule());
    msg!(window, c"makeKeyAndOrderFront:", core::ptr::null_mut::<c_void>());
    msg!(window, c"makeFirstResponder:", view => bool);
    msg!(app, c"activateIgnoringOtherApps:", true);
    if start_fullscreen {
        msg!(window, c"toggleFullScreen:", core::ptr::null_mut::<c_void>());
    }
    msg!(app, c"run");
}

/// A CGRect getter ([view bounds], [view frame]): a 32-byte struct, returned through memory on
/// x86_64 (objc_msgSend_stret).
fn rect_of(obj: Id, getter: &core::ffi::CStr) -> CGRect {
    #[cfg(target_arch = "x86_64")]
    {
        let f: unsafe extern "C" fn(*mut CGRect, Id, Sel) = unsafe { core::mem::transmute(objc_msgSend_stret as *const c_void) };
        let mut r = CGRect::default();
        unsafe { f(&mut r, obj, sel(getter)) };
        r
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        let f: unsafe extern "C" fn(Id, Sel) -> CGRect = unsafe { core::mem::transmute(objc_msgSend as *const c_void) };
        unsafe { f(obj, sel(getter)) }
    }
}

fn bounds(view: Id) -> CGRect {
    rect_of(view, c"bounds")
}

/// Room the window buttons take left of the toolbar.
const LIGHTS_INSET: f32 = 70.0;

/// The title bar is hidden (full-size content, transparent, no title): the window buttons sit
/// centred in the app's toolbar instead, and fade in and out with it. Their container is
/// stretched to the toolbar's height; AppKit lays it out again on resize, so this runs after
/// every size change.
fn place_window_buttons(window: Id, alpha: f32) {
    let close = msg!(window, c"standardWindowButton:", 0usize => Id);
    if close.is_null() {
        return;
    }
    let container = msg!(msg!(close, c"superview" => Id), c"superview" => Id);
    let bar = crate::view::BAR_H as f64;
    let win_h = rect_of(window, c"frame").size.height;
    let mut c = rect_of(container, c"frame");
    if c.size.height != bar {
        c.size.height = bar;
        c.origin.y = win_h - bar;
        msg!(container, c"setFrame:", c);
    }
    let size = rect_of(close, c"frame").size;
    let step = size.width + 6.0;
    for i in 0..3usize {
        let b = msg!(window, c"standardWindowButton:", i => Id);
        msg!(b, c"setFrameOrigin:", CGPoint { x: 14.0 + i as f64 * step, y: ((bar - size.height) / 2.0).round() });
    }
    msg!(container, c"setAlphaValue:", alpha as f64);
}
