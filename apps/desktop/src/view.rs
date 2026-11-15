//! Platform-neutral viewer state and input handling. Platform shells translate
//! native events into calls here and carry out the returned [`Effect`]s.
//!
//! Besides the document this owns the reading-mode chrome: a toolbar that fades
//! in on mouse movement at the top edge (or a tap) and out again, the table of
//! contents drawer and the find bar. The chrome is laid out with the same
//! typesetter as the document, in window coordinates.

use crate::ast::{Kind, Node};
use crate::layout::{BaseStyle, DocLayout, DocPos, DocSelection, Join, Layouter, Paragraph, Rect, RectF, Resume, TextBlock, Typesetter};
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

/// F1 overlay: (column, [keys of each row]). Its texts (title, hint, then each section's
/// heading and row descriptions in this order) are `tables::help()`, shipped compressed.
const HELP: &[(usize, &[&str])] = &[
    (0, &["Bild ↓   →   Leertaste", "Bild ↑   ←   ⇧ Leertaste", "↓   ↑   Mausrad", "Pos1   Ende", "Strg+A   Strg+C"]),
    (0, &["Strg+F   F3", "Enter   ⇧ Enter", "Esc"]),
    (0, &["Strg+O", "F5   Strg+R", "Strg+Q   Strg+W"]),
    (0, &["markdownr Datei.md", "markdownr"]),
    (1, &["Strg+T", "Strg+U", "Strg+D", "Strg++   Strg+−", "Strg+Mausrad", "Strg+0", "F11   Esc", "F1"]),
    (1, &["MARKDOWNR_SETTINGS=Datei", "MARKDOWNR_SAMPLE=1", "MARKDOWNR_LOCALE=de-DE"]),
];

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
    pub view_w: f32,
    pub view_h: f32,
    pub selection: Option<DocSelection>,
    pub fullscreen: bool,
    pub remember_fullscreen: bool,
    pub show_source: bool,
    /// Toolbar, drawer and find bar in window coordinates.
    pub chrome: DocLayout<P>,
    pub chrome_alpha: f32,
    /// Abbreviation tooltip (text, window x, y) and its layout.
    tip: Option<(String, f32, f32)>,
    pub tip_layout: DocLayout<P>,
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
            view_w: 800.0,
            view_h: 600.0,
            selection: None,
            fullscreen: false,
            remember_fullscreen: false,
            show_source: false,
            chrome: DocLayout::default(),
            chrome_alpha: 0.0,
            tip: None,
            tip_layout: DocLayout::default(),
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
            find: None,
            dirty: true,
            dragging: false,
            press: (0.0, 0.0),
            pressed_chrome: false,
            last_click: None,
            last_layout_time: Duration::ZERO,
            resume: None,
            rebuild: None,
        };
        v.title = v.display_title(title);
        v
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
        let base = match fm {
            Some(name) if auto => Theme::named(name, self.dark),
            _ => Theme::named(self.theme_choice.name(), self.dark),
        };
        if auto { base.with_overrides(&self.front_matter) } else { base }
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
            // Keep the reading position: the block at the top of the view stays there.
            let anchor = self.scroll_anchor();
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
                            self.scroll += t.1 - y0;
                        }
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
        self.scroll += r.at.y - r.y0;
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

    fn clamp_scroll(&mut self) {
        // While the layout is incomplete the end is not known yet.
        let max = if self.resume.is_some() || self.rebuild.is_some() { f32::INFINITY } else { (self.layout.height - self.view_h).max(0.0) };
        self.scroll = self.scroll.clamp(0.0, max);
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll += dy;
        self.clamp_scroll();
        if self.toc_open {
            self.chrome_dirty = true; // current-section highlight
        }
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
        let visible = self.pinned() || self.show_until.is_some_and(|t| t > now);
        let target = if visible { 1.0 } else { 0.0 };
        let before = self.chrome_alpha;
        if self.chrome_alpha < target {
            self.chrome_alpha = (self.chrome_alpha + dt / FADE_IN).min(1.0);
        } else if self.chrome_alpha > target {
            self.chrome_alpha = (self.chrome_alpha - dt / FADE_OUT).max(0.0);
        }
        let animating = self.chrome_alpha != target;
        if !animating {
            self.last_tick = None;
        }
        let next = if animating {
            Some(Duration::from_millis(16))
        } else if visible && !self.pinned() {
            self.show_until.map(|t| t.saturating_duration_since(now) + Duration::from_millis(1))
        } else {
            None
        };
        let mut repaint = before != self.chrome_alpha;
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
        let mut x = 6.0;
        x += button(ts, &mut out, &mut buttons, "☰  Inhalt", Action::Toc, x, false, self.toc_open) + 2.0;
        x += button(ts, &mut out, &mut buttons, "Öffnen", Action::Open, x, false, false) + 2.0;
        let left_end = x;

        // Right group, laid out from the right edge.
        let mut rx = w - 6.0;
        let fs = if self.fullscreen { "⤡  Fenster" } else { "⤢  Vollbild" };
        rx -= button(ts, &mut out, &mut buttons, fs, Action::Fullscreen, rx, true, false) + 2.0;
        if self.fullscreen {
            let pin = if self.remember_fullscreen { "☑ Beim Start" } else { "☐ Beim Start" };
            rx -= button(ts, &mut out, &mut buttons, pin, Action::RememberFullscreen, rx, true, false) + 2.0;
        }
        rx -= button(ts, &mut out, &mut buttons, &self.theme_choice.label(), Action::Theme, rx, true, false) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "</>", Action::Source, rx, true, self.show_source) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "A+", Action::FontUp, rx, true, false) + 2.0;
        rx -= button(ts, &mut out, &mut buttons, "A−", Action::FontDown, rx, true, false) + 2.0;
        if self.find.is_none() {
            rx -= button(ts, &mut out, &mut buttons, "⌕  Suchen", Action::Search, rx, true, false) + 2.0;
        }

        // Centre: title or find field.
        let avail = (rx - left_end - 16.0).max(0.0);
        if let Some(find) = &self.find {
            let count = if find.query.is_empty() {
                String::new()
            } else if find.matches.is_empty() {
                "keine Treffer".to_string()
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
            let title = if self.show_source { [&self.title, " (Quelltext)"].concat() } else { self.title.clone() };
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
                let text = ["Saison-Theme: ".into(), crate::theme::theme_def(name).map(|d| d.label()).unwrap_or_default()].concat();
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
                let (para, pw, ph) = label(ts, "Keine Überschriften", &muted, dw - 32.0);
                place(&mut out, para, "Keine Überschriften", 16.0, y + 8.0, pw, ph);
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
        if let Some((text, tx, ty)) = &self.tip {
            let (para, pw, ph) = label(ts, text, &style, 360.0);
            let (bw, bh) = (pw + 20.0, ph + 12.0);
            let bx = (tx - bw / 2.0).clamp(4.0, (self.view_w - bw - 4.0).max(4.0));
            let by = if ty + 24.0 + bh < self.view_h { ty + 24.0 } else { ty - 12.0 - bh };
            tip_out.rects.push(Rect { r: RectF { x: bx - 1.0, y: by - 1.0, w: bw + 2.0, h: bh + 2.0 }, color: th.border, radius: 7.0 });
            tip_out.rects.push(Rect { r: RectF { x: bx, y: by, w: bw, h: bh }, color: th.chrome_bg, radius: 6.0 });
            place(&mut tip_out, para, text, bx + 10.0, by + 6.0, pw, ph);
        }
        if self.help {
            tip_out = self.build_help(ts);
        }
        self.tip_layout = tip_out;
        self.buttons = buttons;
    }

    /// F1 overlay: the page dimmed, on it a panel with the keyboard shortcuts and the
    /// command line, in two columns when the window is wide enough.
    fn build_help<T: Typesetter<Para = P>>(&self, ts: &mut T) -> DocLayout<P> {
        let th = &self.theme;
        let (w, h) = (self.view_w, self.view_h);
        let text = BaseStyle { size: UI_SIZE, color: th.chrome_text, mono: false, bold: false, italic: false, line_height: 1.3 };
        let keys = BaseStyle { bold: true, ..text };
        let head = BaseStyle { color: th.link, ..keys };
        let two = w >= 860.0;
        let col_w = if two { 380.0 } else { (w - 64.0).clamp(220.0, 460.0) };
        let (gap, key_w) = (36.0, 158.0);
        // (paragraph, text, x, y, w, h) relative to the panel's content origin.
        let mut items: Vec<(P, &str, f32, f32, f32, f32)> = Vec::new();
        let mut put = |ts: &mut T, s: &'static str, style: &BaseStyle, x: f32, y: f32, max_w: f32| {
            let para = ts.paragraph(s, &[], style, max_w);
            let (pw, ph) = para.size();
            items.push((para, s, x, y, pw, ph));
            (pw, ph)
        };
        let mut texts = crate::tables::help().lines();
        let mut next = || texts.next().unwrap_or("");
        let (title_s, hint_s) = (next(), next());
        let mut col_h = [0.0f32; 2];
        for &(col, rows) in HELP {
            let c = if two { col } else { 0 };
            let x0 = c as f32 * (col_w + gap);
            let mut y = if col_h[c] > 0.0 { col_h[c] + 14.0 } else { 0.0 };
            y += put(ts, next(), &head, x0, y, col_w).1 + 6.0;
            for &k in rows {
                let d = next();
                let (kw, kh) = put(ts, k, &keys, x0, y, col_w);
                // Long keys (commands, variables) get the description on the next line.
                let (dx, dy) = if kw + 12.0 <= key_w { (x0 + key_w, y) } else { (x0 + 16.0, y + kh) };
                let dh = put(ts, d, &text, dx, dy, col_w - (dx - x0)).1;
                y = (y + kh).max(dy + dh) + 5.0;
            }
            col_h[c] = y;
        }
        let pad = 24.0;
        let content_w = if two { 2.0 * col_w + gap } else { col_w };
        let title_p = ts.paragraph(title_s, &[], &BaseStyle { size: 18.0, ..keys }, content_w);
        let (tw, tht) = title_p.size();
        let hint_p = ts.paragraph(hint_s, &[], &BaseStyle { color: th.muted, ..text }, content_w);
        let (hw, hh) = hint_p.size();
        let (pw, ph) = (content_w + 2.0 * pad, pad + tht + 14.0 + col_h[0].max(col_h[1]) - 5.0 + pad);
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
            place(para, s, px + pad + x, top + y, bw, bh);
        }
        out
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
                    self.scroll = e.y - self.font_size;
                    self.clamp_scroll();
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

    pub fn finding(&self) -> bool {
        self.find.is_some()
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
            self.scroll = y - self.view_h / 3.0;
            self.clamp_scroll();
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

    pub fn wheel(&mut self, dy: f32, mods: Mods, x: f32, y: f32) {
        if self.help {
            return;
        }
        if mods.ctrl {
            self.set_font_size(self.font_size + if dy < 0.0 { 1.0 } else { -1.0 });
        } else if self.toc_open && self.chrome_visible() && x < self.drawer_width() && y > BAR_H {
            let max = (self.chrome.height - (self.view_h - BAR_H)).max(0.0);
            self.toc_scroll = (self.toc_scroll + dy).clamp(0.0, max);
            self.chrome_dirty = true;
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
        self.pressed_chrome = self.over_chrome(x, y);
        if self.pressed_chrome {
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
            let (dx, dy) = self.doc_point(x, y);
            let tip = if self.over_chrome(x, y) { None } else { self.layout.link_at(dx, dy).and_then(|u| u.strip_prefix("abbr:")).map(str::to_string) };
            if tip.as_deref() != self.tip.as_ref().map(|t| t.0.as_str()) {
                self.tip = tip.map(|t| (t, x, y));
                self.chrome_dirty = true;
                repaint = true;
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
        if self.hover.take().is_some() {
            self.chrome_dirty = true;
        }
    }

    /// `tap` is true for touch input: a tap on the page toggles the chrome.
    pub fn release(&mut self, x: f32, y: f32, tap: bool, now: Instant) -> Option<Effect> {
        self.dragging = false;
        if self.help {
            // A click anywhere closes the help.
            self.help = false;
            self.chrome_dirty = true;
            return None;
        }
        let moved = (x - self.press.0).abs() > 3.0 || (y - self.press.1).abs() > 3.0;
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
        if let Some(title) = url.strip_prefix("abbr:") {
            self.tip = Some((title.to_string(), x, y));
            self.chrome_dirty = true;
            return None;
        }
        if let Some(id) = url.strip_prefix('#') {
            let target = self.layout.toc.iter().find(|e| e.id == id).map(|e| e.y - self.font_size)
                .or_else(|| self.layout.anchors.iter().find(|(a, _)| a == id).map(|(_, ay)| *ay));
            if let Some(ty) = target {
                self.scroll = ty;
                self.clamp_scroll();
                self.selection = None;
            }
            None
        } else {
            Some(Effect::OpenUrl(url))
        }
    }

    pub fn over_link(&self, x: f32, y: f32) -> bool {
        if self.help || self.over_chrome(x, y) {
            return false;
        }
        let (dx, dy) = self.doc_point(x, y);
        self.layout.link_at(dx, dy).is_some()
    }

    /// Cursor hint: true over chrome buttons.
    pub fn over_button(&self, x: f32, y: f32) -> bool {
        !self.help && self.chrome_hit(x, y).is_some()
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
}

#[cfg(test)]
mod help_tests {
    /// data/help.txt has exactly one text per HELP heading and row, plus title and hint.
    #[test]
    fn help_texts_match_keys() {
        let want = 2 + super::HELP.iter().map(|(_, rows)| 1 + rows.len()).sum::<usize>();
        assert_eq!(crate::tables::help().lines().count(), want);
    }
}
