//! Platform-neutral viewer state and input handling. Platform shells translate
//! native events into calls here and carry out the returned [`Effect`]s.

use crate::ast::Node;
use crate::layout::{DocLayout, DocSelection, Layouter, Paragraph, Typesetter};
use crate::theme::Theme;
use std::time::{Duration, Instant};

pub const DEFAULT_FONT_SIZE: f32 = 16.0;
const MAX_COLUMN: f32 = 46.0; // in multiples of the font size

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    PageDown,
    PageUp,
    Space,
    Down,
    Up,
    Home,
    End,
    F11,
    F12,
    Escape,
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
    Snapshot,
    OpenUrl(String),
    Quit,
}

pub struct Viewer<P> {
    pub doc: Node,
    pub title: String,
    pub theme: Theme,
    pub dark: bool,
    pub font_size: f32,
    pub layout: DocLayout<P>,
    pub scroll: f32,
    pub view_w: f32,
    pub view_h: f32,
    pub selection: Option<DocSelection>,
    dirty: bool,
    dragging: bool,
    press: (f32, f32),
    last_click: Option<(Instant, (f32, f32), u8)>,
    pub last_layout_time: Duration,
}

impl<P: Paragraph> Viewer<P> {
    pub fn new(title: String, doc: Node) -> Self {
        Viewer {
            doc,
            title,
            theme: Theme::light(),
            dark: false,
            font_size: DEFAULT_FONT_SIZE,
            layout: DocLayout::default(),
            scroll: 0.0,
            view_w: 800.0,
            view_h: 600.0,
            selection: None,
            dirty: true,
            dragging: false,
            press: (0.0, 0.0),
            last_click: None,
            last_layout_time: Duration::ZERO,
        }
    }

    pub fn set_document(&mut self, title: String, doc: Node) {
        self.title = title;
        self.doc = doc;
        self.scroll = 0.0;
        self.selection = None;
        self.dirty = true;
    }

    pub fn set_dark(&mut self, dark: bool) {
        if dark != self.dark {
            self.dark = dark;
            self.dirty = true;
        }
    }

    pub fn resize(&mut self, w: f32, h: f32) {
        if w != self.view_w {
            self.dirty = true;
        }
        self.view_w = w;
        self.view_h = h;
        self.clamp_scroll();
    }

    /// Force a relayout, e.g. after the platform lost its rendering device.
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub fn ensure_layout<T: Typesetter<Para = P>>(&mut self, ts: &mut T) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let ratio = if self.layout.height > 0.0 { self.scroll / self.layout.height } else { 0.0 };
        self.theme = if self.dark { Theme::dark() } else { Theme::light() };
        let w = self.view_w;
        let margin = (self.font_size * 2.0).min(w * 0.06).max(12.0);
        let column = (w - 2.0 * margin).min(self.font_size * MAX_COLUMN).max(100.0);
        let x = ((w - column) / 2.0).max(margin).round();
        let started = Instant::now();
        self.layout = Layouter { ts, theme: &self.theme, base: self.font_size }.layout(&self.doc, x, column, self.font_size * 1.5);
        self.last_layout_time = started.elapsed();
        self.selection = None;
        self.scroll = ratio * self.layout.height;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let max = (self.layout.height - self.view_h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll += dy;
        self.clamp_scroll();
    }

    pub fn set_font_size(&mut self, size: f32) {
        let size = size.clamp(10.0, 40.0);
        if size != self.font_size {
            self.font_size = size;
            self.dirty = true;
        }
    }

    pub fn key(&mut self, key: Key, mods: Mods, fullscreen: bool) -> Option<Effect> {
        let line = self.font_size * 3.0;
        let page = (self.view_h - self.font_size * 4.0).max(line);
        match key {
            Key::PageDown => self.scroll_by(page),
            Key::PageUp => self.scroll_by(-page),
            Key::Space => self.scroll_by(if mods.shift { -page } else { page }),
            Key::Down => self.scroll_by(line),
            Key::Up => self.scroll_by(-line),
            Key::Home => self.scroll_by(-f32::MAX / 4.0),
            Key::End => self.scroll_by(f32::MAX / 4.0),
            Key::F11 => return Some(Effect::ToggleFullscreen),
            Key::F12 => return Some(Effect::Snapshot),
            Key::Escape if fullscreen => return Some(Effect::ExitFullscreen),
            Key::Escape => self.selection = None,
            Key::Char(c) if mods.ctrl => match c.to_ascii_lowercase() {
                'c' => {
                    let text = self.selection.map(|s| self.layout.selected_text(&s)).unwrap_or_default();
                    return (!text.is_empty()).then_some(Effect::Copy(text));
                }
                'a' => self.selection = self.layout.select_all(),
                '+' | '=' => self.set_font_size(self.font_size + 1.0),
                '-' => self.set_font_size(self.font_size - 1.0),
                '0' => self.set_font_size(DEFAULT_FONT_SIZE),
                'q' | 'w' => return Some(Effect::Quit),
                _ => {}
            },
            Key::Char(_) => {}
        }
        None
    }

    pub fn wheel(&mut self, dy: f32, mods: Mods) {
        if mods.ctrl {
            self.set_font_size(self.font_size + if dy < 0.0 { 1.0 } else { -1.0 });
        } else {
            self.scroll_by(dy);
        }
    }

    fn doc_point(&self, x: f32, y: f32) -> (f32, f32) {
        (x, y + self.scroll)
    }

    pub fn press(&mut self, x: f32, y: f32, mods: Mods, double_click_time: Duration) {
        self.press = (x, y);
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
    pub fn mouse_move(&mut self, x: f32, y: f32) -> bool {
        if !self.dragging {
            return false;
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

    pub fn release(&mut self, x: f32, y: f32) -> Option<Effect> {
        self.dragging = false;
        if (x - self.press.0).abs() > 3.0 || (y - self.press.1).abs() > 3.0 {
            return None;
        }
        let (dx, dy) = self.doc_point(x, y);
        let url = self.layout.link_at(dx, dy)?.to_string();
        if let Some(id) = url.strip_prefix('#') {
            if let Some(entry) = self.layout.toc.iter().find(|e| e.id == id) {
                self.scroll = entry.y - self.font_size;
                self.clamp_scroll();
                self.selection = None;
            }
            None
        } else {
            Some(Effect::OpenUrl(url))
        }
    }

    pub fn over_link(&self, x: f32, y: f32) -> bool {
        let (dx, dy) = self.doc_point(x, y);
        self.layout.link_at(dx, dy).is_some()
    }
}
