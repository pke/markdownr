//! Platform-neutral viewer state and input handling. Platform shells translate
//! native events into calls here and carry out the returned [`Effect`]s.
//!
//! Besides the document this owns the reading-mode chrome: a toolbar that fades
//! in on mouse movement at the top edge (or a tap) and out again, the table of
//! contents drawer and the find bar. The chrome is laid out with the same
//! typesetter as the document, in window coordinates.

use crate::ast::{Kind, Node};
use crate::layout::{BaseStyle, DocLayout, DocPos, DocSelection, Join, Layouter, Paragraph, Rect, RectF, Resume, TextBlock, Typesetter};
use crate::paint::ImageFade;
use crate::particles::{Particles, Sprite};
use crate::util::num;
use crate::theme::{Ink, Theme, ThemeChoice, front_matter_pairs, seasonal_theme};
use std::ops::Range;
use std::time::{Duration, Instant};

pub const DEFAULT_FONT_SIZE: f32 = 16.0;
const MAX_COLUMN: f32 = 46.0; // in multiples of the font size
pub const BAR_H: f32 = 44.0;
const UI_SIZE: f32 = 14.0;
const SHOW_FOR: Duration = Duration::from_millis(2500);
const FADE_IN: f32 = 0.12; // seconds
const FADE_OUT: f32 = 0.35;
/// Animation frames while something moves (a display link may pace them instead).
pub const FRAME: Duration = Duration::from_millis(16);
/// Wheel notches glide to their target in this time (ease-out).
const GLIDE: f32 = 0.15; // seconds
/// The overlay scroll bar stays this long after the last scroll, then fades out.
const BAR_FOR: Duration = Duration::from_millis(800);
const BAR_FADE_IN: f32 = 0.1; // seconds
/// An image that loaded fades in over this time as it first shows (ease-out).
const IMAGE_FADE: f32 = 0.25; // seconds
const BAR_FADE_OUT: f32 = 0.3;
/// A page change of the history: both pages slide sideways, speeding up then slowing down.
const SLIDE: f32 = 0.35; // seconds

/// F1 overlay: (column, advanced, [keys of each row]); advanced sections show only after
/// "Advanced usage" is opened. In the keys, `{Ctrl}`, `{PgDn}` … are keys the system names
/// in its keyboard's language (`set_key_namer`) and `{#0}` (mouse wheel), `{#1}` (file) words
/// of the help texts. The texts — title, hint, the two words, then each section's heading and
/// row descriptions in this order, then the interface texts (`Ui`) — are
/// `tables::help(language)`.
const HELP: &[(usize, bool, &[&str])] = &[
    (0, false, &["{PgDn}   →   {Space}", "{PgUp}   ←   ⇧ {Space}", "↓   ↑   {#0}", "{Home}   {End}", "{Ctrl}+A   {Ctrl}+C"]),
    (0, false, &["{Ctrl}+F   F3", "{Enter}   ⇧ {Enter}", "{Esc}"]),
    (0, false, &["{Ctrl}+O", "F5   {Ctrl}+R", "{Ctrl}+Q   {Ctrl}+W"]),
    (0, true, &["markdownr {#1}.md", "markdownr"]),
    (1, false, &["{Ctrl}+T", "{Ctrl}+U", "{Ctrl}+D", "{Ctrl}++   {Ctrl}+−", "{Ctrl}+{#0}", "{Ctrl}+0", "{Full}   {Esc}", "F1"]),
    (1, true, &["MARKDOWNR_SETTINGS={#1}", "MARKDOWNR_SAMPLE=1", "MARKDOWNR_LOCALE=de-DE", "MARKDOWNR_LANGUAGE=de"]),
];

/// Interface texts, in the order they follow the help texts in each language's block.
#[derive(Clone, Copy)]
pub enum Ui {
    Advanced,
    Contents,
    Open,
    Window,
    FullScreen,
    OnStart,
    Search,
    NoMatches,
    Source,
    Seasonal,
    NoHeadings,
    Tagline,
    OpenFile,
    Sample,
    StartKeys,
    Recent,
}
const UI_COUNT: usize = Ui::Recent as usize + 1;

/// The language's help and interface texts, line by line (`tables::help`).
fn texts() -> &'static [&'static str] {
    static LINES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| crate::tables::help(LANGUAGE.get().map_or("en", String::as_str)).lines().collect())
}

/// An interface text in the help's language.
pub fn ui(t: Ui) -> &'static str {
    let all = texts();
    all.get((all.len() + t as usize).wrapping_sub(UI_COUNT)).copied().unwrap_or("")
}

/// Keys of the help by name: (name in HELP, virtual-key code, extended key).
const HELP_KEYS: [(&str, u16, bool); 9] =
    [("Ctrl", 0x11, false), ("Esc", 0x1B, false), ("PgDn", 0x22, true), ("PgUp", 0x21, true), ("Home", 0x24, true), ("End", 0x23, true), ("Space", 0x20, false), ("Enter", 0x0D, false), ("Full", 0x7A, false)];

static LANGUAGE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static KEY_NAMER: std::sync::OnceLock<fn(u16, bool) -> Option<String>> = std::sync::OnceLock::new();

/// The help's language (two-letter code, e.g. from Windows' display language); set once at
/// startup. Unset or unknown: English.
pub fn set_language(code: &str) {
    let _ = LANGUAGE.set(code.get(..2).unwrap_or(code).to_ascii_lowercase());
}

/// How the platform names a key (virtual-key code, extended) in its keyboard's language;
/// set once at startup. Unset or None: English names.
pub fn set_key_namer(f: fn(u16, bool) -> Option<String>) {
    let _ = KEY_NAMER.set(f);
}

/// A HELP key column entry (or a text with keys) with its `{Key}` and `{#n}` parts filled in.
pub fn help_keys(spec: &str, words: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = spec;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else { break };
        let name = &rest[open + 1..open + close];
        if let Some(n) = name.strip_prefix('#').and_then(|n| n.parse::<usize>().ok()) {
            out.push_str(words.get(n).copied().unwrap_or(""));
        } else if let Some(&(_, vk, ext)) = HELP_KEYS.iter().find(|k| k.0 == name) {
            match KEY_NAMER.get().and_then(|f| f(vk, ext)) {
                Some(s) => out.push_str(&s),
                None => out.push_str(match name { "PgDn" => "Page Down", "PgUp" => "Page Up", "Full" => "F11", _ => name }),
            }
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    PageDown,
    PageUp,
    Space,
    Down,
    Up,
    Left,
    Right,
    Home,
    End,
    F1,
    F3,
    F5,
    F11,
    Escape,
    Enter,
    Backspace,
    Char(char),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Copy(String),
    ToggleFullscreen,
    ExitFullscreen,
    OpenUrl(String),
    OpenDialog,
    Reload,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Toc,
    Open,
    Search,
    FontDown,
    FontUp,
    Source,
    Theme,
    Fullscreen,
    RememberFullscreen,
    TocEntry(usize),
    Seasonal,
    FindPrev,
    FindNext,
    FindClose,
}

/// A tooltip over a link or abbreviation, anchored to the line it explains
/// (window coordinates).
struct Tip {
    text: String,
    x: f32,
    top: f32,
    bottom: f32,
    /// The pointer is on the tooltip (its copy button shows).
    hot: bool,
    on_copy: bool,
    copied: bool,
}

impl Tip {
    fn new(text: String, x: f32, top: f32, bottom: f32) -> Self {
        Tip { text, x, top, bottom, hot: false, on_copy: false, copied: false }
    }
}

struct Find {
    query: String,
    matches: Vec<(usize, Range<usize>)>,
    current: usize,
}

pub struct Viewer<P> {
    pub doc: Node,
    pub source: String,
    pub title: String,
    pub theme: Theme,
    pub dark: bool,
    pub theme_choice: ThemeChoice,
    pub font_size: f32,
    pub layout: DocLayout<P>,
    pub scroll: f32,
    /// Device pixels per DIP: the document is drawn at whole device pixels.
    pub pixel: f32,
    pub view_w: f32,
    pub view_h: f32,
    pub selection: Option<DocSelection>,
    pub fullscreen: bool,
    pub remember_fullscreen: bool,
    pub show_source: bool,
    /// Toolbar, drawer and find bar in window coordinates.
    pub chrome: DocLayout<P>,
    pub chrome_alpha: f32,
    /// Room left of the toolbar's buttons (macOS: the window buttons sit there).
    pub bar_inset: f32,
    /// Link or abbreviation tooltip and its layout.
    tip: Option<Tip>,
    pub tip_layout: DocLayout<P>,
    /// The tooltip's box as drawn, its copy button (while pointed at) and the
    /// box plus the gap to the link, where the pointer keeps it open.
    tip_box: RectF,
    tip_copy: RectF,
    tip_zone: RectF,
    pressed_tip: bool,
    pub particles: Option<Particles>,
    /// Bumped whenever `particles` is made anew.
    pub particles_gen: u32,
    /// The shell animates the particles itself (a compositor overlay): no particle
    /// frames and no sprites to draw.
    pub composited: bool,
    pub anim_start: Instant,
    /// System "show animations" off: no particles.
    pub reduce_motion: bool,
    /// Local date (y, m, d) and hemisphere for the seasonal theme suggestion.
    pub today: (i32, u32, u32),
    pub southern: bool,
    front_matter: Vec<(String, String)>,
    source_doc: Option<Node>,
    buttons: Vec<(RectF, Action)>,
    hover: Option<Action>,
    chrome_dirty: bool,
    show_until: Option<Instant>,
    last_tick: Option<Instant>,
    toc_open: bool,
    toc_scroll: f32,
    /// F1 overlay with the keyboard shortcuts.
    pub help: bool,
    /// The help shows its advanced sections (command line, environment variables).
    help_advanced: bool,
    /// Where "Advanced usage" is in the help (window coordinates).
    help_reveal: RectF,
    find: Option<Find>,
    dirty: bool,
    dragging: bool,
    press: (f32, f32),
    pressed_chrome: bool,
    last_click: Option<(Instant, (f32, f32), u8)>,
    pub last_layout_time: Duration,
    /// Where a partial document layout continues (None when complete).
    resume: Option<Resume>,
    /// Exact layout from the top being built behind a provisional one (after a relayout
    /// while scrolled down).
    rebuild: Option<Rebuild<P>>,
    /// A wheel glide in progress.
    glide: Option<Glide>,
    /// Overlay scroll bar: opacity, shown until, the scroll position it last saw.
    pub bar_alpha: f32,
    bar_until: Option<Instant>,
    bar_seen: f32,
    /// A page change of the history sliding sideways: the page leaving.
    pub slide: Option<Slide<P>>,
    /// Images asked for (near the view) and not placed yet: they fade in once they are.
    asked: Vec<String>,
    /// Images that arrived, fading in.
    pub image_fades: Vec<ImageFade>,
}

/// The page leaving as it was drawn, sliding out while the new one slides in from the
/// side `dir` (-1: from the left, going back; 1: from the right, going forward).
pub struct Slide<P> {
    pub layout: DocLayout<P>,
    pub theme: Theme,
    pub scroll: f32,
    dir: f32,
    start: Instant,
}

/// Scrolling from `from` to `to` (ease-out over GLIDE from `start`); `at` is where it
/// put the view last (anything else moving the view ends it).
#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    to: f32,
    start: Instant,
    at: f32,
}

/// The exact layout from the top, built in idle time behind a provisional layout that
/// starts at top-level node `node` placed at `y0` (where it was before the relayout).
struct Rebuild<P> {
    layout: DocLayout<P>,
    at: Resume,
    node: usize,
    y0: f32,
}

fn front_matter_of(doc: &Node) -> Vec<(String, String)> {
    doc.children.iter().find(|n| n.kind == Kind::FrontMatter).map(|n| front_matter_pairs(&n.text)).unwrap_or_default()
}

/// Hover text for a link target: an abbreviation's expansion, a link's URL
/// (footnote references show none).
fn tip_text(url: &str) -> Option<&str> {
    match url.strip_prefix("abbr:") {
        Some(title) => Some(title),
        None if url.starts_with("#fn-") => None,
        None => Some(url),
    }
}

fn contains(r: &RectF, x: f32, y: f32) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

impl<P: Paragraph> Viewer<P> {
    pub fn new(title: String, source: String, doc: Node) -> Self {
        let mut v = Viewer {
            front_matter: front_matter_of(&doc),
            doc,
            source,
            title: String::new(),
            theme: Theme::light(),
            dark: false,
            theme_choice: ThemeChoice::default(),
            font_size: DEFAULT_FONT_SIZE,
            layout: DocLayout::default(),
            scroll: 0.0,
            pixel: 1.0,
            view_w: 800.0,
            view_h: 600.0,
            selection: None,
            fullscreen: false,
            remember_fullscreen: false,
            show_source: false,
            chrome: DocLayout::default(),
            chrome_alpha: 0.0,
            bar_inset: 0.0,
            tip: None,
            tip_layout: DocLayout::default(),
            tip_box: RectF::default(),
            tip_copy: RectF::default(),
            tip_zone: RectF::default(),
            pressed_tip: false,
            particles: None,
            particles_gen: 0,
            composited: false,
            anim_start: Instant::now(),
            reduce_motion: false,
            today: (2026, 1, 1),
            southern: false,
            source_doc: None,
            buttons: Vec::new(),
            hover: None,
            chrome_dirty: true,
            show_until: None,
            last_tick: None,
            toc_open: false,
            toc_scroll: 0.0,
            help: false,
            help_advanced: false,
            help_reveal: RectF::default(),
            find: None,
            dirty: true,
            dragging: false,
            press: (0.0, 0.0),
            pressed_chrome: false,
            last_click: None,
            last_layout_time: Duration::ZERO,
            resume: None,
            rebuild: None,
            glide: None,
            bar_alpha: 0.0,
            bar_until: None,
            bar_seen: 0.0,
            slide: None,
            asked: Vec::new(),
            image_fades: Vec::new(),
        };
        v.title = v.display_title(title);
        v
    }

    /// An image's opacity: below 1 while it fades in after it arrived.
    #[cfg(test)]
    pub fn image_alpha(&self, url: &str) -> f32 {
        self.image_fades.iter().find(|f| f.url == url).map_or(1.0, |f| f.alpha)
    }

    /// Whether an image of the document is in view.
    fn image_in_view(&self, url: &str) -> bool {
        self.layout.images.iter().any(|i| i.url == url && i.r.y + i.r.h > self.scroll && i.r.y < self.scroll + self.view_h)
    }

    /// An image that arrived is in view and waits for its fade to start (in `tick`).
    pub fn image_fade_waiting(&self) -> bool {
        self.image_fades.iter().any(|f| f.start.is_none() && self.image_in_view(&f.url))
    }

    /// Load the images within a screen above and below the view (the platform starts each
    /// once); those asked for that have been placed since fade in.
    fn ask_images<T: Typesetter<Para = P>>(&mut self, ts: &mut T) {
        let (top, bottom) = (self.scroll - self.view_h, self.scroll + 2.0 * self.view_h);
        for p in self.layout.pending.iter().filter(|p| p.r.y + p.r.h >= top && p.r.y <= bottom) {
            ts.load_image(&p.url);
            if !self.asked.contains(&p.url) {
                self.asked.push(p.url.clone());
            }
        }
        let images = &self.layout.images;
        let mut arrived = Vec::new();
        self.asked.retain(|url| {
            let placed = images.iter().any(|i| &i.url == url);
            if placed {
                arrived.push(url.clone());
            }
            !placed
        });
        if !self.reduce_motion {
            self.image_fades.extend(arrived.into_iter().map(|url| ImageFade { url, alpha: 0.0, start: None }));
        }
    }

    /// Start the fades of arrived images that came into view, advance the others; true
    /// while one is fading.
    fn step_image_fades(&mut self, now: Instant) -> bool {
        let mut fades = std::mem::take(&mut self.image_fades);
        for f in &mut fades {
            if f.start.is_none() && self.image_in_view(&f.url) {
                f.start = Some(now);
            }
            if let Some(s) = f.start {
                let t = (now.saturating_duration_since(s).as_secs_f32() / IMAGE_FADE).min(1.0);
                f.alpha = 1.0 - (1.0 - t) * (1.0 - t);
            }
        }
        let fading = fades.iter().any(|f| f.start.is_some());
        fades.retain(|f| f.alpha < 1.0);
        self.image_fades = fades;
        fading
    }

    /// Plain text of the document's first heading (any level, document order), if any.
    pub fn first_heading(&self) -> Option<String> {
        fn find(n: &Node) -> Option<&Node> {
            if matches!(n.kind, Kind::Heading { .. }) {
                return Some(n);
            }
            n.children.iter().find_map(find)
        }
        let text = find(&self.doc)?.plain_text();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    }

    fn display_title(&self, fallback: String) -> String {
        self.front_matter.iter().find(|(k, _)| k == "title").map(|(_, v)| v.clone()).filter(|t| !t.is_empty()).unwrap_or(fallback)
    }

    /// Replace the document. `keep_scroll` is used when reloading the same file.
    pub fn set_document(&mut self, title: String, source: String, doc: Node, keep_scroll: bool) {
        self.front_matter = front_matter_of(&doc);
        self.title = self.display_title(title);
        self.doc = doc;
        self.source = source;
        self.source_doc = None;
        if !keep_scroll {
            self.scroll = 0.0;
            self.show_source = false;
            self.toc_scroll = 0.0;
        }
        self.selection = None;
        self.asked.clear();
        self.image_fades.clear();
        self.find = self.find.take().map(|f| Find { matches: Vec::new(), current: 0, ..f });
        self.dirty = true;
        self.chrome_dirty = true;
    }

    pub fn set_dark(&mut self, dark: bool) {
        if dark != self.dark {
            self.dark = dark;
            self.dirty = true;
        }
    }

    pub fn set_fullscreen(&mut self, on: bool) {
        self.fullscreen = on;
        self.chrome_dirty = true;
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        if w != self.view_w {
            self.dirty = true;
        }
        self.view_w = w;
        self.view_h = h;
        self.chrome_dirty = true;
        self.clamp_scroll();
    }

    /// Force a relayout, e.g. after the platform lost its rendering device or images arrived.
    pub fn invalidate(&mut self) {
        self.dirty = true;
        self.chrome_dirty = true;
    }

    fn resolve_theme(&self) -> Theme {
        let fm = self.front_matter.iter().find(|(k, _)| k == "theme").map(|(_, v)| v.as_str());
        let auto = self.theme_choice.0.is_none();
        let name = fm.filter(|_| auto).unwrap_or(self.theme_choice.name());
        if auto { Theme::for_page(name, self.dark, &self.front_matter) } else { Theme::named(name, self.dark) }
    }

    /// Top-level layout step at the top of the view: (node index, its y).
    fn scroll_anchor(&self) -> Option<(usize, f32)> {
        if self.scroll <= 0.0 {
            return None;
        }
        let tops = &self.layout.tops;
        let i = tops.partition_point(|t| t.1 <= self.scroll).checked_sub(1)?;
        Some((tops[i].0, tops[i].1))
    }

    pub fn ensure_layout<T: Typesetter<Para = P>>(&mut self, ts: &mut T) {
        if self.dirty {
            self.dirty = false;
            // Keep the reading position: the block at the top of the view stays there; at the
            // end of the document the view stays at the end (an image arriving there grows it),
            // and so does a glide on its way there (the wheel spun to the end).
            let anchor = self.scroll_anchor();
            let to = self.glide.map_or(self.scroll, |g| g.to);
            let at_end = self.resume.is_none() && to > 0.0 && to >= self.layout.height - self.view_h - 0.5;
            self.theme = self.resolve_theme();
            // One seed at every width: a particle keeps its glyph and size, only x follows.
            self.particles = self.theme.overlay.map(|o| Particles::new(o, self.view_w, 0x9e37_79b9));
            self.particles_gen = self.particles_gen.wrapping_add(1);
            let w = self.view_w;
            let margin = (self.font_size * 2.0).min(w * 0.06).max(12.0);
            let column = (w - 2.0 * margin).min(self.font_size * MAX_COLUMN).max(100.0);
            let x = ((w - column) / 2.0).max(margin).round();
            let started = Instant::now();
            if self.show_source && self.source_doc.is_none() {
                let mut code = Node { kind: Kind::CodeBlock { lang: "markdown".into() }, text: self.source.clone(), children: Vec::new() };
                code.text.push('\n');
                self.source_doc = Some(Node { kind: Kind::Document, text: String::new(), children: vec![code] });
            }
            let doc = if self.show_source { self.source_doc.as_ref().unwrap() } else { &self.doc };
            // Only the screens around the view are laid out now; the rest follows in idle
            // time (`continue_layout`) or on demand. Searching and the TOC need everything.
            let whole = self.find.is_some() || self.toc_open;
            let until = if whole { f32::INFINITY } else { self.scroll + self.view_h * 2.0 };
            let top = Resume { next: 0, y: self.font_size * 1.5, x, width: column };
            let mut layouter = Layouter { ts, theme: &self.theme, base: self.font_size };
            self.layout = DocLayout::default();
            self.rebuild = None;
            match anchor {
                // Scrolled down: lay out from the anchor block at its old place now
                // (provisional); the exact layout from the top follows in `rebuild`.
                Some((node, y0)) if !whole => {
                    let node = node.min(doc.children.len());
                    let mut at = Resume { next: node, y: y0, ..top };
                    let done = layouter.layout_more(doc, &mut at, &mut self.layout, until, usize::MAX, None);
                    self.resume = (!done).then_some(at);
                    self.rebuild = Some(Rebuild { layout: DocLayout::default(), at: top, node, y0 });
                }
                _ => {
                    let mut at = top;
                    let done = layouter.layout_more(doc, &mut at, &mut self.layout, until, usize::MAX, None);
                    self.resume = (!done).then_some(at);
                    if let Some((node, y0)) = anchor {
                        if let Some(t) = self.layout.tops.iter().find(|t| t.0 >= node) {
                            self.shift_scroll(t.1 - y0);
                        }
                    }
                }
            }
            if at_end {
                // The rest from the view on (little: the end was in view).
                self.extend_layout(ts, f32::INFINITY, None);
                if self.resume.is_none() {
                    let end = (self.layout.height - self.view_h).max(0.0);
                    match &mut self.glide {
                        Some(g) => g.to = end,
                        None => self.shift_scroll(end - self.scroll),
                    }
                }
            }
            self.last_layout_time = started.elapsed();
            self.selection = None;
            self.clamp_scroll();
            self.refind(false);
            self.chrome_dirty = true;
        }
        // Scrolled above the provisional part: the exact layout is needed now.
        if self.rebuild.as_ref().is_some_and(|r| self.scroll < r.y0) {
            self.advance_rebuild(ts, None);
        }
        // Scrolled past the laid-out part (wheel, End key): extend to cover the view.
        if self.resume.is_some_and(|at| at.y <= self.scroll + self.view_h) {
            self.extend_layout(ts, self.scroll + self.view_h * 2.0, None);
        }
        self.ask_images(ts);
        if self.chrome_dirty {
            self.chrome_dirty = false;
            self.build_chrome(ts);
        }
    }

    /// Lay out more of a partially laid-out document; finishing it settles the
    /// scroll position, search matches and chrome as a full layout would.
    fn extend_layout<T: Typesetter<Para = P>>(&mut self, ts: &mut T, until_y: f32, deadline: Option<Instant>) {
        let Some(mut at) = self.resume else { return };
        if self.dirty {
            return; // a fresh layout replaces it at the next paint
        }
        let doc = if self.show_source { self.source_doc.as_ref().unwrap() } else { &self.doc };
        let done = Layouter { ts, theme: &self.theme, base: self.font_size }.layout_more(doc, &mut at, &mut self.layout, until_y, usize::MAX, deadline);
        if done {
            self.resume = None;
            self.clamp_scroll();
            self.refind(false);
            self.chrome_dirty = true;
        } else {
            self.resume = Some(at);
        }
    }

    /// Advance the exact layout from the top behind a provisional one. Once it reaches
    /// the provisional part's first block it takes over: the blocks from there on are
    /// the same, so the scroll position (and a selection) just shift with it.
    fn advance_rebuild<T: Typesetter<Para = P>>(&mut self, ts: &mut T, deadline: Option<Instant>) {
        let Some(r) = &mut self.rebuild else { return };
        if self.dirty {
            return;
        }
        let doc = if self.show_source { self.source_doc.as_ref().unwrap() } else { &self.doc };
        let ended = Layouter { ts, theme: &self.theme, base: self.font_size }.layout_more(doc, &mut r.at, &mut r.layout, f32::INFINITY, r.node, deadline);
        if !ended && r.at.next < r.node {
            return;
        }
        let Some(r) = self.rebuild.take() else { return };
        let head = r.layout.blocks.len();
        self.shift_scroll(r.at.y - r.y0);
        self.selection = self.selection.map(|s| {
            let shift = |p: DocPos| DocPos { block: p.block + head, ..p };
            DocSelection { anchor: shift(s.anchor), focus: shift(s.focus) }
        });
        self.layout = r.layout;
        self.resume = (!ended).then_some(r.at);
        self.clamp_scroll();
        self.refind(false);
        self.chrome_dirty = true;
    }

    /// Whether part of the document still waits for `continue_layout`.
    pub fn layout_pending(&self) -> bool {
        (self.resume.is_some() || self.rebuild.is_some()) && !self.dirty
    }

    /// Idle-time slice of a pending layout; returns true once it is complete.
    pub fn continue_layout<T: Typesetter<Para = P>>(&mut self, ts: &mut T, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        self.advance_rebuild(ts, Some(deadline));
        if self.rebuild.is_none() {
            self.extend_layout(ts, f32::INFINITY, Some(deadline));
        }
        self.resume.is_none() && self.rebuild.is_none()
    }

    /// Complete a pending layout now (before input that may need all of it).
    pub fn finish_layout<T: Typesetter<Para = P>>(&mut self, ts: &mut T) {
        self.advance_rebuild(ts, None);
        self.extend_layout(ts, f32::INFINITY, None);
    }

    fn max_scroll(&self) -> f32 {
        // While the layout is incomplete the end is not known yet.
        if self.resume.is_some() || self.rebuild.is_some() { f32::INFINITY } else { (self.layout.height - self.view_h).max(0.0) }
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// The same content moved by `d` (a relayout): the view and a glide move with it, and
    /// the scroll bar does not take it for scrolling.
    fn shift_scroll(&mut self, d: f32) {
        self.scroll += d;
        self.bar_seen += d;
        if let Some(g) = &mut self.glide {
            g.from += d;
            g.to += d;
            g.at += d;
        }
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.glide = None;
        let before = self.scroll;
        self.scroll += dy;
        self.clamp_scroll();
        if self.scroll != before && self.tip.take().is_some() {
            self.chrome_dirty = true; // the tooltip's line moved away
        }
        if self.toc_open {
            self.chrome_dirty = true; // current-section highlight
        }
    }

    /// Scroll by `dy` gliding there (wheel notches): the target moves at once, the view
    /// follows in `tick`. Turning the wheel back starts from where the view is.
    pub fn glide_by(&mut self, dy: f32, now: Instant) {
        let base = match self.glide {
            Some(g) if (g.to - self.scroll) * dy > 0.0 => g.to,
            _ => self.scroll,
        };
        let to = (base + dy).clamp(0.0, self.max_scroll());
        // A frame ahead: the frame painted for the notch right away already moves (else each
        // notch of a fast spin left one frame standing still).
        let start = now.checked_sub(FRAME).unwrap_or(now);
        self.glide = (to != self.scroll).then_some(Glide { from: self.scroll, to, start, at: self.scroll });
        if self.glide.is_some() && self.tip.take().is_some() {
            self.chrome_dirty = true; // the tooltip's line moves away
        }
    }

    /// Advance a glide; true when the view moved.
    fn step_glide(&mut self, now: Instant) -> bool {
        let Some(mut g) = self.glide else { return false };
        if self.scroll != g.at {
            self.glide = None; // something else scrolled
            return false;
        }
        let t = (now.saturating_duration_since(g.start).as_secs_f32() / GLIDE).min(1.0);
        let y = g.from + (g.to - g.from) * (1.0 - (1.0 - t).powi(3));
        let before = self.scroll;
        self.scroll = y;
        self.clamp_scroll();
        g.at = self.scroll;
        self.glide = (t < 1.0 && self.scroll == y).then_some(g);
        if self.toc_open {
            self.chrome_dirty = true;
        }
        self.scroll != before
    }

    /// Before going `back` or forward in the history: the page shown is kept to slide out
    /// while the next one (set and laid out after this) slides in.
    pub fn slide(&mut self, back: bool, now: Instant) {
        if self.reduce_motion || self.view_w <= 0.0 {
            return;
        }
        // (The new layout then starts without the old one's tops as a scroll anchor.)
        let layout = std::mem::take(&mut self.layout);
        let scroll = (self.scroll * self.pixel).round() / self.pixel;
        self.slide = Some(Slide { layout, theme: self.theme.clone(), scroll, dir: if back { -1.0 } else { 1.0 }, start: now });
    }

    /// While a page change slides: (x of the page shown, x of the one leaving), in whole
    /// device pixels.
    pub fn slide_x(&self, now: Instant) -> Option<(f32, f32)> {
        let s = self.slide.as_ref()?;
        let t = (now.saturating_duration_since(s.start).as_secs_f32() / SLIDE).min(1.0);
        let eased = if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (2.0 - 2.0 * t).powi(3) / 2.0 };
        let x = (s.dir * self.view_w * (1.0 - eased) * self.pixel).round() / self.pixel;
        Some((x, x - s.dir * self.view_w))
    }

    /// The view scrolled since the scroll bar looked last (or glides).
    pub fn scroll_moved(&self) -> bool {
        self.scroll != self.bar_seen || self.glide.is_some()
    }

    /// The overlay scroll bar's knob and colour while it shows (scrolling).
    pub fn scroll_bar(&self) -> Option<(RectF, Ink)> {
        let max = self.layout.height - self.view_h;
        if self.bar_alpha <= 0.0 || self.help || max <= 0.5 {
            return None;
        }
        let (inset, w) = (3.0, 7.0);
        let track = self.view_h - 2.0 * inset;
        let len = (track * self.view_h / self.layout.height).max(32.0).min(track);
        let y = inset + (track - len) * (self.scroll / max).clamp(0.0, 1.0);
        let [r, g, b, _] = self.theme.background.0;
        let (rgb, a) = if (r as u32 + g as u32 + b as u32) < 384 { (0xffffff, 0.5) } else { (0x000000, 0.45) };
        let ink = Ink::rgba(rgb, (a * self.bar_alpha * 255.0).round() as u8);
        Some((RectF { x: self.view_w - inset - w, y, w, h: len }, ink))
    }

    pub fn page(&mut self, down: bool) {
        let page = (self.view_h - self.font_size * 4.0).max(self.font_size * 3.0);
        self.scroll_by(if down { page } else { -page });
    }

    pub fn set_font_size(&mut self, size: f32) {
        let size = size.clamp(10.0, 40.0);
        if size != self.font_size {
            self.font_size = size;
            self.dirty = true;
        }
    }

    // ---------------------------------------------------------------- chrome

    /// Show the toolbar for a while (mouse at the top edge, tap, toolbar shortcut).
    pub fn show_chrome(&mut self, now: Instant) {
        self.show_until = Some(now + SHOW_FOR);
    }

    fn pinned(&self) -> bool {
        self.toc_open || self.find.is_some() || self.hover.is_some()
    }

    pub fn chrome_visible(&self) -> bool {
        self.chrome_alpha > 0.01
    }

    /// Advance the fade animation. Returns (repaint, time until the next tick is needed).
    pub fn tick(&mut self, now: Instant) -> (bool, Option<Duration>) {
        let dt = self.last_tick.map_or(0.0, |t| (now - t).as_secs_f32()).min(0.1);
        self.last_tick = Some(now);
        // Fade `alpha` towards on (1) or off (0); true while it is not there yet.
        let fade = |alpha: &mut f32, on: bool, fade_in: f32, fade_out: f32| {
            *alpha = if on { (*alpha + dt / fade_in).min(1.0) } else { (*alpha - dt / fade_out).max(0.0) };
            *alpha != if on { 1.0 } else { 0.0 }
        };
        let mut repaint = self.step_glide(now);
        let visible = self.pinned() || self.show_until.is_some_and(|t| t > now);
        let before = self.chrome_alpha;
        let chrome_fading = fade(&mut self.chrome_alpha, visible, FADE_IN, FADE_OUT);
        // The overlay scroll bar shows while the view scrolls, by whatever means.
        if self.scroll != self.bar_seen {
            self.bar_seen = self.scroll;
            if self.layout.height > self.view_h + 0.5 {
                self.bar_until = Some(now + BAR_FOR);
            }
        }
        let bar_on = self.bar_until.is_some_and(|t| t > now);
        let bar_before = self.bar_alpha;
        let bar_fading = fade(&mut self.bar_alpha, bar_on, BAR_FADE_IN, BAR_FADE_OUT);
        repaint |= before != self.chrome_alpha || bar_before != self.bar_alpha;
        if let Some(s) = &self.slide {
            repaint = true;
            if now.saturating_duration_since(s.start).as_secs_f32() >= SLIDE {
                self.slide = None;
            }
        }
        let images_fading = self.step_image_fades(now);
        repaint |= images_fading;
        let animating = chrome_fading || bar_fading || images_fading || self.glide.is_some() || self.slide.is_some();
        if !animating {
            self.last_tick = None;
        }
        let mut next = animating.then_some(FRAME);
        let mut wake = |at: Option<Instant>| {
            if let Some(t) = at {
                let d = t.saturating_duration_since(now) + Duration::from_millis(1);
                next = Some(next.map_or(d, |n: Duration| n.min(d)));
            }
        };
        if visible && !self.pinned() {
            wake(self.show_until);
        }
        if bar_on {
            wake(self.bar_until);
        }
        if !self.layout.cycles.is_empty() && !self.reduce_motion {
            // Wake for the next quote crossfade (every 10 s, 600 ms long).
            let t = (now - self.anim_start).as_millis() as u64 % 10_000;
            let wait = if t < 650 { 33 } else { 10_000 - t };
            let d = Duration::from_millis(wait);
            repaint |= t < 650;
            let next = Some(next.map_or(d, |n| n.min(d)));
            if self.particles.is_none() || self.composited {
                return (repaint, next);
            }
        }
        if self.particles.is_some() && !self.reduce_motion && !self.composited {
            repaint = true;
            let frame = Some(Duration::from_millis(33));
            return (repaint, next.map_or(frame, |n| Some(n.min(Duration::from_millis(33)))));
        }
        (repaint, next)
    }

    fn build_chrome<T: Typesetter<Para = P>>(&mut self, ts: &mut T) {
        let th = &self.theme;
        let mut out: DocLayout<P> = DocLayout::default();
        let mut buttons = Vec::new();
        let w = self.view_w;
        let style = BaseStyle { size: UI_SIZE, color: th.chrome_text, mono: false, bold: false, italic: false, line_height: 1.25 };
        let label = |ts: &mut T, text: &str, style: &BaseStyle, max_w: f32| {
            let para = ts.paragraph(text, &[], style, max_w);
            let (pw, ph) = para.size();
            (para, pw, ph)
        };
        let place = |out: &mut DocLayout<P>, para: P, text: &str, x: f32, y: f32, pw: f32, ph: f32| {
            out.blocks.push(TextBlock { para, text: text.to_string(), x, y, w: pw, h: ph, selectable: false, join: Join::Line, links: Vec::new(), object_sources: Vec::new() });
        };
        out.rects.push(Rect { r: RectF { x: 0.0, y: 0.0, w, h: BAR_H }, color: th.chrome_bg, radius: 0.0 });
        out.rects.push(Rect { r: RectF { x: 0.0, y: BAR_H - 1.0, w, h: 1.0 }, color: th.border, radius: 0.0 });

        let pad = 10.0;
        let bh = 32.0;
        let by = (BAR_H - bh) / 2.0;
        let button = |ts: &mut T, out: &mut DocLayout<P>, buttons: &mut Vec<(RectF, Action)>, text: &str, action: Action, x: f32, from_right: bool, active: bool| -> f32 {
            let (para, pw, ph) = label(ts, text, &style, 400.0);
            let bw = pw + 2.0 * pad;
            let bx = if from_right { x - bw } else { x };
            let r = RectF { x: bx, y: by, w: bw, h: bh };
            if self.hover == Some(action) || active {
                out.rects.push(Rect { r, color: th.chrome_hover, radius: 6.0 });
            }
            place(out, para, text, bx + pad, by + (bh - ph) / 2.0, pw, ph);
            buttons.push((r, action));
            bw
        };

        // Left group.
        let mut x = 6.0 + self.bar_inset;
        x += button(ts, &mut out, &mut buttons, &["☰  ", ui(Ui::Contents)].concat(), Action::Toc, x, false, self.toc_open) + 2.0;
        x += button(ts, &mut out, &mut buttons, ui(Ui::Open), Action::Open, x, false, false) + 2.0;
        let left_end = x;

        // Right group, laid out from the right edge.
        let mut rx = w - 6.0;
        let fs = if self.fullscreen { ["⤡  ", ui(Ui::Window)].concat() } else { ["⤢  ", ui(Ui::FullScreen)].concat() };
        rx -= button(ts, &mut out, &mut buttons, &fs, Action::Fullscreen, rx, true, false) + 2.0;
        if self.fullscreen {
            let pin = [if self.remember_fullscreen { "☑ " } else { "☐ " }, ui(Ui::OnStart)].concat();
            rx -= button(ts, &mut out, &mut buttons, &pin, Action::RememberFullscreen, rx, true, false) + 2.0;
        }
        rx -= button(ts, &mut out, &mut buttons, &self.theme_choice.label(), Action::Theme, rx, true, false) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "</>", Action::Source, rx, true, self.show_source) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "A+", Action::FontUp, rx, true, false) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "A−", Action::FontDown, rx, true, false) + 2.0;
        if self.find.is_none() {
            rx -= button(ts, &mut out, &mut buttons, &["⌕  ", ui(Ui::Search)].concat(), Action::Search, rx, true, false) + 2.0;
        }

        // Centre: title or find field.
        let avail = (rx - left_end - 16.0).max(0.0);
        if let Some(find) = &self.find {
            let count = if find.query.is_empty() {
                String::new()
            } else if find.matches.is_empty() {
                ui(Ui::NoMatches).to_string()
            } else {
                [num(find.current as u64 + 1), num(find.matches.len() as u64)].join(" / ")
            };
            let mut cx = rx - 2.0;
            cx -= button(ts, &mut out, &mut buttons, "✕", Action::FindClose, cx, true, false) + 2.0;
            cx -= button(ts, &mut out, &mut buttons, "↓", Action::FindNext, cx, true, false) + 2.0;
            cx -= button(ts, &mut out, &mut buttons, "↑", Action::FindPrev, cx, true, false) + 2.0;
            let muted = BaseStyle { color: th.muted, ..style };
            let (para, pw, ph) = label(ts, &count, &muted, 400.0);
            cx -= pw + 8.0;
            place(&mut out, para, &count, cx, (BAR_H - ph) / 2.0, pw, ph);
            let field_x = left_end + 8.0;
            let field_w = (cx - 8.0 - field_x).max(40.0);
            out.rects.push(Rect { r: RectF { x: field_x, y: by, w: field_w, h: bh }, color: th.border, radius: 6.0 });
            out.rects.push(Rect { r: RectF { x: field_x + 1.0, y: by + 1.0, w: field_w - 2.0, h: bh - 2.0 }, color: th.background, radius: 5.0 });
            let text = ["⌕  ", &find.query, "▏"].concat();
            let (para, pw, ph) = label(ts, &text, &style, field_w - 2.0 * pad);
            place(&mut out, para, &text, field_x + pad, by + (bh - ph) / 2.0, pw, ph);
        } else if avail > 60.0 {
            let title = if self.show_source { [&self.title, " (", ui(Ui::Source), ")"].concat() } else { self.title.clone() };
            let bold = BaseStyle { bold: true, ..style };
            let (mut para, mut pw, mut ph) = label(ts, &title, &bold, 4000.0);
            if pw > avail {
                para = ts.paragraph(&title, &[], &bold, avail);
                (pw, ph) = (avail, para.size().1.min(BAR_H));
            }
            let tx = ((w - pw) / 2.0).max(left_end + 8.0).min(rx - 8.0 - pw);
            if ph < BAR_H {
                place(&mut out, para, &title, tx, (BAR_H - ph) / 2.0, pw, ph);
            }
        }

        // Table of contents drawer.
        if self.toc_open {
            let dw = (w * 0.85).min(340.0);
            let top = BAR_H;
            out.rects.push(Rect { r: RectF { x: 0.0, y: top, w: dw, h: self.view_h - top }, color: th.chrome_bg, radius: 0.0 });
            out.rects.push(Rect { r: RectF { x: dw - 1.0, y: top, w: 1.0, h: self.view_h - top }, color: th.border, radius: 0.0 });
            let current = self.current_section();
            let mut y = top + 8.0 - self.toc_scroll;
            if let Some(name) = self.seasonal_suggestion() {
                let text = [ui(Ui::Seasonal).into(), " ".into(), crate::theme::theme_def(name).map(|d| d.label()).unwrap_or_default()].concat();
                let s = BaseStyle { color: th.link, ..style };
                let (para, pw, ph) = label(ts, &text, &s, dw - 32.0);
                let row = RectF { x: 6.0, y, w: dw - 12.0, h: ph + 12.0 };
                if self.hover == Some(Action::Seasonal) {
                    out.rects.push(Rect { r: row, color: th.chrome_hover, radius: 6.0 });
                }
                place(&mut out, para, &text, 16.0, y + 6.0, pw, ph);
                buttons.push((row, Action::Seasonal));
                y += ph + 12.0;
                out.rects.push(Rect { r: RectF { x: 16.0, y: y + 3.0, w: dw - 32.0, h: 1.0 }, color: th.border, radius: 0.0 });
                y += 8.0;
            }
            if self.layout.toc.is_empty() {
                let muted = BaseStyle { color: th.muted, ..style };
                let (para, pw, ph) = label(ts, ui(Ui::NoHeadings), &muted, dw - 32.0);
                place(&mut out, para, ui(Ui::NoHeadings), 16.0, y + 8.0, pw, ph);
            }
            let min_level = self.layout.toc.iter().map(|e| e.level).min().unwrap_or(1);
            for (i, e) in self.layout.toc.iter().enumerate() {
                let indent = 16.0 + (e.level - min_level) as f32 * 14.0;
                let s = BaseStyle {
                    bold: e.level == min_level || current == Some(i),
                    color: if current == Some(i) { th.link } else if e.level > min_level + 1 { th.muted } else { th.chrome_text },
                    ..style
                };
                let (para, pw, ph) = label(ts, &e.title, &s, dw - indent - 16.0);
                let row = RectF { x: 6.0, y, w: dw - 12.0, h: ph + 12.0 };
                if row.y + row.h > top && row.y < self.view_h {
                    if self.hover == Some(Action::TocEntry(i)) || current == Some(i) {
                        out.rects.push(Rect { r: row, color: th.chrome_hover, radius: 6.0 });
                    }
                    place(&mut out, para, &e.title, indent, y + 6.0, pw, ph);
                    buttons.push((row, Action::TocEntry(i)));
                }
                y += ph + 12.0;
            }
            out.height = y + self.toc_scroll - top + 8.0; // content height of the drawer
        }
        self.chrome = out;
        let mut tip_out: DocLayout<P> = DocLayout::default();
        (self.tip_box, self.tip_copy, self.tip_zone) = Default::default();
        if let Some(t) = &self.tip {
            let (para, pw, ph) = label(ts, &t.text, &style, 360.0);
            // Above the line it explains (below when there is no room), growing to the
            // right by a copy button while pointed at.
            let bh = ph + 12.0;
            let btn = ph + 4.0;
            let full_w = pw + 20.0 + btn + 2.0;
            let bw = if t.hot { full_w } else { pw + 20.0 };
            let bx = (t.x - (pw + 20.0) / 2.0).clamp(4.0, (self.view_w - full_w - 4.0).max(4.0));
            let above = t.top - 6.0 - bh >= 4.0;
            let by = if above { t.top - 6.0 - bh } else { t.bottom + 6.0 };
            self.tip_box = RectF { x: bx, y: by, w: bw, h: bh };
            self.tip_zone = if above {
                RectF { x: bx, y: by, w: bw, h: t.top - by }
            } else {
                RectF { x: bx, y: t.bottom, w: bw, h: by + bh - t.bottom }
            };
            tip_out.rects.push(Rect { r: RectF { x: bx - 1.0, y: by - 1.0, w: bw + 2.0, h: bh + 2.0 }, color: th.border, radius: 7.0 });
            tip_out.rects.push(Rect { r: self.tip_box, color: th.chrome_bg, radius: 6.0 });
            place(&mut tip_out, para, &t.text, bx + 10.0, by + 6.0, pw, ph);
            if t.hot {
                let r = RectF { x: bx + pw + 16.0, y: by + 4.0, w: btn, h: btn };
                self.tip_copy = r;
                let bg = if t.on_copy { th.chrome_hover } else { th.chrome_bg };
                tip_out.rects.push(Rect { r, color: bg, radius: 4.0 });
                let (cx, cy) = (r.x + btn / 2.0, r.y + btn / 2.0);
                if t.copied {
                    let (para, cw, ch) = label(ts, "✓", &BaseStyle { color: th.link, ..style }, 100.0);
                    place(&mut tip_out, para, "✓", cx - cw / 2.0, cy - ch / 2.0, cw, ch);
                } else {
                    // Copy icon: two overlapping outlined squares.
                    let s = (btn * 0.42).round();
                    let o = (s * 0.4).round();
                    for (x, y) in [(cx - (s + o) / 2.0, cy - (s + o) / 2.0), (cx - (s + o) / 2.0 + o, cy - (s + o) / 2.0 + o)] {
                        tip_out.rects.push(Rect { r: RectF { x, y, w: s, h: s }, color: th.chrome_text, radius: 2.5 });
                        tip_out.rects.push(Rect { r: RectF { x: x + 1.5, y: y + 1.5, w: s - 3.0, h: s - 3.0 }, color: bg, radius: 1.5 });
                    }
                }
            }
        }
        if self.help {
            (tip_out, self.help_reveal) = self.build_help(ts);
        }
        self.tip_layout = tip_out;
        self.buttons = buttons;
    }

    /// F1 overlay: the page dimmed, on it a panel with the keyboard shortcuts and the
    /// command line, in two columns when the window is wide enough; also the rectangle of
    /// "Advanced usage".
    fn build_help<T: Typesetter<Para = P>>(&self, ts: &mut T) -> (DocLayout<P>, RectF) {
        let th = &self.theme;
        let (w, h) = (self.view_w, self.view_h);
        let text = BaseStyle { size: UI_SIZE, color: th.chrome_text, mono: false, bold: false, italic: false, line_height: 1.3 };
        let keys = BaseStyle { bold: true, ..text };
        let head = BaseStyle { color: th.link, ..keys };
        let two = w >= 860.0;
        let col_w = if two { 380.0 } else { (w - 64.0).clamp(220.0, 460.0) };
        let (gap, key_w) = (36.0, 158.0);
        // (paragraph, text, x, y, w, h) relative to the panel's content origin.
        let mut items: Vec<(P, String, f32, f32, f32, f32)> = Vec::new();
        let mut put = |ts: &mut T, s: &str, style: &BaseStyle, x: f32, y: f32, max_w: f32| {
            let para = ts.paragraph(s, &[], style, max_w);
            let (pw, ph) = para.size();
            items.push((para, s.to_string(), x, y, pw, ph));
            (pw, ph)
        };
        let mut texts = texts().iter().copied();
        let mut next = || texts.next().unwrap_or("");
        let (title_s, hint_s) = (next(), next());
        let words = [next(), next()];
        let mut col_h = [0.0f32; 2];
        for &(col, advanced, rows) in HELP {
            let heading = next();
            if advanced && !self.help_advanced {
                rows.iter().for_each(|_| _ = next());
                continue;
            }
            let c = if two { col } else { 0 };
            let x0 = c as f32 * (col_w + gap);
            let mut y = if col_h[c] > 0.0 { col_h[c] + 14.0 } else { 0.0 };
            y += put(ts, heading, &head, x0, y, col_w).1 + 6.0;
            for &k in rows {
                let d = next().replace("{path}", crate::settings::PATH_HINT);
                let (kw, kh) = put(ts, &help_keys(k, &words), &keys, x0, y, col_w);
                // Long keys (commands, variables) get the description on the next line.
                let (dx, dy) = if kw + 12.0 <= key_w { (x0 + key_w, y) } else { (x0 + 16.0, y + kh) };
                let dh = put(ts, &d, &text, dx, dy, col_w - (dx - x0)).1;
                y = (y + kh).max(dy + dh) + 5.0;
            }
            col_h[c] = y;
        }
        // "Advanced usage" under both columns opens and closes the command line and
        // environment variable sections.
        let ry = col_h[0].max(col_h[1]) + 10.0;
        let reveal = [if self.help_advanced { "▾  " } else { "▸  " }, ui(Ui::Advanced)].concat();
        let (rw, rh) = put(ts, &reveal, &head, 0.0, ry, col_w);
        let bottom = ry + rh + 5.0;
        let pad = 24.0;
        let content_w = if two { 2.0 * col_w + gap } else { col_w };
        let title_p = ts.paragraph(title_s, &[], &BaseStyle { size: 18.0, ..keys }, content_w);
        let (tw, tht) = title_p.size();
        let hint_p = ts.paragraph(hint_s, &[], &BaseStyle { color: th.muted, ..text }, content_w);
        let (hw, hh) = hint_p.size();
        let (pw, ph) = (content_w + 2.0 * pad, pad + tht + 14.0 + bottom - 5.0 + pad);
        let (px, py) = (((w - pw) / 2.0).max(8.0).round(), ((h - ph) / 2.0).max(8.0).round());
        let mut out: DocLayout<P> = DocLayout::default();
        out.rects.push(Rect { r: RectF { x: 0.0, y: 0.0, w, h }, color: Ink::rgba(0x000000, 120), radius: 0.0 });
        out.rects.push(Rect { r: RectF { x: px - 1.0, y: py - 1.0, w: pw + 2.0, h: ph + 2.0 }, color: th.border, radius: 11.0 });
        out.rects.push(Rect { r: RectF { x: px, y: py, w: pw, h: ph }, color: th.chrome_bg, radius: 10.0 });
        let mut place = |para: P, s: &str, x: f32, y: f32, bw: f32, bh: f32| {
            out.blocks.push(TextBlock { para, text: s.to_string(), x, y, w: bw, h: bh, selectable: false, join: Join::Line, links: Vec::new(), object_sources: Vec::new() });
        };
        place(title_p, title_s, px + pad, py + pad, tw, tht);
        place(hint_p, hint_s, px + pw - pad - hw, py + pad + tht - hh, hw, hh);
        let top = py + pad + tht + 14.0;
        for (para, s, x, y, bw, bh) in items {
            place(para, &s, px + pad + x, top + y, bw, bh);
        }
        (out, RectF { x: px + pad - 6.0, y: top + ry - 4.0, w: rw + 12.0, h: rh + 8.0 })
    }

    fn drawer_width(&self) -> f32 {
        (self.view_w * 0.85).min(340.0)
    }

    fn current_section(&self) -> Option<usize> {
        let probe = self.scroll + self.font_size * 3.0;
        self.layout.toc.iter().rposition(|e| e.y <= probe)
    }

    fn chrome_hit(&self, x: f32, y: f32) -> Option<Action> {
        if !self.chrome_visible() {
            return None;
        }
        self.buttons.iter().find(|(r, _)| contains(r, x, y)).map(|(_, a)| *a)
    }

    /// True if (x, y) is covered by visible chrome (toolbar or drawer).
    fn over_chrome(&self, x: f32, y: f32) -> bool {
        self.chrome_visible() && (y < BAR_H || (self.toc_open && x < self.drawer_width()))
    }

    fn act(&mut self, action: Action, now: Instant) -> Option<Effect> {
        self.chrome_dirty = true;
        self.show_chrome(now);
        match action {
            Action::Toc => self.toc_open = !self.toc_open,
            Action::Open => return Some(Effect::OpenDialog),
            Action::Search => self.open_find(),
            Action::FontDown => self.set_font_size(self.font_size - 1.0),
            Action::FontUp => self.set_font_size(self.font_size + 1.0),
            Action::Source => self.toggle_source(),
            Action::Theme => {
                self.theme_choice = self.theme_choice.next();
                self.dirty = true;
            }
            Action::Fullscreen => return Some(Effect::ToggleFullscreen),
            Action::RememberFullscreen => self.remember_fullscreen = !self.remember_fullscreen,
            Action::Seasonal => {
                if let Some(name) = self.seasonal_suggestion() {
                    self.theme_choice = ThemeChoice::from_name(name);
                    self.dirty = true;
                }
            }
            Action::TocEntry(i) => {
                if let Some(e) = self.layout.toc.get(i) {
                    self.scroll_by(e.y - self.font_size - self.scroll);
                    self.selection = None;
                }
                if self.view_w < 900.0 {
                    self.toc_open = false;
                }
            }
            Action::FindPrev => self.find_step(false),
            Action::FindNext => self.find_step(true),
            Action::FindClose => self.find = None,
        }
        None
    }

    pub fn toggle_source(&mut self) {
        self.show_source = !self.show_source;
        self.scroll = 0.0;
        self.dirty = true;
        self.chrome_dirty = true;
    }

    // ------------------------------------------------------------------ find

    fn open_find(&mut self) {
        if self.find.is_none() {
            self.find = Some(Find { query: String::new(), matches: Vec::new(), current: 0 });
        }
        self.chrome_dirty = true;
    }

    /// Matches as (block, byte range, is current) for drawing.
    pub fn find_highlights(&self) -> Vec<(usize, Range<usize>, bool)> {
        let Some(f) = &self.find else { return Vec::new() };
        f.matches.iter().enumerate().map(|(i, (b, r))| (*b, r.clone(), i == f.current)).collect()
    }

    fn refind(&mut self, reveal: bool) {
        let Some(f) = &mut self.find else { return };
        f.matches.clear();
        let q: Vec<char> = f.query.chars().flat_map(char::to_lowercase).collect();
        if !q.is_empty() {
            for (bi, block) in self.layout.blocks.iter().enumerate() {
                if !block.selectable {
                    continue;
                }
                let chars: Vec<(usize, char)> = block.text.char_indices().collect();
                let mut i = 0;
                while i < chars.len() {
                    let mut j = 0;
                    let mut k = i;
                    while j < q.len() && k < chars.len() {
                        let mut lc = chars[k].1.to_lowercase();
                        if lc.next() != Some(q[j]) || lc.next().is_some() {
                            break;
                        }
                        j += 1;
                        k += 1;
                    }
                    if j == q.len() {
                        let end = chars.get(k).map_or(block.text.len(), |c| c.0);
                        f.matches.push((bi, chars[i].0..end));
                        i = k;
                    } else {
                        i += 1;
                    }
                }
            }
        }
        // Start at the first match on or below the current view.
        let top = self.scroll;
        f.current = f.matches.iter().position(|(b, _)| self.layout.blocks[*b].y >= top).unwrap_or(0);
        self.chrome_dirty = true;
        if reveal {
            self.reveal_match();
        }
    }

    fn find_step(&mut self, forward: bool) {
        if let Some(f) = &mut self.find {
            let n = f.matches.len();
            if n > 0 {
                f.current = if forward { (f.current + 1) % n } else { (f.current + n - 1) % n };
            }
        }
        self.chrome_dirty = true;
        self.reveal_match();
    }

    fn reveal_match(&mut self) {
        let Some(f) = &self.find else { return };
        let Some((b, r)) = f.matches.get(f.current) else { return };
        let block = &self.layout.blocks[*b];
        let y = block.para.range_rects(r.clone()).first().map_or(block.y, |rr| block.y + rr.y);
        if y < self.scroll + BAR_H + 8.0 || y > self.scroll + self.view_h - self.font_size * 3.0 {
            self.scroll_by(y - self.view_h / 3.0 - self.scroll);
        }
    }

    /// Text input (WM_CHAR etc.). Only used by the find field.
    pub fn text_input(&mut self, c: char) -> bool {
        let Some(f) = &mut self.find else { return false };
        if c.is_control() || self.help {
            return false;
        }
        f.query.push(c);
        self.refind(true);
        true
    }

    // ----------------------------------------------------------------- input

    pub fn key(&mut self, key: Key, mods: Mods, now: Instant) -> Option<Effect> {
        let line = self.font_size * 3.0;
        if key == Key::F1 || self.help && matches!(key, Key::Escape | Key::Enter | Key::Space) {
            self.help = !self.help;
            self.tip = None;
            self.chrome_dirty = true;
            return None;
        }
        if self.help {
            return None;
        }
        if let Some(f) = &mut self.find {
            match key {
                Key::Escape => {
                    self.find = None;
                    self.chrome_dirty = true;
                    return None;
                }
                Key::Enter | Key::F3 => {
                    self.find_step(!mods.shift);
                    return None;
                }
                Key::Backspace => {
                    f.query.pop();
                    self.refind(true);
                    return None;
                }
                Key::Space | Key::Char(_) if !mods.ctrl => return None, // arrives as text input
                _ => {}
            }
        }
        match key {
            Key::PageDown | Key::Right => self.page(true),
            Key::PageUp | Key::Left => self.page(false),
            Key::Space => self.page(!mods.shift),
            Key::Down => self.scroll_by(line),
            Key::Up => self.scroll_by(-line),
            Key::Home => self.scroll_by(-f32::MAX / 4.0),
            Key::End => self.scroll_by(f32::MAX / 4.0),
            Key::F3 => {
                self.open_find();
                self.show_chrome(now);
            }
            Key::F5 => return Some(Effect::Reload),
            Key::F11 => return Some(Effect::ToggleFullscreen),
            Key::Escape if self.toc_open => {
                self.toc_open = false;
                self.chrome_dirty = true;
            }
            Key::Escape if self.fullscreen => return Some(Effect::ExitFullscreen),
            Key::Escape => self.selection = None,
            Key::Enter | Key::Backspace | Key::F1 => {}
            Key::Char(c) if mods.ctrl => match c.to_ascii_lowercase() {
                'c' => {
                    let text = self.selection.map(|s| self.layout.selected_text(&s)).unwrap_or_default();
                    return (!text.is_empty()).then_some(Effect::Copy(text));
                }
                'a' => self.selection = self.layout.select_all(),
                '+' | '=' => self.set_font_size(self.font_size + 1.0),
                '-' => self.set_font_size(self.font_size - 1.0),
                '0' => self.set_font_size(DEFAULT_FONT_SIZE),
                'f' => {
                    self.open_find();
                    self.show_chrome(now);
                }
                't' => return self.act(Action::Toc, now),
                'u' => return self.act(Action::Source, now),
                'd' => return self.act(Action::Theme, now),
                'o' => return Some(Effect::OpenDialog),
                'r' => return Some(Effect::Reload),
                'q' | 'w' => return Some(Effect::Quit),
                _ => {}
            },
            Key::Char(_) => {}
        }
        None
    }

    /// Wheel or trackpad scrolling by `dy` DIPs: gliding (from `glide`, now) or at once.
    pub fn wheel(&mut self, dy: f32, mods: Mods, x: f32, y: f32, glide: Option<Instant>) {
        if self.help {
            return;
        }
        if mods.ctrl {
            self.set_font_size(self.font_size + if dy < 0.0 { 1.0 } else { -1.0 });
        } else if self.toc_open && self.chrome_visible() && x < self.drawer_width() && y > BAR_H {
            let max = (self.chrome.height - (self.view_h - BAR_H)).max(0.0);
            self.toc_scroll = (self.toc_scroll + dy).clamp(0.0, max);
            self.chrome_dirty = true;
        } else if let Some(now) = glide {
            self.glide_by(dy, now);
        } else {
            self.scroll_by(dy);
        }
    }

    fn doc_point(&self, x: f32, y: f32) -> (f32, f32) {
        (x, y + self.scroll)
    }

    pub fn press(&mut self, x: f32, y: f32, mods: Mods, double_click_time: Duration) {
        self.press = (x, y);
        if self.help {
            self.pressed_chrome = false;
            return;
        }
        self.pressed_tip = self.over_tip(x, y);
        self.pressed_chrome = self.over_chrome(x, y);
        if self.pressed_tip || self.pressed_chrome {
            return;
        }
        let (dx, dy) = self.doc_point(x, y);
        let now = Instant::now();
        let count = match self.last_click {
            Some((t, p, n)) if now - t < double_click_time && (p.0 - dx).abs() < 4.0 && (p.1 - dy).abs() < 4.0 => n % 3 + 1,
            _ => 1,
        };
        self.last_click = Some((now, (dx, dy), count));
        let Some(pos) = self.layout.hit(dx, dy) else { return };
        self.selection = Some(match count {
            2 => self.layout.word_at(pos),
            3 => self.layout.block_selection(pos.block),
            _ => match self.selection {
                Some(sel) if mods.shift => DocSelection { focus: pos, ..sel },
                _ => DocSelection::caret(pos),
            },
        });
        self.dragging = count == 1;
    }

    /// Returns true if the view needs a repaint.
    pub fn mouse_move(&mut self, x: f32, y: f32, now: Instant) -> bool {
        let mut repaint = false;
        if self.help {
            return false;
        }
        if !self.dragging {
            if y < BAR_H + 12.0 && y >= 0.0 {
                let was = self.chrome_visible();
                self.show_chrome(now);
                repaint |= !was;
            }
            if let Some(t) = self.tip.as_mut().filter(|_| contains(&self.tip_zone, x, y) || contains(&self.tip_box, x, y)) {
                // On the tooltip or on the way to it: it stays, with its copy button.
                let (hot, on_copy) = (contains(&self.tip_box, x, y), contains(&self.tip_copy, x, y));
                if (hot, on_copy) != (t.hot, t.on_copy) {
                    (t.hot, t.on_copy) = (hot, on_copy);
                    self.chrome_dirty = true;
                    repaint = true;
                }
            } else {
                let tip = if self.over_chrome(x, y) { None } else { self.tip_at(x, y) };
                if tip.as_ref().map(|t| (&t.text, t.top)) != self.tip.as_ref().map(|t| (&t.text, t.top)) {
                    self.tip = tip;
                    self.chrome_dirty = true;
                    repaint = true;
                }
            }
            let hover = self.chrome_hit(x, y);
            if hover != self.hover {
                self.hover = hover;
                self.chrome_dirty = true;
                repaint = true;
            }
            return repaint;
        }
        if y < 0.0 {
            self.scroll_by(y);
        } else if y > self.view_h {
            self.scroll_by(y - self.view_h);
        }
        let (dx, dy) = self.doc_point(x, y);
        if let (Some(sel), Some(pos)) = (self.selection.as_mut(), self.layout.hit(dx, dy)) {
            sel.focus = pos;
        }
        true
    }

    pub fn mouse_leave(&mut self) {
        if self.hover.take().is_some() | self.tip.take().is_some() {
            self.chrome_dirty = true;
        }
    }

    /// The tooltip for the link or abbreviation at window point (x, y).
    fn tip_at(&self, x: f32, y: f32) -> Option<Tip> {
        let (dx, dy) = self.doc_point(x, y);
        let (url, top, bottom) = self.layout.link_line_at(dx, dy)?;
        Some(Tip::new(tip_text(url)?.to_string(), x, top - self.scroll, bottom - self.scroll))
    }

    fn over_tip(&self, x: f32, y: f32) -> bool {
        self.tip.is_some() && contains(&self.tip_box, x, y)
    }

    /// `tap` is true for touch input: a tap on the page toggles the chrome.
    pub fn release(&mut self, x: f32, y: f32, tap: bool, now: Instant) -> Option<Effect> {
        self.dragging = false;
        if self.help {
            // "Advanced usage" opens or closes those sections; a click anywhere else closes the help.
            if contains(&self.help_reveal, x, y) {
                self.help_advanced = !self.help_advanced;
            } else {
                self.help = false;
            }
            self.chrome_dirty = true;
            return None;
        }
        let moved = (x - self.press.0).abs() > 3.0 || (y - self.press.1).abs() > 3.0;
        if self.pressed_tip {
            // The tooltip takes clicks; its copy button copies the text.
            self.pressed_tip = false;
            self.pressed_chrome = false;
            let t = self.tip.as_mut().filter(|_| !moved && contains(&self.tip_copy, x, y))?;
            t.copied = true;
            self.chrome_dirty = true;
            return Some(Effect::Copy(t.text.clone()));
        }
        if self.pressed_chrome {
            self.pressed_chrome = false;
            if moved {
                return None;
            }
            return self.chrome_hit(x, y).and_then(|a| self.act(a, now));
        }
        if moved {
            return None;
        }
        let (dx, dy) = self.doc_point(x, y);
        let Some(url) = self.layout.link_at(dx, dy).map(str::to_string) else {
            if tap {
                if self.chrome_visible() && !self.pinned() {
                    self.show_until = None;
                } else {
                    self.show_chrome(now);
                }
                self.selection = None;
            } else if self.toc_open && self.view_w < 900.0 {
                self.toc_open = false;
                self.chrome_dirty = true;
            }
            return None;
        };
        if url.starts_with("abbr:") {
            // Touch has no hover: a tap shows the expansion, with its copy button.
            self.tip = self.tip_at(x, y).map(|t| Tip { hot: tap, ..t });
            self.chrome_dirty = true;
            return None;
        }
        if let Some(id) = url.strip_prefix('#') {
            let target = self.layout.toc.iter().find(|e| e.id == id).map(|e| e.y - self.font_size)
                .or_else(|| self.layout.anchors.iter().find(|(a, _)| a == id).map(|(_, ay)| *ay));
            if let Some(ty) = target {
                self.scroll_by(ty - self.scroll);
                self.selection = None;
            }
            None
        } else {
            Some(Effect::OpenUrl(url))
        }
    }

    pub fn over_link(&self, x: f32, y: f32) -> bool {
        if self.help || self.over_chrome(x, y) || self.over_tip(x, y) {
            return false;
        }
        let (dx, dy) = self.doc_point(x, y);
        self.layout.link_at(dx, dy).is_some()
    }

    /// Cursor hint: true over chrome buttons.
    pub fn over_button(&self, x: f32, y: f32) -> bool {
        if self.help {
            contains(&self.help_reveal, x, y)
        } else {
            self.over_tip(x, y) && contains(&self.tip_copy, x, y) || self.chrome_hit(x, y).is_some()
        }
    }

    pub fn over_chrome_area(&self, x: f32, y: f32) -> bool {
        self.over_chrome(x, y)
    }

    /// Touch pan / swipe. `dx` horizontal flick pages.
    pub fn swipe(&mut self, dx: f32, dy: f32) {
        if dx.abs() > 2.0 * dy.abs() && dx.abs() > 80.0 {
            self.page(dx < 0.0);
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.set_font_size((self.font_size * factor).round());
    }
}

impl<P: Paragraph> Viewer<P> {
    /// Particle sprites to draw now (empty without an animated theme).
    pub fn sprites(&self, now: Instant) -> Vec<Sprite> {
        match &self.particles {
            Some(p) if !self.reduce_motion && !self.composited => p.sprites((now - self.anim_start).as_secs_f32() * 1000.0, self.view_h),
            _ => Vec::new(),
        }
    }

    /// Seasonal theme to suggest in the drawer, unless the document picks a
    /// theme or the user already uses it.
    fn seasonal_suggestion(&self) -> Option<&'static str> {
        if self.front_matter.iter().any(|(k, _)| k == "theme") {
            return None;
        }
        let (y, m, d) = self.today;
        let name = seasonal_theme(y, m, d, self.southern);
        (self.theme_choice.name() != name).then_some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::tests::{Fake, FakePara, summary};

    /// The test document (built once: `:::quotes` order is shuffled per build).
    fn doc() -> Node {
        static DOC: std::sync::OnceLock<Node> = std::sync::OnceLock::new();
        DOC.get_or_init(build_doc).clone()
    }

    fn build_doc() -> Node {
        let one = [include_str!("../assets/sample.md"), include_str!("../assets/extensions.md"), include_str!("../assets/transform-tests/transforms.md")].concat();
        let src = one.repeat(6);
        let mut d = crate::ast::parse(&crate::transforms::preprocess_html(&src));
        crate::transforms::compose(&mut d, &crate::transforms::default_pipeline());
        d
    }

    fn viewer(w: f32) -> Viewer<FakePara> {
        let mut v = Viewer::new("t".into(), String::new(), doc());
        v.resize(w, 600.0);
        v
    }

    /// The complete layout of a fresh viewer at width `w`.
    fn reference(w: f32) -> String {
        let mut v = viewer(w);
        v.ensure_layout(&mut Fake);
        v.finish_layout(&mut Fake);
        summary(&v.layout)
    }

    /// A click on "Advanced usage" opens (and closes) the advanced sections and keeps the
    /// help; a click elsewhere closes it.
    #[test]
    fn help_advanced_usage_toggles() {
        let mut v = viewer(1000.0);
        let now = Instant::now();
        v.key(Key::F1, Mods::default(), now);
        v.ensure_layout(&mut Fake);
        let closed_blocks = v.tip_layout.blocks.len();
        let r = v.help_reveal;
        assert!(r.w > 0.0 && v.over_button(r.x + 2.0, r.y + 2.0));
        v.release(r.x + 2.0, r.y + 2.0, false, now);
        assert!(v.help && v.help_advanced);
        v.ensure_layout(&mut Fake);
        assert!(v.tip_layout.blocks.len() > closed_blocks, "advanced sections shown");
        v.release(5.0, 5.0, false, now);
        assert!(!v.help);
    }

    #[test]
    fn link_tooltips() {
        assert_eq!(tip_text("https://example.com/a?b"), Some("https://example.com/a?b"));
        assert_eq!(tip_text("#install"), Some("#install"));
        assert_eq!(tip_text("abbr:HyperText Markup Language"), Some("HyperText Markup Language"));
        assert_eq!(tip_text("#fn-1"), None);

        // Pointing at the sample's link and at its AST abbreviation shows URL and expansion.
        let mut v = viewer(900.0);
        v.ensure_layout(&mut Fake);
        v.finish_layout(&mut Fake);
        // The document holds the sample several times: `nth` picks the copy.
        let point_at = |v: &Viewer<FakePara>, target: &str, nth: usize| {
            let b = v.layout.blocks.iter().filter(|b| b.links.iter().any(|(_, u)| u == target)).nth(nth).unwrap();
            let r = &b.links.iter().find(|(_, u)| u == target).unwrap().0;
            (b.x + b.text[..r.start].chars().count() as f32 * 10.0 + 10.0, b.y + 5.0 - v.scroll)
        };
        let now = Instant::now();
        // Right under the window's top: below the line.
        let (_, y) = point_at(&v, "https://markdownr.dudesoft.app", 0);
        v.scroll_by(y - 20.0);
        let (x, y) = point_at(&v, "https://markdownr.dudesoft.app", 0);
        v.mouse_move(x, y, now);
        v.ensure_layout(&mut Fake);
        assert!(v.tip_box.y > v.tip.as_ref().unwrap().bottom);
        for (target, shown) in [("https://markdownr.dudesoft.app", "https://markdownr.dudesoft.app"), ("abbr:Abstract Syntax Tree", "Abstract Syntax Tree")] {
            let (_, y) = point_at(&v, target, 2);
            v.scroll_by(y - 300.0);
            let (x, y) = point_at(&v, target, 2);
            assert!(v.mouse_move(x, y, now));
            assert_eq!(v.tip.as_ref().map(|t| t.text.as_str()), Some(shown));
            v.ensure_layout(&mut Fake);
            assert!(v.tip_layout.blocks.iter().any(|b| b.text == shown));
            // Above the line, no copy button until pointed at.
            let (b, top) = (v.tip_box, v.tip.as_ref().unwrap().top);
            assert!(b.y + b.h < top && top < y, "{b:?} {top} {y}");
            assert_eq!(v.tip_copy, RectF::default());
        }
        // Up through the gap onto the tooltip: it stays and shows its copy button.
        let b = v.tip_box;
        v.mouse_move(b.x + 5.0, b.y + b.h + 3.0, now);
        v.mouse_move(b.x + 5.0, b.y + b.h / 2.0, now);
        v.ensure_layout(&mut Fake);
        assert!(v.tip.as_ref().is_some_and(|t| t.hot));
        let c = v.tip_copy;
        assert!(c.w > 0.0 && v.tip_box.w > b.w);
        assert!(v.over_button(c.x + c.w / 2.0, c.y + c.h / 2.0));
        assert!(!v.over_link(b.x + 5.0, b.y + b.h / 2.0));
        v.press(c.x + c.w / 2.0, c.y + c.h / 2.0, Mods::default(), Duration::from_millis(500));
        assert!(matches!(v.release(c.x + c.w / 2.0, c.y + c.h / 2.0, false, now), Some(Effect::Copy(t)) if t == "Abstract Syntax Tree"));
        assert!(v.selection.is_none() && v.tip.as_ref().is_some_and(|t| t.copied));
        // Away from link and tooltip, and out of the window: gone.
        v.mouse_move(b.x + 5.0, b.y - 40.0, now);
        assert!(v.tip.is_none());
        let (x, y) = point_at(&v, "abbr:Abstract Syntax Tree", 2);
        v.mouse_move(x, y, now);
        v.scroll_by(10.0);
        assert!(v.tip.is_none(), "scrolling hides it");
        v.mouse_move(x, y - 10.0, now);
        assert!(v.tip.is_some());
        v.wheel(48.0, Mods::default(), x, y - 10.0, Some(now));
        assert!(v.tip.is_none(), "a wheel notch (gliding) hides it too");
        v.mouse_move(x, y - 10.0, now);
        v.mouse_leave();
        assert!(v.tip.is_none());
    }

    #[test]
    fn fresh_layout_is_partial_then_complete() {
        let mut v = viewer(900.0);
        v.ensure_layout(&mut Fake);
        assert!(v.layout_pending());
        assert!(v.layout.blocks.len() < 100);
        while !v.continue_layout(&mut Fake, Duration::ZERO) {}
        assert_eq!(summary(&v.layout), reference(900.0));
    }

    #[test]
    fn relayout_keeps_the_block_at_the_top_and_converges() {
        for finish_by_slices in [false, true] {
            let mut v = viewer(900.0);
            v.ensure_layout(&mut Fake);
            v.finish_layout(&mut Fake);
            v.scroll_by(20_000.0);
            v.ensure_layout(&mut Fake);
            let (node, y) = v.scroll_anchor().unwrap();
            let offset = v.scroll - y;
            let before = v.scroll;
            // Narrower window: provisional layout from the anchor block, same scroll.
            v.resize(700.0, 600.0);
            v.ensure_layout(&mut Fake);
            assert!(v.rebuild.is_some());
            assert_eq!(v.scroll, before);
            assert_eq!((v.layout.tops[0].0, v.layout.tops[0].1), (node, y));
            assert!(v.layout.blocks.len() < 200, "only the view is laid out");
            if finish_by_slices {
                while !v.continue_layout(&mut Fake, Duration::ZERO) {}
            } else {
                v.finish_layout(&mut Fake);
            }
            // Exactly the fresh layout, with the anchor block still at the top of the view.
            assert_eq!(summary(&v.layout), reference(700.0));
            let t = v.layout.tops.iter().find(|t| t.0 == node).unwrap();
            assert!((v.scroll - (t.1 + offset)).abs() < 0.01, "{} vs {}", v.scroll, t.1 + offset);
        }
    }

    #[test]
    fn scrolling_above_a_provisional_layout_completes_it() {
        let mut v = viewer(900.0);
        v.ensure_layout(&mut Fake);
        v.finish_layout(&mut Fake);
        v.scroll_by(20_000.0);
        v.ensure_layout(&mut Fake);
        v.resize(700.0, 600.0);
        v.ensure_layout(&mut Fake);
        v.scroll_by(-15_000.0);
        v.ensure_layout(&mut Fake);
        assert!(v.rebuild.is_none());
        assert_eq!(v.layout.tops[0].0, 0);
    }

    fn laid_out() -> Viewer<FakePara> {
        let mut v = viewer(900.0);
        v.ensure_layout(&mut Fake);
        v.finish_layout(&mut Fake);
        v
    }

    /// `v` changes to a short document while sliding (`back`: the new page from the left).
    fn change_page(v: &mut Viewer<FakePara>, back: bool, now: Instant) {
        v.slide(back, now);
        v.set_document("n".into(), String::new(), crate::app::parse("# Neu"), false);
        v.ensure_layout(&mut Fake);
    }

    #[test]
    fn going_back_slides_the_pages_sideways_with_acceleration() {
        let mut v = laid_out();
        v.scroll_by(500.0);
        let old_blocks = v.layout.blocks.len();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        change_page(&mut v, true, t0);
        let s = v.slide.as_ref().expect("the pages slide");
        assert_eq!((s.layout.blocks.len(), s.scroll), (old_blocks, 500.0), "the leaving page as it was");
        assert!(v.layout.blocks.len() < old_blocks, "the new page laid out");
        let w = v.view_w;
        assert_eq!(v.slide_x(t0), Some((-w, 0.0)), "the previous page waits left of the view");
        let (x, _) = v.slide_x(ms(87)).unwrap();
        assert!(x + w < 0.1 * w, "it starts slowly: {x}");
        let (x, old) = v.slide_x(ms(175)).unwrap();
        assert!((x + w / 2.0).abs() < 1.0 && (old - w / 2.0).abs() < 1.0, "halfway at half time: {x} {old}");
        let (x, _) = v.slide_x(ms(262)).unwrap();
        assert!(-x < 0.1 * w, "and slows down at the end: {x}");
        let (repaint, next) = v.tick(ms(100));
        assert!(repaint && next == Some(FRAME), "frames while sliding");
        v.tick(ms(400));
        assert!(v.slide.is_none() && v.slide_x(ms(400)).is_none(), "done, the leaving page dropped");
    }

    #[test]
    fn going_forward_slides_the_other_way_reduced_motion_not_at_all() {
        let mut v = laid_out();
        let t0 = Instant::now();
        change_page(&mut v, false, t0);
        let w = v.view_w;
        assert_eq!(v.slide_x(t0), Some((w, 0.0)), "the next page waits right of the view");
        let (x, old) = v.slide_x(t0 + Duration::from_millis(175)).unwrap();
        assert!((x - w / 2.0).abs() < 1.0 && (old + w / 2.0).abs() < 1.0, "{x} {old}");
        let mut v = laid_out();
        v.reduce_motion = true;
        change_page(&mut v, true, t0);
        assert!(v.slide.is_none() && v.slide_x(t0).is_none());
    }

    /// Measured on Windows: each notch of a fast spin restarted the glide at its own time, so
    /// the frame painted for it at once barely moved (32.1 → 32.3 px) and the view lagged.
    #[test]
    fn a_notch_moves_the_view_in_its_own_frame_even_while_gliding() {
        let mut v = laid_out();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        v.wheel(48.0, Mods::default(), 400.0, 300.0, Some(t0));
        v.tick(t0);
        assert!(v.scroll >= 0.2 * 48.0, "the first frame answers the notch: {}", v.scroll);
        // Notches every 30 ms: every frame painted for one moves on (at least half the
        // average speed of the spin, 48 px / 30 ms).
        let mut last = v.scroll;
        for n in 1..10u64 {
            v.tick(ms(n * 30 - 15));
            v.wheel(48.0, Mods::default(), 400.0, 300.0, Some(ms(n * 30)));
            v.tick(ms(n * 30));
            let step = v.scroll - last;
            assert!(step >= 0.5 * 48.0 / 2.0, "notch {n}: {last} → {} in its 15 ms", v.scroll);
            last = v.scroll;
        }
        assert!(480.0 - v.scroll <= 2.0 * 48.0, "at most two notches behind the wheel: {}", v.scroll);
    }

    #[test]
    fn wheel_notches_glide_to_their_target() {
        let mut v = laid_out();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        v.wheel(100.0, Mods::default(), 400.0, 300.0, Some(t0));
        assert_eq!(v.scroll, 0.0, "the view follows in tick");
        v.wheel(100.0, Mods::default(), 400.0, 300.0, Some(t0));
        let (repaint, next) = v.tick(ms(30));
        assert!(repaint && next == Some(FRAME));
        let a = v.scroll;
        v.tick(ms(90));
        assert!(0.0 < a && a < v.scroll && v.scroll < 200.0, "{a} {}", v.scroll);
        v.tick(ms(400));
        assert_eq!(v.scroll, 200.0, "both notches add up");
        assert!(v.glide.is_none());
        // Turning back starts from the view, not from the target ahead.
        v.wheel(300.0, Mods::default(), 0.0, 0.0, Some(ms(400)));
        v.tick(ms(450));
        let at = v.scroll;
        v.wheel(-100.0, Mods::default(), 0.0, 0.0, Some(ms(450)));
        v.tick(ms(1000));
        assert!((v.scroll - (at - 100.0)).abs() < 0.01, "{} vs {}", v.scroll, at - 100.0);
        // Other scrolling ends a glide.
        v.wheel(500.0, Mods::default(), 0.0, 0.0, Some(ms(1000)));
        v.tick(ms(1030));
        v.scroll_by(-10.0);
        let at = v.scroll;
        v.tick(ms(1100));
        assert_eq!(v.scroll, at);
        // A glide stops at the end of the document.
        v.wheel(1e7, Mods::default(), 0.0, 0.0, Some(ms(2000)));
        v.tick(ms(3000));
        assert_eq!(v.scroll, v.layout.height - v.view_h);
        assert!(v.glide.is_none());
    }

    #[test]
    fn scroll_bar_shows_while_scrolling_then_fades() {
        let mut v = laid_out();
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        v.tick(t0);
        assert!(v.scroll_bar().is_none());
        v.scroll_by(1000.0);
        assert!(v.scroll_moved());
        v.tick(ms(1));
        v.tick(ms(200));
        let (r, ink) = v.scroll_bar().expect("shown while scrolling");
        assert!(ink.0[3] > 0 && r.x + r.w < v.view_w && r.h >= 32.0 && r.h < v.view_h);
        let max = v.layout.height - v.view_h;
        let track = v.view_h - 6.0 - r.h;
        assert!((r.y - 3.0 - track * 1000.0 / max).abs() < 0.5, "knob at the scroll position");
        // Still shown a little after the last scroll, gone a while later.
        let (_, next) = v.tick(ms(500));
        assert!(v.scroll_bar().is_some() && next.is_some());
        for t in (520..=1800).step_by(20) {
            v.tick(ms(t));
        }
        assert!(v.scroll_bar().is_none());
        assert_eq!(v.bar_alpha, 0.0);
        assert!(v.tick(ms(1900)).1.is_none_or(|d| d > Duration::from_millis(100)), "no frames once it is gone");
        // A short document has none.
        let mut short = Viewer::<FakePara>::new("t".into(), String::new(), crate::app::parse("# Hi"));
        short.resize(900.0, 600.0);
        short.ensure_layout(&mut Fake);
        short.scroll_by(100.0);
        short.tick(t0);
        short.tick(ms(200));
        assert!(short.scroll_bar().is_none());
    }

    /// The fake typesetter with images: the `ready` ones have a size (400×200), `asked`
    /// lists the loads started, in order, each once.
    #[derive(Default)]
    struct Pics {
        ready: Vec<String>,
        asked: Vec<String>,
    }

    impl Typesetter for Pics {
        type Para = FakePara;
        fn paragraph(&mut self, text: &str, spans: &[(Range<usize>, crate::layout::Span)], base: &BaseStyle, w: f32) -> FakePara {
            Fake.paragraph(text, spans, base, w)
        }
        fn image_size(&self, url: &str) -> Option<(f32, f32)> {
            self.ready.iter().any(|u| u == url).then_some((400.0, 200.0))
        }
        fn load_image(&mut self, url: &str) {
            if !self.asked.iter().any(|u| u == url) {
                self.asked.push(url.to_string());
            }
        }
    }

    /// near.png at the top, far.png some 35 screens further down, an inline one after it.
    fn pictures() -> Viewer<FakePara> {
        let src = ["![n](near.png)\n\n", &"Absatz\n\n".repeat(600), "![f](far.png)\n\n", &"Absatz\n\n".repeat(40), "Text ![i](inline.png) Text\n\n", &"Absatz\n\n".repeat(40)].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        v
    }

    fn pending_y(v: &Viewer<FakePara>, url: &str) -> f32 {
        v.layout.pending.iter().find(|p| p.url == url).unwrap_or_else(|| panic!("{url} waits")).r.y
    }

    fn scroll_to(v: &mut Viewer<FakePara>, ts: &mut Pics, y: f32) {
        v.scroll_by(y - v.scroll);
        v.ensure_layout(ts);
    }

    #[test]
    fn images_load_only_once_the_view_comes_near() {
        let mut v = pictures();
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        v.ensure_layout(&mut ts);
        assert_eq!(ts.asked, ["near.png"], "the one in view, not those far below (the whole document is laid out)");
        let h = v.view_h;
        let far = pending_y(&v, "far.png");
        scroll_to(&mut v, &mut ts, far - 3.0 * h);
        assert_eq!(ts.asked, ["near.png"], "two screens above it: not yet");
        scroll_to(&mut v, &mut ts, far - 1.8 * h);
        assert_eq!(ts.asked, ["near.png", "far.png"], "within a screen below the view: now");
        let inline = pending_y(&v, "inline.png");
        scroll_to(&mut v, &mut ts, inline + 1.5 * h);
        assert_eq!(ts.asked.len(), 2, "1.5 screens below it: not yet");
        scroll_to(&mut v, &mut ts, inline + 0.8 * h);
        assert_eq!(ts.asked, ["near.png", "far.png", "inline.png"], "an inline image within a screen above the view");
    }

    #[test]
    fn a_loaded_image_fades_in() {
        let mut v = pictures();
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        assert_eq!(ts.asked, ["near.png"]);
        let t0 = Instant::now();
        let ms = |n: u64| t0 + Duration::from_millis(n);
        ts.ready.push("near.png".into());
        v.invalidate(); // the platform's "an image arrived"
        v.ensure_layout(&mut ts);
        assert!(v.layout.images.iter().any(|i| i.url == "near.png") && v.layout.pending.iter().all(|p| p.url != "near.png"));
        assert_eq!(v.image_alpha("near.png"), 0.0, "not shown before its fade starts");
        let (repaint, next) = v.tick(t0);
        assert!(repaint && next == Some(FRAME), "frames while it fades in");
        assert_eq!(v.image_alpha("near.png"), 0.0, "it starts transparent");
        v.tick(ms(120));
        let a = v.image_alpha("near.png");
        assert!(a > 0.3 && a < 0.95, "halfway: {a}");
        let (repaint, _) = v.tick(ms(260));
        assert!(repaint);
        assert_eq!(v.image_alpha("near.png"), 1.0, "in within 260 ms");
        assert_eq!(v.tick(ms(280)).1, None, "and no more frames");
        // Relayouts (another image arrived, a resize) do not fade it again.
        v.invalidate();
        v.resize(800.0, 600.0);
        v.ensure_layout(&mut ts);
        assert_eq!(v.image_alpha("near.png"), 1.0);
        assert_eq!(v.tick(ms(300)).1, None);
    }

    #[test]
    fn an_image_loaded_already_shows_at_once() {
        let mut v = pictures();
        let mut ts = Pics { ready: vec!["near.png".into()], ..Default::default() };
        v.ensure_layout(&mut ts);
        assert!(ts.asked.is_empty(), "nothing to load");
        assert_eq!(v.image_alpha("near.png"), 1.0);
        assert_eq!(v.tick(Instant::now()), (false, None));
    }

    #[test]
    fn with_reduced_motion_a_loaded_image_shows_at_once() {
        let mut v = pictures();
        v.reduce_motion = true;
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        ts.ready.push("near.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        assert_eq!(v.image_alpha("near.png"), 1.0);
        assert_eq!(v.tick(Instant::now()).1, None);
    }

    /// Loaded ahead (a screen below the view), it fades in when it comes into view.
    #[test]
    fn an_image_loaded_ahead_fades_in_as_it_comes_into_view() {
        let mut v = pictures();
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        let h = v.view_h;
        let far = pending_y(&v, "far.png");
        scroll_to(&mut v, &mut ts, far - 1.5 * h);
        assert!(ts.asked.iter().any(|u| u == "far.png"));
        ts.ready.push("far.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        let t0 = Instant::now();
        v.tick(t0);
        v.tick(t0 + Duration::from_millis(1000));
        assert_eq!(v.image_alpha("far.png"), 0.0, "no fade out of view");
        let far = v.layout.images.iter().find(|i| i.url == "far.png").expect("placed").r.y;
        scroll_to(&mut v, &mut ts, far - h + 50.0);
        let t1 = t0 + Duration::from_millis(1500);
        v.tick(t1);
        assert_eq!(v.image_alpha("far.png"), 0.0, "its fade starts as it shows");
        v.tick(t1 + Duration::from_millis(120));
        assert!(v.image_alpha("far.png") > 0.3);
        v.tick(t1 + Duration::from_millis(260));
        assert_eq!(v.image_alpha("far.png"), 1.0);
    }

    /// At the end of the document (End), an image arriving there keeps the view at the end:
    /// the document grows and the view goes with it. Elsewhere the view stays where it was.
    #[test]
    fn at_the_end_an_arriving_image_keeps_the_view_at_the_end() {
        let src = ["# Oben\n\n", &"Absatz\n\n".repeat(100), "![e](end.png)\n"].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        let h = v.view_h;
        // A screen above the end: the image is asked for, arrives, the view stays.
        let above = v.layout.height - 2.0 * h;
        scroll_to(&mut v, &mut ts, above);
        assert_eq!(ts.asked, ["end.png"]);
        let top = v.layout.blocks[v.layout.hit(0.0, v.scroll).unwrap().block].text.clone();
        ts.ready.push("end.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        assert_eq!(v.scroll, above, "not at the end: the view stays");
        assert_eq!(v.layout.blocks[v.layout.hit(0.0, v.scroll).unwrap().block].text, top);
        // At the end: the image arrives and the view stays at the (new) end.
        ts.ready.clear();
        v.set_document("t".into(), String::new(), crate::app::parse(&src), false);
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        v.key(Key::End, Mods::default(), Instant::now());
        v.ensure_layout(&mut ts);
        let end = v.layout.height - h;
        assert_eq!(v.scroll, end, "at the end");
        ts.ready.push("end.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        assert!(v.layout.height - h > end + 50.0, "the document grew");
        assert_eq!(v.scroll, v.layout.height - h, "the view is at the end again");
        let img = v.layout.images.iter().find(|i| i.url == "end.png").unwrap().r;
        assert!(img.y + img.h <= v.scroll + h, "the image all in view");
    }

    /// End jumps over the middle of a document laid out only at its top: the images at the
    /// end load, the one passed over does not.
    #[test]
    fn end_loads_the_images_at_the_end_not_those_passed_over() {
        let src = ["![n](near.png)\n\n", &"Absatz\n\n".repeat(600), "![m](mid.png)\n\n", &"Absatz\n\n".repeat(600), "![e](end.png)\n"].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        assert!(v.resume.is_some(), "only the top is laid out");
        v.key(Key::End, Mods::default(), Instant::now());
        v.ensure_layout(&mut ts);
        assert_eq!(v.scroll, v.layout.height - v.view_h, "at the end");
        assert_eq!(ts.asked, ["near.png", "end.png"]);
    }

    /// An image that fails to load keeps its placeholder: it does not fade, keeps no frames
    /// coming, is not asked for again and does not hold up another image's fade.
    #[test]
    fn a_failed_image_keeps_its_placeholder() {
        let src = ["![a](broken.png)\n\n![b](fine.png)\n\n", &"Absatz\n\n".repeat(100)].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        assert_eq!(ts.asked, ["broken.png", "fine.png"]);
        let placeholder = v.layout.pending.iter().find(|p| p.url == "broken.png").expect("waits").r;
        // broken.png failed (the platform stops there, no relayout); fine.png arrives.
        ts.ready.push("fine.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        assert!(v.layout.images.iter().all(|i| i.url != "broken.png"), "never placed");
        assert_eq!(v.layout.pending.iter().find(|p| p.url == "broken.png").map(|p| p.r), Some(placeholder), "its placeholder stays");
        assert_eq!(v.image_alpha("broken.png"), 1.0, "nothing fades for it");
        assert_eq!(v.image_alpha("fine.png"), 0.0, "the other one waits for its fade");
        let t0 = Instant::now();
        v.tick(t0);
        v.tick(t0 + Duration::from_millis(260));
        assert_eq!(v.image_alpha("fine.png"), 1.0, "and fades in");
        assert!(!v.image_fade_waiting());
        assert_eq!(v.tick(t0 + Duration::from_millis(300)), (false, None), "no frames for the broken one");
        // More relayouts and scrolling do not ask again or start anything.
        v.invalidate();
        v.ensure_layout(&mut ts);
        scroll_to(&mut v, &mut ts, 100.0);
        scroll_to(&mut v, &mut ts, 0.0);
        assert_eq!(ts.asked, ["broken.png", "fine.png"]);
        assert!(!v.image_fade_waiting());
        assert_eq!(v.tick(t0 + Duration::from_millis(400)).1, None);
    }

    /// At the end, a relayout with nothing new (a failed image, a theme change) leaves the
    /// view exactly at the end.
    #[test]
    fn at_the_end_a_failed_image_leaves_the_view_where_it_is() {
        let src = ["# Oben\n\n", &"Absatz\n\n".repeat(100), "![e](broken.png)\n"].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.key(Key::End, Mods::default(), Instant::now());
        v.ensure_layout(&mut ts);
        let end = v.scroll;
        assert_eq!(end, v.layout.height - v.view_h);
        assert_eq!(ts.asked, ["broken.png"]);
        for _ in 0..3 {
            v.invalidate();
            v.ensure_layout(&mut ts);
            v.finish_layout(&mut ts);
            assert_eq!(v.scroll, end);
            assert_eq!(v.scroll, v.layout.height - v.view_h);
        }
    }

    /// At the end, an image arriving a screen above the view grows the document above it:
    /// the view stays at the end.
    #[test]
    fn at_the_end_an_image_arriving_above_the_view_keeps_it_at_the_end() {
        let src = ["# Oben\n\n", &"Absatz\n\n".repeat(100), "![a](above.png)\n\n", &"Absatz\n\n".repeat(20)].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.key(Key::End, Mods::default(), Instant::now());
        v.ensure_layout(&mut ts);
        let h = v.view_h;
        let p = v.layout.pending.iter().find(|p| p.url == "above.png").expect("waits").r;
        assert!(p.y + p.h <= v.scroll && p.y >= v.scroll - h, "a screen above the view: {} {} (view at {})", p.y, p.h, v.scroll);
        assert_eq!(ts.asked, ["above.png"]);
        let last = v.layout.blocks.last().unwrap().text.clone();
        let height = v.layout.height;
        ts.ready.push("above.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        assert!(v.layout.height > height + 50.0, "the document grew");
        assert_eq!(v.scroll, v.layout.height - h, "the view is at the end");
        assert_eq!(v.layout.blocks.last().unwrap().text, last);
    }

    /// A document shorter than the view: an image arriving makes it longer, the view stays
    /// at the top (where the reader is), not at the new end.
    #[test]
    fn a_short_document_that_grows_stays_at_the_top() {
        let src = ["# Oben\n\n", &"Absatz\n\n".repeat(10), "![e](end.png)\n"].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        assert!(v.layout.height <= v.view_h, "fits: {}", v.layout.height);
        v.key(Key::End, Mods::default(), Instant::now());
        assert_eq!(v.scroll, 0.0);
        ts.ready.push("end.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        assert!(v.layout.height > v.view_h, "longer than the view now: {}", v.layout.height);
        assert_eq!(v.scroll, 0.0);
    }

    /// A wheel glide on its way to the end when the image there arrives: the glide goes on
    /// to the new end (as if the image had arrived a moment later, once it got there).
    #[test]
    fn a_glide_to_the_end_goes_to_the_new_end_when_an_image_arrives() {
        let src = ["# Oben\n\n", &"Absatz\n\n".repeat(100), "![e](end.png)\n"].concat();
        let mut v = Viewer::new("t".into(), String::new(), crate::app::parse(&src));
        v.resize(900.0, 600.0);
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        let h = v.view_h;
        let end = v.layout.height - h;
        scroll_to(&mut v, &mut ts, end - 300.0);
        assert_eq!(ts.asked, ["end.png"]);
        let t0 = Instant::now();
        v.wheel(400.0, Mods::default(), 400.0, 300.0, Some(t0));
        v.tick(t0 + Duration::from_millis(40));
        assert!(v.scroll > end - 300.0 && v.scroll < end, "on its way: {}", v.scroll);
        ts.ready.push("end.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        assert!(v.layout.height - h > end + 50.0, "the document grew");
        let before = v.scroll;
        v.tick(t0 + Duration::from_millis(60));
        assert!(v.scroll >= before, "no step back: {before} → {}", v.scroll);
        v.tick(t0 + Duration::from_millis(400));
        assert_eq!(v.scroll, v.layout.height - h, "at the new end");
        let img = v.layout.images.iter().find(|i| i.url == "end.png").unwrap().r;
        assert!(img.y + img.h <= v.scroll + h, "the image all in view");
        // A glide that stops short of the end is not pulled there.
        v.set_document("t".into(), String::new(), crate::app::parse(&src), false);
        ts.ready.clear();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        scroll_to(&mut v, &mut ts, end - 500.0);
        v.wheel(100.0, Mods::default(), 400.0, 300.0, Some(t0));
        v.tick(t0 + Duration::from_millis(40));
        ts.ready.push("end.png".into());
        v.invalidate();
        v.ensure_layout(&mut ts);
        v.finish_layout(&mut ts);
        v.tick(t0 + Duration::from_millis(400));
        assert_eq!(v.scroll, end - 400.0);
    }

    /// A new document starts afresh: its images load near the view again.
    #[test]
    fn a_new_document_asks_again() {
        let mut v = pictures();
        let mut ts = Pics::default();
        v.ensure_layout(&mut ts);
        ts.asked.clear(); // the platform dropped its images with the new document
        v.set_document("n".into(), String::new(), crate::app::parse("![n](near.png)"), false);
        v.ensure_layout(&mut ts);
        assert_eq!(ts.asked, ["near.png"]);
    }
}

#[cfg(test)]
mod help_tests {
    /// Every language has exactly one text per HELP heading and row, plus title, hint and
    /// the two words.
    #[test]
    fn help_texts_match_keys() {
        let want = 4 + super::HELP.iter().map(|(_, _, rows)| 1 + rows.len()).sum::<usize>() + super::UI_COUNT;
        for code in crate::tables::HELP_LANGUAGES {
            assert_eq!(crate::tables::help(code).lines().count(), want, "{code}");
        }
    }

    /// Keys and words are filled in (English key names without a platform namer).
    #[test]
    fn help_keys_filled_in() {
        assert_eq!(super::help_keys("{Ctrl}+{#0}", &["Mausrad", "Datei"]), "Ctrl+Mausrad");
        assert_eq!(super::help_keys("{PgDn}   →   {Space}", &[]), "Page Down   →   Space");
        assert_eq!(super::help_keys("markdownr {#1}.md", &["x", "Datei"]), "markdownr Datei.md");
        assert_eq!(super::help_keys("F1", &[]), "F1");
    }
}
