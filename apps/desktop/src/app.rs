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
    /// Once a second: look for changes where the system cannot report them (`Host::watch`).
    Poll,
}

/// What the app needs from the window system.
pub trait Host {
    /// Request a repaint.
    fn invalidate(&self);
    fn set_title(&self, title: &str);
    /// Fire `timer` every `every` until it is set again, or stop it (None).
    fn set_timer(&mut self, timer: Timer, every: Option<Duration>);
    /// The system's open-file dialog, or with `folder` its choose-a-folder dialog.
    fn open_dialog(&mut self, folder: bool) -> Option<PathBuf>;
    /// Hand a web or mail link, or a file that is not Markdown, to the system.
    fn open_external(&mut self, target: &str);
    /// While minimised no animation timers run.
    fn minimized(&self) -> bool {
        false
    }
    /// Report changes in `dirs` (empty: stop) by calling `App::changed`: in each directory
    /// itself, or with `tree` in the whole tree below the first (which the others are part
    /// of, for systems that watch one directory at a time). False when the system cannot;
    /// the app then polls.
    fn watch(&mut self, _dirs: &[PathBuf], _tree: bool) -> bool {
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

/// (title, source, document) of a file, or the index page of a folder; an error page when
/// it cannot be read.
pub fn read_doc(path: &Path) -> (String, String, Node) {
    if crate::util::fs::is_dir(path) {
        return folder_doc(path, &crate::folder::scan(path));
    }
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

fn folder_doc(path: &Path, listing: &crate::folder::Listing) -> (String, String, Node) {
    let name = folder_name(path);
    let text = crate::folder::index(&name, listing);
    let doc = parse(&text);
    (name, text, doc)
}

fn folder_name(path: &Path) -> String {
    let name = crate::util::path::file_name(path).to_string_lossy().into_owned();
    if name.is_empty() { path.to_string_lossy().into_owned() } else { name } // a drive or `/`
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
    /// The file shown, or the folder whose index page shows.
    pub path: Option<PathBuf>,
    modified: Option<u64>,
    /// The folder opened (its index page or one of its files shows).
    pub folder: Option<PathBuf>,
    /// What is being watched: the directories and whether as a tree (`Host::watch`), and
    /// whether the system could not and the app polls instead.
    watched: (Vec<PathBuf>, bool),
    polling: bool,
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
    /// The documents left for others (the last one is the previous), and those gone
    /// back from (the last one is the next).
    back: Vec<Place>,
    ahead: Vec<Place>,
}

/// A document of the history and how far it was scrolled.
struct Place {
    /// None: the start page or, with `sample`, the sample.
    path: Option<PathBuf>,
    sample: bool,
    scroll: f32,
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
        let dir = path.as_deref().is_some_and(crate::util::fs::is_dir);
        gfx.set_image_base(path.as_deref().map(|p| if dir { p.to_path_buf() } else { crate::util::path::parent(p).to_path_buf() }).or_else(crate::util::env::current_dir));
        let modified = path.as_deref().and_then(crate::util::fs::modified);
        let folder = path.clone().filter(|_| dir);
        let mut app = App { host, gfx, viewer, modified, path, folder, watched: (Vec::new(), false), polling: false, chrome_timer: false, sizing_until: None, settings, settings_path, sample, fullscreen: false, mouse: (0.0, 0.0), back: Vec::new(), ahead: Vec::new() };
        app.rewatch(None);
        if dir {
            app.open_first();
        }
        app
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
        if !self.chrome_timer || self.viewer.scroll_moved() || self.viewer.image_fade_waiting() {
            // The layout just made may animate (particles, quote crossfades, an image that
            // arrived fading in), or the view scrolled (the scroll bar shows): arm the timer
            // now rather than at the next input.
            self.schedule();
        }
        let v = &self.viewer;
        let finds = v.find_highlights();
        let sprites = v.sprites(Instant::now());
        let slide = v.slide_x(Instant::now());
        let frame = Frame {
            doc_x: slide.map_or(0.0, |s| s.0),
            view_w: v.view_w,
            leaving: v.slide.as_ref().zip(slide).map(|(s, (_, x))| (&s.layout, &s.theme, s.scroll, x)),
            doc: &v.layout,
            selection: v.selection.as_ref(),
            finds: &finds,
            theme: &v.theme,
            // Whole device pixels: steps of 1/pixel DIP (half a point on a Retina display).
            scroll: (v.scroll * v.pixel).round() / v.pixel,
            view_h: v.view_h,
            scroll_bar: v.scroll_bar(),
            chrome: v.chrome_visible().then_some((&v.chrome, v.chrome_alpha)),
            particles: v.particles.as_ref().map(|p| (p, sprites.as_slice())),
            tip: &v.tip_layout,
            now_ms: (Instant::now() - v.anim_start).as_secs_f32() * 1000.0,
            image_fades: &v.image_fades,
        };
        if !self.gfx.draw(&frame) {
            self.viewer.invalidate();
            self.invalidate();
        }
    }

    /// Where the reader is, for the history.
    fn here(&self) -> Place {
        Place { path: self.path.clone(), sample: self.path.is_none() && self.viewer.source == self.sample, scroll: self.viewer.scroll }
    }

    /// Leaving the current document for another: it goes on the history.
    fn remember(&mut self) {
        let here = self.here();
        self.back.push(here);
        self.ahead.clear();
    }

    /// The previous document of the history (mouse back button), where it was left; false at its start.
    pub fn back(&mut self) -> bool {
        self.step(true)
    }

    /// The next document of the history (mouse forward button); false at its end.
    pub fn forward(&mut self) -> bool {
        self.step(false)
    }

    fn step(&mut self, back: bool) -> bool {
        let Some(to) = (if back { self.back.pop() } else { self.ahead.pop() }) else { return false };
        let here = self.here();
        if back { self.ahead.push(here) } else { self.back.push(here) }
        self.viewer.slide(back, Instant::now());
        match to.path {
            Some(p) => self.show_file(p),
            None if to.sample => self.show_text("Beispiel", self.sample),
            None => {
                let start = self.settings.start_page();
                self.show_text("Markdownr", &start);
            }
        }
        // Laid out from the top, then moved to where the reader left it (the rest of the
        // layout follows as usual; while it is incomplete the scroll is not clamped).
        self.viewer.ensure_layout(&mut self.gfx);
        self.viewer.scroll_by(to.scroll);
        self.schedule();
        true
    }

    /// While a page change of the history slides: (x of the page shown, x of the one leaving).
    pub fn slide_x(&self) -> Option<(f32, f32)> {
        self.viewer.slide_x(Instant::now())
    }

    pub fn load(&mut self, path: PathBuf) {
        let same = self.path.as_ref().is_some_and(|p| p.as_os_str() == path.as_os_str()); // (PathBuf == parses components)
        if !same {
            self.remember();
        }
        self.show_file(path);
    }

    /// Open what the reader chose (dialog, drop, the system): a file, or a folder, which
    /// starts at its first file when that is numbered or the only one (`Listing::first`).
    pub fn open(&mut self, path: PathBuf) {
        self.load(path);
        self.open_first();
    }

    fn open_first(&mut self) {
        let Some(root) = self.folder.clone().filter(|_| self.on_index()) else { return };
        if let Some(first) = crate::folder::scan(&root).first() {
            self.load(crate::util::path::join(&root, Path::new(&first)));
        }
    }

    /// The opened folder's index page shows.
    fn on_index(&self) -> bool {
        matches!((&self.path, &self.folder), (Some(p), Some(f)) if p.as_os_str() == f.as_os_str())
    }

    /// The folder's index page, from one of its files.
    pub fn folder_index(&mut self) {
        if let Some(root) = self.folder.clone() {
            self.load(root);
        }
    }

    fn show_file(&mut self, path: PathBuf) {
        self.show(path, true, None);
    }

    /// Show `path` (a file or a folder's index page), on the recent list with `recent`.
    /// `listing`: of the folder `path`, when just read.
    fn show(&mut self, path: PathBuf, recent: bool, listing: Option<crate::folder::Listing>) {
        let same = self.path.as_ref().is_some_and(|p| p.as_os_str() == path.as_os_str());
        let listing = listing.or_else(|| crate::util::fs::is_dir(&path).then(|| crate::folder::scan(&path)));
        let (title, source, doc) = match &listing {
            Some(l) => folder_doc(&path, l),
            None => read_doc(&path),
        };
        // A folder's own files keep it open; anything else closes it.
        if listing.is_some() {
            self.folder = Some(path.clone());
        } else if !self.folder.as_deref().is_some_and(|f| crate::folder::within(f, &path)) {
            self.folder = None;
        }
        self.gfx.set_image_base(Some(if listing.is_some() { path.clone() } else { crate::util::path::parent(&path).to_path_buf() }));
        self.viewer.set_document(title, source, doc, same);
        self.viewer.folder = self.folder.as_deref().filter(|_| listing.is_none()).map(folder_name);
        self.modified = crate::util::fs::modified(&path);
        // A folder's files are not listed one by one: the folder is.
        if recent && (listing.is_some() || self.folder.is_none()) {
            self.settings.add_recent(&path);
            self.settings.save(&self.settings_path);
        }
        self.path = Some(path);
        self.rewatch(listing);
        self.set_title();
        self.invalidate();
    }

    /// Watch what shows: a folder's whole tree while its index page shows (a file added,
    /// removed or renamed anywhere in it changes the page), otherwise the shown file's
    /// directory (an editor may save by replacing the file); nothing for the start page.
    fn rewatch(&mut self, listing: Option<crate::folder::Listing>) {
        let (dirs, tree) = match (&self.path, listing) {
            (Some(_), Some(l)) => (l.dirs, true),
            (Some(p), None) if crate::util::fs::is_dir(p) => (crate::folder::scan(p).dirs, true),
            (Some(p), None) => (vec![crate::util::path::parent(p).to_path_buf()], false),
            (None, _) => (Vec::new(), false),
        };
        let same = |a: &[PathBuf], b: &[PathBuf]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.as_os_str() == b.as_os_str());
        if tree == self.watched.1 && same(&dirs, &self.watched.0) {
            return;
        }
        let ok = self.host.watch(&dirs, tree);
        let poll = !ok && !dirs.is_empty();
        self.watched = (dirs, tree);
        if poll != self.polling {
            self.polling = poll;
            self.host.set_timer(Timer::Poll, poll.then_some(Duration::from_secs(1)));
        }
    }

    fn load_text(&mut self, title: &str, text: &str) {
        self.remember();
        self.show_text(title, text);
    }

    fn show_text(&mut self, title: &str, text: &str) {
        self.path = None;
        self.modified = None;
        self.folder = None;
        self.viewer.folder = None;
        self.rewatch(None);
        self.gfx.set_image_base(crate::util::env::current_dir());
        self.viewer.set_document(title.to_string(), text.to_string(), parse(text), false);
        self.set_title();
        self.invalidate();
    }

    pub fn reload(&mut self) {
        match self.path.clone() {
            Some(p) => self.show(p, false, None),
            None => {
                let start = self.settings.start_page();
                self.viewer.set_document("Markdownr".into(), start.clone(), parse(&start), true);
            }
        }
        self.invalidate();
    }

    /// Something changed where the app watches (`Host::watch`), or it is time to poll: reload
    /// the file when it was written, the index page when the folder's files are others now.
    pub fn changed(&mut self) {
        let Some(p) = self.path.clone() else { return };
        if crate::util::fs::is_dir(&p) {
            let listing = crate::folder::scan(&p);
            if crate::folder::index(&folder_name(&p), &listing) != self.viewer.source {
                self.show(p, false, Some(listing));
            } else {
                self.rewatch(Some(listing)); // a new directory without Markdown files yet
            }
        } else {
            let now = crate::util::fs::modified(&p);
            if now.is_some() && now != self.modified {
                self.show(p, false, None);
            }
        }
    }

    pub fn open_dialog(&mut self, folder: bool) {
        if let Some(p) = self.host.open_dialog(folder) {
            self.open(p);
        }
    }

    pub fn open_url(&mut self, url: &str) {
        let lower = url.to_ascii_lowercase();
        match lower.as_str() {
            "markdownr:open" => return self.open_dialog(false),
            "markdownr:open-folder" => return self.open_dialog(true),
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
            // From a folder's index page its links are relative to the folder itself.
            let dir = if self.on_index() { doc.as_path() } else { crate::util::path::parent(doc) };
            crate::util::path::join(dir, p)
        } else {
            return;
        };
        if crate::util::fs::is_dir(&target) {
            return self.load(target); // a recent folder from the start page
        }
        if !crate::util::fs::is_file(&target) {
            return;
        }
        if crate::folder::is_markdown(&target) {
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
                self.open_dialog(false);
                None
            }
            Effect::OpenFolderDialog => {
                self.open_dialog(true);
                None
            }
            Effect::FolderIndex => {
                self.folder_index();
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

    /// `notches` of the wheel (positive: towards the user, i.e. down). `glide`: the view
    /// glides there (whole notches of a wheel) rather than jumping.
    pub fn wheel(&mut self, notches: f32, glide: bool, mods: Mods, x: f32, y: f32) {
        let dy = notches * self.viewer.font_size * 3.0;
        if !glide {
            return self.scroll(dy, mods, x, y);
        }
        self.viewer.wheel(dy, mods, x, y, Some(Instant::now()));
        self.schedule();
        self.invalidate();
    }

    /// Scrolled by `dy` DIPs at once (precise trackpad deltas; positive: down).
    pub fn scroll(&mut self, dy: f32, mods: Mods, x: f32, y: f32) {
        self.viewer.wheel(dy, mods, x, y, None);
        self.schedule();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::{Fake, FakePara};

    impl Gfx for Fake {
        fn set_image_base(&mut self, _: Option<PathBuf>) {}
        fn draw(&mut self, _: &Frame<FakePara>) -> bool {
            true
        }
    }

    /// Records what it is asked to watch; `notify`: whether it can.
    #[derive(Default)]
    struct NoHost {
        notify: bool,
        watching: Option<(Vec<PathBuf>, bool)>,
        poll: bool,
    }
    impl Host for NoHost {
        fn invalidate(&self) {}
        fn set_title(&self, _: &str) {}
        fn set_timer(&mut self, t: Timer, every: Option<Duration>) {
            if t == Timer::Poll {
                self.poll = every.is_some();
            }
        }
        fn open_dialog(&mut self, _: bool) -> Option<PathBuf> {
            None
        }
        fn open_external(&mut self, _: &str) {}
        fn watch(&mut self, dirs: &[PathBuf], tree: bool) -> bool {
            self.watching = Some((dirs.to_vec(), tree));
            self.notify
        }
    }

    /// A fresh directory with a.md (long, links to b.md), b.md and c.md.
    fn docs(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(["markdownr-", name, "-", &std::process::id().to_string()].concat());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let long: String = (0..200).map(|i| format!("Absatz {i}\n\n")).collect();
        std::fs::write(dir.join("a.md"), ["# A\n\n[nach B](b.md)\n\n", &long].concat()).unwrap();
        std::fs::write(dir.join("b.md"), "# B\n\n[nach C](c.md)\n").unwrap();
        std::fs::write(dir.join("c.md"), "# C\n").unwrap();
        dir
    }

    fn app(dir: &Path, path: Option<PathBuf>) -> App<Fake, NoHost> {
        let doc = match &path {
            Some(p) => read_doc(p),
            None => start_doc(&Settings::default(), "# Beispiel\n"),
        };
        let mut a = App::new(NoHost { notify: true, ..NoHost::default() }, Fake, path, doc, Settings::default(), dir.join("settings.ini"), "# Beispiel\n");
        a.resize(800.0, 600.0, false);
        a.layout();
        a.settle();
        a
    }

    fn at(a: &App<Fake, NoHost>) -> Option<String> {
        a.path.as_deref().map(|p| crate::util::path::file_name(p).to_string_lossy().into_owned())
    }

    #[test]
    fn back_and_forward_walk_the_followed_links() {
        let dir = docs("history");
        let mut a = app(&dir, Some(dir.join("a.md")));
        assert!(!a.back(), "nothing to go back to yet");
        a.viewer.scroll_by(700.0);
        let read_to = a.viewer.scroll;
        assert!(read_to > 0.0);

        a.open_url("b.md");
        assert_eq!(at(&a).as_deref(), Some("b.md"));
        assert_eq!(a.viewer.scroll, 0.0);

        assert!(a.slide_x().is_none(), "following a link does not slide");
        assert!(a.back());
        assert!(a.slide_x().is_some_and(|(x, _)| x < 0.0), "the previous page slides in from the left");
        assert_eq!(at(&a).as_deref(), Some("a.md"));
        a.layout();
        assert_eq!(a.viewer.scroll, read_to, "back returns to where the reader was");
        assert!(!a.back(), "the start of the history");

        assert!(a.forward());
        assert!(a.slide_x().is_some_and(|(x, _)| x > 0.0), "the next page slides in from the right");
        assert_eq!(at(&a).as_deref(), Some("b.md"));
        assert!(!a.forward(), "the end of the history");

        // Following a link after going back drops what lay ahead.
        assert!(a.back());
        a.open_url("b.md");
        a.open_url("c.md");
        assert!(!a.forward());
        assert!(a.back());
        assert_eq!(at(&a).as_deref(), Some("b.md"));
        assert!(a.back());
        assert_eq!(at(&a).as_deref(), Some("a.md"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reload_and_the_same_file_add_no_history() {
        let dir = docs("same");
        let mut a = app(&dir, Some(dir.join("a.md")));
        a.reload();
        a.open_url("a.md");
        assert!(!a.back());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder with a.md, b.md and sub/c.md (linking back up to a.md).
    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(["markdownr-folder-", name, "-", &std::process::id().to_string()].concat());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "# A\n").unwrap();
        std::fs::write(dir.join("b.md"), "# B\n").unwrap();
        std::fs::write(dir.join("sub").join("c.md"), "# C\n\n[up](../a.md)\n").unwrap();
        dir
    }

    #[test]
    fn a_folder_shows_its_index_and_its_files_lead_back() {
        let dir = folder("index");
        let mut a = app(&dir, None);
        a.open(dir.clone());
        assert_eq!(a.path.as_ref(), Some(&dir), "a.md and b.md are not numbered: the index page");
        assert_eq!(a.viewer.source, "# markdownr\\-folder\\-index\\-".to_string() + &std::process::id().to_string() + "\n\n- [a](<a.md>)\n- [b](<b.md>)\n\n## sub\n\n- [c](<sub/c.md>)\n\n");
        assert_eq!(a.viewer.folder, None, "no way back to the index on the index");
        assert_eq!(a.host.watching, Some((vec![dir.clone(), dir.join("sub")], true)), "the whole tree while the index shows");

        a.open_url("sub/c.md");
        assert_eq!(at(&a).as_deref(), Some("c.md"));
        assert!(a.viewer.folder.as_deref().is_some_and(|f| f.starts_with("markdownr-folder-index")));
        assert_eq!(a.host.watching, Some((vec![dir.join("sub")], false)), "only the file's directory once a file shows");
        a.open_url("../a.md");
        assert_eq!(at(&a).as_deref(), Some("a.md"));
        assert!(a.viewer.folder.is_some(), "still in the folder");

        a.folder_index();
        assert_eq!(a.path.as_ref(), Some(&dir));
        assert_eq!(a.host.watching.as_ref().map(|w| w.1), Some(true));
        assert!(a.back());
        assert_eq!(at(&a).as_deref(), Some("a.md"));

        // Only the folder goes on the recent list, not each of its files.
        assert_eq!(a.settings.recent.len(), 1);
        assert!(a.settings.start_page().contains("markdownr-folder-index"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_numbered_folder_starts_at_its_first_file() {
        let dir = folder("numbered");
        std::fs::write(dir.join("01 Intro.md"), "# Intro\n").unwrap();
        let mut a = app(&dir, Some(dir.clone()));
        assert_eq!(at(&a).as_deref(), Some("01 Intro.md"));
        assert!(a.back(), "the index page is on the history");
        assert_eq!(a.path.as_ref(), Some(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_index_follows_the_folder_and_a_file_its_changes() {
        let dir = folder("changes");
        let mut a = app(&dir, None);
        a.open(dir.clone());
        std::fs::create_dir_all(dir.join("new")).unwrap();
        a.changed();
        assert!(!a.viewer.source.contains("new"), "a directory without Markdown files is not listed");
        assert_eq!(a.host.watching.as_ref().map(|w| w.0.len()), Some(3), "but watched");
        std::fs::write(dir.join("new").join("d.md"), "# D\n").unwrap();
        a.changed();
        assert!(a.viewer.source.contains("[d](<new/d.md>)"));
        std::fs::remove_file(dir.join("b.md")).unwrap();
        a.changed();
        assert!(!a.viewer.source.contains("b.md"));
        assert_eq!(a.back.len(), 1, "only the start page before: a refreshed index is no new page");

        a.open_url("a.md");
        std::thread::sleep(Duration::from_millis(20));
        std::fs::write(dir.join("a.md"), "# A2\n").unwrap();
        a.changed();
        assert_eq!(a.viewer.source, "# A2\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_notifications_the_app_polls() {
        let dir = folder("poll");
        let doc = start_doc(&Settings::default(), "");
        let mut a = App::new(NoHost::default(), Fake, None, doc, Settings::default(), dir.join("settings.ini"), "");
        assert!(!a.host.poll, "nothing to watch on the start page");
        a.open(dir.join("a.md"));
        assert!(a.host.poll);
        a.open_url("markdownr:sample");
        assert!(!a.host.poll);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn back_returns_to_the_start_page() {
        let dir = docs("start");
        let mut a = app(&dir, None);
        a.open_url(&dir.join("b.md").to_string_lossy());
        assert_eq!(at(&a).as_deref(), Some("b.md"));
        assert!(a.back());
        assert_eq!(a.path, None);
        assert_eq!(a.viewer.title, "Markdownr");
        assert!(a.forward());
        assert_eq!(at(&a).as_deref(), Some("b.md"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
