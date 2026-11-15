//! The platform-neutral app: loading and reloading documents, links, settings, the
//! timers' logic, input routing and frame preparation. A platform shell (`win`,
//! `linux`, `mac`) owns the window and the event loop; it implements [`Host`] for
//! what only the window system can do and [`Gfx`] for text and drawing, and forwards
//! its events to the methods here.

use crate::ast::Node;
use crate::layout::Typesetter;
use crate::paint::Frame;
use crate::settings::Settings;
use crate::theme::ThemeChoice;
use crate::view::{Effect, Key, Mods, Viewer};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Text layout and drawing of a platform.
pub trait Gfx: Typesetter {
    /// The directory relative image paths resolve against.
    fn set_image_base(&mut self, dir: Option<PathBuf>);
    /// Paint a frame; false when the device was lost and everything must be redone.
    fn draw(&mut self, frame: &Frame<Self::Para>) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timer {
    /// Chrome fades, particles and other animation.
    Chrome,
    /// Idle-time slices of a partial document layout.
    Layout,
}

/// What the app needs from the window system.
pub trait Host {
    /// Request a repaint.
    fn invalidate(&self);
    fn set_title(&self, title: &str);
    /// Fire `timer` every `every` until it is set again, or stop it (None).
    fn set_timer(&mut self, timer: Timer, every: Option<Duration>);
    /// The system's open-file dialog.
    fn open_dialog(&mut self) -> Option<PathBuf>;
    /// Hand a web or mail link, or a file that is not Markdown, to the system.
    fn open_external(&mut self, target: &str);
    /// While minimised no animation timers run.
    fn minimized(&self) -> bool {
        false
    }
}

/// Mouse cursor to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cursor {
    Arrow,
    Hand,
    Text,
}

pub fn parse(text: &str) -> Node {
    let mut doc = crate::ast::parse(&crate::transforms::preprocess_html(text));
    crate::transforms::compose(&mut doc, &crate::transforms::default_pipeline());
    doc
}

/// (title, source, document) of a file; an error page when it cannot be read.
pub fn read_doc(path: &Path) -> (String, String, Node) {
    let text = match crate::util::fs::read(path) {
        Some(bytes) => {
            let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
            String::from_utf8(bytes.to_vec()).unwrap_or_else(|e| e.into_bytes().iter().map(|&b| b as char).collect())
        }
        None => ["# Datei kann nicht geöffnet werden\n\n`", &path.to_string_lossy(), "`"].concat(),
    };
    let doc = parse(&text);
    (crate::util::path::file_name(path).to_string_lossy().into_owned(), text, doc)
}

/// The document to show without a file: the sample (MARKDOWNR_SAMPLE) or the start page.
pub fn start_doc(settings: &Settings, sample: &str) -> (String, String, Node) {
    if crate::util::env::var("MARKDOWNR_SAMPLE").is_some() {
        ("Beispiel".into(), sample.to_string(), parse(sample))
    } else {
        let start = settings.start_page();
        ("Markdownr".into(), start.clone(), parse(&start))
    }
}

/// Help language and quote style: MARKDOWNR_LANGUAGE / MARKDOWNR_LOCALE, else the system's.
pub fn init_locale(ui_language: impl FnOnce() -> String, locale: impl FnOnce() -> Option<String>) {
    crate::view::set_language(&crate::util::env::string("MARKDOWNR_LANGUAGE").unwrap_or_else(ui_language));
    if let Some(l) = crate::util::env::string("MARKDOWNR_LOCALE").or_else(locale) {
        crate::transforms::set_locale(&l);
    }
}

/// The settings file: MARKDOWNR_SETTINGS, else the platform's place for it.
pub fn settings_path() -> PathBuf {
    crate::util::env::var("MARKDOWNR_SETTINGS").map(PathBuf::from).unwrap_or_else(crate::settings::default_path)
}

pub struct App<G: Gfx, H: Host> {
    pub host: H,
    pub gfx: G,
    pub viewer: Viewer<G::Para>,
    pub path: Option<PathBuf>,
    modified: Option<u64>,
    chrome_timer: bool,
    /// While the window is being resized (until 150 ms after the last size change or the
    /// end of the drag) the idle layout waits: each size step would discard it.
    pub sizing_until: Option<Instant>,
    pub settings: Settings,
    settings_path: PathBuf,
    sample: &'static str,
    /// The window is in full screen (remembered for the next start when wanted).
    pub fullscreen: bool,
    /// Last mouse position (DIPs), for the cursor shape.
    pub mouse: (f32, f32),
}

impl<G: Gfx, H: Host> App<G, H> {
    /// The app showing `doc` (from `path`, or the start page / sample without one).
    pub fn new(host: H, mut gfx: G, path: Option<PathBuf>, doc: (String, String, Node), mut settings: Settings, settings_path: PathBuf, sample: &'static str) -> Self {
        if let Some(p) = &path {
            settings.add_recent(p);
            settings.save(&settings_path);
        }
        let (title, source, doc) = doc;
        let mut viewer = Viewer::new(title, source, doc);
        viewer.font_size = settings.font_size;
        viewer.theme_choice = ThemeChoice::from_name(&settings.theme);
        viewer.remember_fullscreen = settings.remember_fullscreen;
        if let Some(ms) = crate::util::env::string("MARKDOWNR_TIME_MS").and_then(|s| s.parse::<u64>().ok()) {
            viewer.anim_start = Instant::now() - Duration::from_millis(ms);
        }
        gfx.set_image_base(path.as_deref().map(|p| crate::util::path::parent(p).to_path_buf()).or_else(crate::util::env::current_dir));
        App { host, gfx, viewer, modified: path.as_deref().and_then(crate::util::fs::modified), path, chrome_timer: false, sizing_until: None, settings, settings_path, sample, fullscreen: false, mouse: (0.0, 0.0) }
    }

    /// Start in full screen: the last session ended there and that is to be remembered.
    pub fn start_fullscreen(&self) -> bool {
        self.settings.remember_fullscreen && self.settings.fullscreen
    }

    pub fn invalidate(&self) {
        self.host.invalidate();
    }

    /// Complete a partial layout before input that may need all of it (selection
    /// end, links and anchors, find, TOC, copy). Scrolling extends it as needed.
    pub fn settle(&mut self) {
        self.viewer.finish_layout(&mut self.gfx);
    }

    /// Arm the idle layout: at once, or once a resize has come to rest.
    pub fn arm_layout(&mut self) {
        let wait = self.sizing_until.map_or(Duration::ZERO, |t| t.saturating_duration_since(Instant::now()));
        if wait.is_zero() {
            self.sizing_until = None;
        }
        self.host.set_timer(Timer::Layout, Some(wait.max(Duration::from_millis(1))));
    }

    /// One idle slice of a partial layout; repaint once it is complete.
    pub fn continue_layout(&mut self) {
        if self.sizing_until.is_some_and(|t| t > Instant::now()) {
            return self.arm_layout();
        }
        if self.viewer.continue_layout(&mut self.gfx, Duration::from_millis(30)) || !self.viewer.layout_pending() {
            self.host.set_timer(Timer::Layout, None);
            self.invalidate();
        }
    }

    /// The window was resized (or is being resized: `live`), in DIPs.
    pub fn resize(&mut self, w: f32, h: f32, live: bool) {
        if live {
            self.sizing_until = Some(Instant::now() + Duration::from_millis(150));
        }
        self.viewer.resize(w, h);
        self.invalidate();
    }

    /// The interactive resize ended.
    pub fn resize_done(&mut self) {
        self.sizing_until = None;
        if self.viewer.layout_pending() {
            self.arm_layout();
        }
    }

    /// "First heading | Markdownr", or just "Markdownr" when the document has no heading.
    pub fn set_title(&self) {
        let title = match self.viewer.first_heading() {
            Some(h) => [h.as_str(), " | Markdownr"].concat(),
            None => "Markdownr".to_string(),
        };
        self.host.set_title(&title);
    }

    /// Drive the chrome fade and other animation: (re)arm or stop the timer.
    pub fn schedule(&mut self) {
        let (repaint, next) = self.viewer.tick(Instant::now());
        let next = if self.host.minimized() { None } else { next };
        if repaint {
            self.invalidate();
        }
        match next {
            Some(d) => {
                self.host.set_timer(Timer::Chrome, Some(d.clamp(Duration::from_millis(10), Duration::from_secs(60))));
                self.chrome_timer = true;
            }
            None if self.chrome_timer => {
                self.host.set_timer(Timer::Chrome, None);
                self.chrome_timer = false;
            }
            None => {}
        }
    }

    /// Lay out what the next frame shows (before the shell's own per-frame work).
    pub fn layout(&mut self) {
        self.viewer.ensure_layout(&mut self.gfx);
        if self.viewer.layout_pending() {
            self.arm_layout();
        }
    }

    /// Paint the frame laid out by `layout`.
    pub fn draw(&mut self) {
        if !self.chrome_timer {
            // The layout just made may animate (particles, quote crossfades): arm the
            // timer now rather than at the next input.
            self.schedule();
        }
        let v = &self.viewer;
        let finds = v.find_highlights();
        let sprites = v.sprites(Instant::now());
        let frame = Frame {
            doc: &v.layout,
            selection: v.selection.as_ref(),
            finds: &finds,
            theme: &v.theme,
            scroll: v.scroll,
            view_h: v.view_h,
            chrome: v.chrome_visible().then_some((&v.chrome, v.chrome_alpha)),
            particles: v.particles.as_ref().map(|p| (p, sprites.as_slice())),
            tip: &v.tip_layout,
            now_ms: (Instant::now() - v.anim_start).as_secs_f32() * 1000.0,
        };
        if !self.gfx.draw(&frame) {
            self.viewer.invalidate();
            self.invalidate();
        }
    }

    pub fn load(&mut self, path: PathBuf) {
        let (title, source, doc) = read_doc(&path);
        let same = self.path.as_ref().is_some_and(|p| p.as_os_str() == path.as_os_str()); // (PathBuf == parses components)
        self.gfx.set_image_base(Some(crate::util::path::parent(&path).to_path_buf()));
        self.viewer.set_document(title, source, doc, same);
        self.modified = crate::util::fs::modified(&path);
        self.settings.add_recent(&path);
        self.settings.save(&self.settings_path);
        self.path = Some(path);
        self.set_title();
        self.invalidate();
    }

    fn load_text(&mut self, title: &str, text: &str) {
        self.path = None;
        self.modified = None;
        self.gfx.set_image_base(crate::util::env::current_dir());
        self.viewer.set_document(title.to_string(), text.to_string(), parse(text), false);
        self.set_title();
        self.invalidate();
    }

    pub fn reload(&mut self) {
        match self.path.clone() {
            Some(p) => {
                let (title, source, doc) = read_doc(&p);
                self.viewer.set_document(title, source, doc, true);
                self.modified = crate::util::fs::modified(&p);
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
    pub fn watch(&mut self) {
        if let Some(p) = &self.path {
            let now = crate::util::fs::modified(p);
            if now.is_some() && now != self.modified {
                self.reload();
            }
        }
    }

    pub fn open_dialog(&mut self) {
        if let Some(p) = self.host.open_dialog() {
            self.load(p);
        }
    }

    pub fn open_url(&mut self, url: &str) {
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
            return self.host.open_external(url);
        }
        let file = crate::util::percent_decode(url.split('#').next().unwrap_or(""));
        let file = file.strip_prefix("file://").map_or(file.as_str(), |f| {
            // file:///C:/x (Windows) and file:///home/x (elsewhere)
            if cfg!(windows) { f.strip_prefix('/').unwrap_or(f) } else { f }
        });
        let p = Path::new(file);
        let target = if crate::util::path::is_absolute(p) {
            p.to_path_buf()
        } else if let Some(doc) = &self.path {
            crate::util::path::join(crate::util::path::parent(doc), p)
        } else {
            return;
        };
        if !crate::util::fs::is_file(&target) {
            return;
        }
        let ext = crate::util::path::extension(&target);
        if matches!(ext.as_str(), "md" | "markdown" | "mdown" | "mkd" | "txt") {
            self.load(target);
        } else {
            self.host.open_external(&target.to_string_lossy());
        }
    }

    pub fn save_settings(&mut self) {
        self.settings.font_size = self.viewer.font_size;
        self.settings.theme = self.viewer.theme_choice.name().to_string();
        self.settings.remember_fullscreen = self.viewer.remember_fullscreen;
        self.settings.fullscreen = self.viewer.remember_fullscreen && self.fullscreen;
        self.settings.save(&self.settings_path);
    }

    /// Carry out what the app can do itself; the window-system effects (copy, full
    /// screen, quit) come back for the shell.
    pub fn effect(&mut self, e: Effect) -> Option<Effect> {
        let rest = match e {
            Effect::OpenUrl(url) => {
                self.open_url(&url);
                None
            }
            Effect::OpenDialog => {
                self.open_dialog();
                None
            }
            Effect::Reload => {
                self.reload();
                None
            }
            other => Some(other),
        };
        self.schedule();
        self.invalidate();
        rest
    }

    // ------------------------------------------------------------------ input

    pub fn key(&mut self, key: Key, mods: Mods) -> Option<Effect> {
        if !matches!(key, Key::PageDown | Key::PageUp | Key::Space | Key::Down | Key::Up | Key::Left | Key::Right | Key::Home | Key::End) {
            self.settle();
        }
        let e = self.viewer.key(key, mods, Instant::now());
        self.schedule();
        self.invalidate();
        e
    }

    pub fn text(&mut self, c: char) {
        self.settle();
        if self.viewer.text_input(c) {
            self.invalidate();
        }
    }

    /// `notches` of the wheel (positive: towards the user, i.e. down).
    pub fn wheel(&mut self, notches: f32, mods: Mods, x: f32, y: f32) {
        let step = self.viewer.font_size * 3.0;
        self.scroll(notches * step, mods, x, y);
    }

    /// Scrolled by `dy` DIPs (precise trackpad deltas; positive: down).
    pub fn scroll(&mut self, dy: f32, mods: Mods, x: f32, y: f32) {
        self.viewer.wheel(dy, mods, x, y);
        self.invalidate();
    }

    pub fn press(&mut self, x: f32, y: f32, mods: Mods, double_click: Duration) {
        self.viewer.press(x, y, mods, double_click);
        self.invalidate();
    }

    /// `tap`: touch input (a tap toggles the chrome).
    pub fn release(&mut self, x: f32, y: f32, tap: bool) -> Option<Effect> {
        self.settle();
        let e = self.viewer.release(x, y, tap, Instant::now());
        self.schedule();
        self.invalidate();
        e
    }

    /// `touch`: synthesised from touch input (no hover effects).
    pub fn mouse_move(&mut self, x: f32, y: f32, touch: bool) {
        self.mouse = (x, y);
        if !touch && self.viewer.mouse_move(x, y, Instant::now()) {
            self.invalidate();
        }
        self.schedule();
    }

    pub fn mouse_leave(&mut self) {
        self.viewer.mouse_leave();
        self.schedule();
        self.invalidate();
    }

    pub fn cursor(&self) -> Cursor {
        let (x, y) = self.mouse;
        if self.viewer.over_button(x, y) || self.viewer.over_link(x, y) {
            Cursor::Hand
        } else if self.viewer.over_chrome_area(x, y) {
            Cursor::Arrow
        } else {
            Cursor::Text
        }
    }

    /// The system's light/dark appearance changed (or is set at start).
    pub fn set_dark(&mut self, dark: bool) {
        self.viewer.set_dark(dark);
        self.invalidate();
    }

    pub fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
        self.viewer.set_fullscreen(on);
    }
}
