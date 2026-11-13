//! Document layout: turns the AST into positioned parley text blocks plus
//! decoration rectangles. Everything is in logical pixels, y grows downwards from
//! the top of the document.

use crate::ast::{AlertKind, Align, Kind, Node};
use crate::theme::{Ink, Theme};
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontContext, FontFamily, FontStyle, FontWeight,
    Layout, LayoutContext, LineHeight, Selection, StyleProperty, VerticalAlign,
};
use std::borrow::Cow;
use std::ops::Range;

const BODY_FONT: &str = "\"Segoe UI Variable Text\", \"Segoe UI\", -apple-system, \"SF Pro Text\", system-ui, \"Noto Sans\", \"Noto Color Emoji\", \"Segoe UI Emoji\", sans-serif";
const MONO_FONT: &str = "\"Cascadia Mono\", Consolas, \"SF Mono\", Menlo, \"DejaVu Sans Mono\", monospace";

/// How a block's text is joined to the previous block when copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    Paragraph,
    Line,
    Tab,
}

pub struct TextBlock {
    pub layout: Layout<Ink>,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub selectable: bool,
    pub join: Join,
    pub links: Vec<(Range<usize>, String)>,
}

impl TextBlock {
    pub fn height(&self) -> f32 {
        self.layout.height()
    }
    pub fn width(&self) -> f32 {
        self.layout.width()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Ink,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub level: u8,
    pub title: String,
    pub id: String,
    pub y: f32,
}

#[derive(Default)]
pub struct DocLayout {
    pub blocks: Vec<TextBlock>,
    pub rects: Vec<Rect>,
    pub toc: Vec<TocEntry>,
    pub front_matter: Option<String>,
    pub height: f32,
    pub width: f32,
}

#[derive(Clone)]
struct Ctx {
    x: f32,
    width: f32,
    size: f32,
    color: Ink,
    mono: bool,
    bold: bool,
    italic: bool,
    gap: f32,
    depth: usize,
    line_height: f32,
}

/// Inline content flattened into one string plus style spans.
#[derive(Default)]
struct Inline {
    text: String,
    spans: Vec<(Range<usize>, Span)>,
    backgrounds: Vec<(Range<usize>, Ink)>,
    links: Vec<(Range<usize>, String)>,
}

#[derive(Clone, Copy)]
enum Span {
    Bold,
    Italic,
    Mono,
    Color(Ink),
    Strike,
    Underline,
    Scale(f32),
    Super,
    Sub,
}

pub struct Layouter<'a> {
    pub fcx: &'a mut FontContext,
    pub lcx: &'a mut LayoutContext<Ink>,
    pub theme: &'a Theme,
    /// Base font size in logical px.
    pub base: f32,
}

impl Layouter<'_> {
    /// Lay out `doc` for a content column of `width` logical px starting at `x`.
    pub fn layout(&mut self, doc: &Node, x: f32, width: f32, top: f32) -> DocLayout {
        let mut out = DocLayout { width, ..Default::default() };
        let ctx = Ctx {
            x,
            width,
            size: self.base,
            color: self.theme.text,
            mono: false,
            bold: false,
            italic: false,
            gap: self.base,
            depth: 0,
            line_height: 1.6,
        };
        let mut y = top;
        self.blocks(&doc.children, &ctx, &mut y, &mut out, Join::Paragraph);
        out.height = y + self.base * 2.0;
        out
    }

    fn blocks(&mut self, nodes: &[Node], ctx: &Ctx, y: &mut f32, out: &mut DocLayout, first_join: Join) {
        let mut i = 0;
        let mut join = first_join;
        while i < nodes.len() {
            if nodes[i].kind.is_inline() {
                let j = nodes[i..].iter().position(|n| !n.kind.is_inline()).map_or(nodes.len(), |p| i + p);
                self.paragraph(&nodes[i..j], ctx, y, out, join);
                i = j;
            } else {
                self.block(&nodes[i], ctx, y, out, join);
                i += 1;
            }
            join = Join::Paragraph;
        }
    }

    fn paragraph(&mut self, inlines: &[Node], ctx: &Ctx, y: &mut f32, out: &mut DocLayout, join: Join) {
        // A paragraph that is just an image becomes an image placeholder block.
        if let [Node { kind: Kind::Image { url }, children, .. }] = inlines {
            self.image_placeholder(url, &Node::with_children(Kind::Paragraph, children.clone()).plain_text(), ctx, y, out, join);
            return;
        }
        let mut inline = Inline::default();
        self.flatten(inlines, &mut inline);
        let text = inline.text.trim_end().to_string();
        if text.is_empty() {
            return;
        }
        inline.text = text;
        self.push_text(inline, ctx, ctx.x, *y, ctx.width, None, true, join, out);
        *y += out.blocks.last().unwrap().height() + ctx.gap;
    }

    fn image_placeholder(&mut self, url: &str, alt: &str, ctx: &Ctx, y: &mut f32, out: &mut DocLayout, join: Join) {
        let h = self.base * 6.0;
        out.rects.push(Rect { x: ctx.x, y: *y, w: ctx.width.min(self.base * 24.0), h, color: self.theme.code_bg, radius: 8.0 });
        let label = if alt.is_empty() { format!("🖼 {url}") } else { format!("🖼 {alt}") };
        let inline = Inline { text: label, ..Default::default() };
        let c = Ctx { color: self.theme.muted, size: ctx.size * 0.9, ..ctx.clone() };
        let pad = self.base;
        self.push_text(inline, &c, ctx.x + pad, *y + pad, ctx.width.min(self.base * 24.0) - 2.0 * pad, None, true, join, out);
        *y += h + ctx.gap;
    }

    fn block(&mut self, node: &Node, ctx: &Ctx, y: &mut f32, out: &mut DocLayout, join: Join) {
        let theme = self.theme;
        let base = self.base;
        match &node.kind {
            Kind::FrontMatter => out.front_matter = Some(node.text.clone()),
            Kind::Paragraph => self.paragraph(&node.children, ctx, y, out, join),
            Kind::Heading { level, id } => {
                let scale = match level { 1 => 2.0, 2 => 1.5, 3 => 1.25, 4 => 1.0, 5 => 0.875, _ => 0.85 };
                if *y > base * 2.0 {
                    *y += base * 0.5;
                }
                let c = Ctx { size: base * scale, bold: true, color: theme.heading, line_height: 1.25, ..ctx.clone() };
                let mut inline = Inline::default();
                self.flatten(&node.children, &mut inline);
                out.toc.push(TocEntry { level: *level, title: inline.text.clone(), id: id.clone(), y: *y });
                self.push_text(inline, &c, ctx.x, *y, ctx.width, None, true, join, out);
                *y += out.blocks.last().unwrap().height();
                if *level <= 2 {
                    *y += base * 0.3;
                    out.rects.push(Rect { x: ctx.x, y: *y, w: ctx.width, h: 1.0, color: theme.border, radius: 0.0 });
                }
                *y += base * 0.75;
            }
            Kind::CodeBlock { .. } | Kind::HtmlBlock | Kind::DisplayMath => {
                let pad = base;
                let y0 = *y;
                let text = node.text.trim_end_matches('\n').to_string();
                let c = Ctx {
                    mono: true,
                    size: base * 0.875,
                    color: if node.kind == Kind::HtmlBlock { theme.muted } else { theme.code_text },
                    italic: node.kind == Kind::DisplayMath,
                    line_height: 1.45,
                    ..ctx.clone()
                };
                let rect_index = out.rects.len();
                out.rects.push(Rect { x: ctx.x, y: y0, w: ctx.width, h: 0.0, color: theme.code_bg, radius: 6.0 });
                let align = (node.kind == Kind::DisplayMath).then_some(Alignment::Center);
                self.push_text(Inline { text, ..Default::default() }, &c, ctx.x + pad, y0 + pad, ctx.width - 2.0 * pad, align, true, join, out);
                let h = out.blocks.last().unwrap().height() + 2.0 * pad;
                out.rects[rect_index].h = h;
                *y += h + ctx.gap;
            }
            Kind::BlockQuote(alert) => {
                let bar_color = match alert {
                    None => theme.quote_bar,
                    Some(AlertKind::Note) => theme.alert_note,
                    Some(AlertKind::Tip) => theme.alert_tip,
                    Some(AlertKind::Important) => theme.alert_important,
                    Some(AlertKind::Warning) => theme.alert_warning,
                    Some(AlertKind::Caution) => theme.alert_caution,
                };
                let indent = base * 1.0 + 4.0;
                let y0 = *y;
                let inner = Ctx {
                    x: ctx.x + indent,
                    width: ctx.width - indent,
                    color: if alert.is_some() { ctx.color } else { theme.muted },
                    ..ctx.clone()
                };
                let mut first = join;
                if let Some(kind) = alert {
                    let title = match kind {
                        AlertKind::Note => "ⓘ Note",
                        AlertKind::Tip => "💡 Tip",
                        AlertKind::Important => "❗ Important",
                        AlertKind::Warning => "⚠ Warning",
                        AlertKind::Caution => "⛔ Caution",
                    };
                    let c = Ctx { color: bar_color, bold: true, ..inner.clone() };
                    self.push_text(Inline { text: title.into(), ..Default::default() }, &c, inner.x, *y, inner.width, None, true, join, out);
                    *y += out.blocks.last().unwrap().height() + base * 0.5;
                    first = Join::Line;
                }
                self.blocks(&node.children, &inner, y, out, first);
                *y -= ctx.gap;
                out.rects.push(Rect { x: ctx.x, y: y0, w: 4.0, h: *y - y0, color: bar_color, radius: 2.0 });
                *y += ctx.gap;
            }
            Kind::List { start } => {
                let indent = base * 1.75;
                let tight = node.children.iter().all(|item| item.children.iter().all(|c| c.kind != Kind::Paragraph));
                let item_gap = if tight { base * 0.3 } else { ctx.gap };
                let inner = Ctx { x: ctx.x + indent, width: ctx.width - indent, depth: ctx.depth + 1, gap: item_gap, ..ctx.clone() };
                let mut first_join = join;
                for (idx, item) in node.children.iter().enumerate() {
                    let marker = match (&item.kind, start) {
                        (Kind::Item { task: Some(true) }, _) => "☑".to_string(),
                        (Kind::Item { task: Some(false) }, _) => "☐".to_string(),
                        (_, Some(s)) => format!("{}.", s + idx as u64),
                        _ => ["•", "◦", "▪"][ctx.depth % 3].to_string(),
                    };
                    let c = Ctx { color: theme.muted, ..ctx.clone() };
                    let marker_width = indent - base * 0.4;
                    self.push_text(Inline { text: marker, ..Default::default() }, &c, ctx.x, *y, marker_width, Some(Alignment::End), false, Join::Line, out);
                    self.blocks(&item.children, &inner, y, out, first_join);
                    first_join = Join::Line;
                }
                *y += ctx.gap - item_gap;
            }
            Kind::FootnoteDef { label } => {
                let c = Ctx { size: base * 0.85, color: theme.muted, ..ctx.clone() };
                let indent = base * 2.0;
                self.push_text(Inline { text: format!("{label}."), ..Default::default() }, &c, ctx.x, *y, indent - 6.0, Some(Alignment::End), false, Join::Line, out);
                let inner = Ctx { x: ctx.x + indent, width: ctx.width - indent, gap: base * 0.4, ..c };
                self.blocks(&node.children, &inner, y, out, join);
            }
            Kind::DefList => {
                for child in &node.children {
                    match child.kind {
                        Kind::DefTitle => {
                            let c = Ctx { bold: true, gap: base * 0.25, ..ctx.clone() };
                            self.blocks(&child.children, &c, y, out, join);
                        }
                        _ => {
                            let indent = base * 1.5;
                            let c = Ctx { x: ctx.x + indent, width: ctx.width - indent, gap: base * 0.5, ..ctx.clone() };
                            self.blocks(&child.children, &c, y, out, Join::Line);
                        }
                    }
                }
                *y += ctx.gap * 0.5;
            }
            Kind::Table { aligns } => self.table(node, aligns, ctx, y, out, join),
            Kind::Rule => {
                out.rects.push(Rect { x: ctx.x, y: *y + base * 0.5, w: ctx.width, h: 2.0, color: theme.border, radius: 1.0 });
                *y += base + ctx.gap;
            }
            // Inline kinds are handled by `paragraph`; stray containers just recurse.
            _ => self.blocks(&node.children, ctx, y, out, join),
        }
    }

    fn table(&mut self, node: &Node, aligns: &[Align], ctx: &Ctx, y: &mut f32, out: &mut DocLayout, join: Join) {
        let theme = self.theme;
        let pad = self.base * 0.6;
        let ncols = aligns.len().max(1);
        // Collect rows: (is_header, cells)
        let mut rows: Vec<(bool, &[Node])> = Vec::new();
        for part in &node.children {
            match part.kind {
                Kind::TableHead => rows.extend(part.children.iter().map(|r| (true, r.children.as_slice()))),
                Kind::TableRow => rows.push((false, part.children.as_slice())),
                _ => {}
            }
        }
        // Measure natural column widths.
        let mut natural = vec![0f32; ncols];
        let mut minimum = vec![0f32; ncols];
        let mut built: Vec<Vec<(Inline, bool)>> = Vec::new();
        for (header, cells) in &rows {
            let mut row = Vec::new();
            for (ci, cell) in cells.iter().enumerate().take(ncols) {
                let mut inline = Inline::default();
                self.flatten(&cell.children, &mut inline);
                let c = Ctx { bold: *header, ..ctx.clone() };
                let layout = self.build(&inline, &c);
                let widths = layout.calculate_content_widths();
                natural[ci] = natural[ci].max(widths.max + 2.0 * pad);
                minimum[ci] = minimum[ci].max(widths.min + 2.0 * pad);
                row.push((inline, *header));
            }
            built.push(row);
        }
        let total: f32 = natural.iter().sum();
        let widths: Vec<f32> = if total <= ctx.width {
            natural.clone()
        } else {
            let min_total: f32 = minimum.iter().sum();
            let spare = (ctx.width - min_total).max(0.0);
            let flex: f32 = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum::<f32>().max(1.0);
            natural.iter().zip(&minimum).map(|(n, m)| m + spare * (n - m) / flex).collect()
        };
        let table_w: f32 = widths.iter().sum();
        let y0 = *y;
        let mut first_row = true;
        for (ri, row) in built.into_iter().enumerate() {
            let row_y = *y;
            let bg_index = out.rects.len();
            let mut x = ctx.x;
            let mut row_h: f32 = 0.0;
            let header = row.first().is_some_and(|c| c.1);
            for (ci, (inline, header)) in row.into_iter().enumerate() {
                let align = match aligns.get(ci) {
                    Some(Align::Center) => Some(Alignment::Center),
                    Some(Align::Right) => Some(Alignment::End),
                    _ => None,
                };
                let c = Ctx { bold: header, ..ctx.clone() };
                let cell_join = if ci == 0 { if first_row { join } else { Join::Line } } else { Join::Tab };
                self.push_text(inline, &c, x + pad, row_y + pad, widths[ci] - 2.0 * pad, align, true, cell_join, out);
                row_h = row_h.max(out.blocks.last().unwrap().height() + 2.0 * pad);
                x += widths[ci];
            }
            // Header and every other body row get a stripe, drawn below the cell contents.
            if header || ri % 2 == 0 {
                out.rects.insert(bg_index, Rect { x: ctx.x, y: row_y, w: table_w, h: row_h, color: theme.table_stripe, radius: 0.0 });
            }
            out.rects.push(Rect { x: ctx.x, y: row_y, w: table_w, h: 1.0, color: theme.border, radius: 0.0 });
            *y += row_h;
            first_row = false;
        }
        // Outer bottom border and column separators.
        out.rects.push(Rect { x: ctx.x, y: *y, w: table_w, h: 1.0, color: theme.border, radius: 0.0 });
        let mut x = ctx.x;
        for w in widths.iter().chain(std::iter::once(&0.0)) {
            out.rects.push(Rect { x, y: y0, w: 1.0, h: *y - y0 + 1.0, color: theme.border, radius: 0.0 });
            x += w;
        }
        *y += ctx.gap;
    }

    fn flatten(&self, nodes: &[Node], out: &mut Inline) {
        let theme = self.theme;
        for node in nodes {
            let start = out.text.len();
            match &node.kind {
                Kind::Text => out.text.push_str(&node.text),
                Kind::SoftBreak => out.text.push(' '),
                Kind::HardBreak => out.text.push('\n'),
                Kind::Code => {
                    out.text.push('\u{2009}');
                    out.text.push_str(&node.text);
                    out.text.push('\u{2009}');
                    let r = start..out.text.len();
                    out.spans.push((r.clone(), Span::Mono));
                    out.spans.push((r.clone(), Span::Scale(0.9)));
                    out.spans.push((r.clone(), Span::Color(theme.code_text)));
                    out.backgrounds.push((r, theme.code_bg));
                }
                Kind::InlineMath => {
                    out.text.push_str(&node.text);
                    let r = start..out.text.len();
                    out.spans.push((r.clone(), Span::Italic));
                    out.spans.push((r, Span::Mono));
                }
                Kind::InlineHtml => {
                    let tag = node.text.trim().to_ascii_lowercase();
                    if tag.starts_with("<br") {
                        out.text.push('\n');
                    }
                    // Other inline HTML tags are dropped in the prototype.
                }
                Kind::FootnoteRef { label } => {
                    out.text.push_str(label);
                    let r = start..out.text.len();
                    out.spans.push((r.clone(), Span::Super));
                    out.spans.push((r.clone(), Span::Scale(0.75)));
                    out.spans.push((r, Span::Color(theme.link)));
                }
                Kind::Image { url } => {
                    let alt = Node::with_children(Kind::Paragraph, node.children.clone()).plain_text();
                    out.text.push_str(&format!("🖼 {}", if alt.is_empty() { url.as_str() } else { alt.as_str() }));
                    out.spans.push((start..out.text.len(), Span::Color(theme.muted)));
                }
                kind => {
                    self.flatten(&node.children, out);
                    let r = start..out.text.len();
                    match kind {
                        Kind::Emph => out.spans.push((r, Span::Italic)),
                        Kind::Strong => out.spans.push((r, Span::Bold)),
                        Kind::Strike => out.spans.push((r, Span::Strike)),
                        Kind::Sup => {
                            out.spans.push((r.clone(), Span::Super));
                            out.spans.push((r, Span::Scale(0.75)));
                        }
                        Kind::Sub => {
                            out.spans.push((r.clone(), Span::Sub));
                            out.spans.push((r, Span::Scale(0.75)));
                        }
                        Kind::Mark => out.backgrounds.push((r, theme.mark_bg)),
                        Kind::Ins => {
                            out.spans.push((r.clone(), Span::Underline));
                            out.spans.push((r, Span::Color(theme.ins)));
                        }
                        Kind::Link { url } => {
                            out.spans.push((r.clone(), Span::Color(theme.link)));
                            out.links.push((r, url.clone()));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn build(&mut self, inline: &Inline, ctx: &Ctx) -> Layout<Ink> {
        let mut b = self.lcx.ranged_builder(self.fcx, &inline.text, 1.0, true);
        b.push_default(StyleProperty::FontSize(ctx.size));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(ctx.line_height)));
        b.push_default(StyleProperty::Brush(ctx.color));
        b.push_default(StyleProperty::FontFamily(FontFamily::Source(Cow::Borrowed(if ctx.mono { MONO_FONT } else { BODY_FONT }))));
        if ctx.bold {
            b.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
        }
        if ctx.italic {
            b.push_default(StyleProperty::FontStyle(FontStyle::Italic));
        }
        for (range, span) in &inline.spans {
            let r = range.clone();
            match *span {
                Span::Bold => b.push(StyleProperty::FontWeight(FontWeight::BOLD), r),
                Span::Italic => b.push(StyleProperty::FontStyle(FontStyle::Italic), r),
                Span::Mono => b.push(StyleProperty::FontFamily(FontFamily::Source(Cow::Borrowed(MONO_FONT))), r),
                Span::Color(c) => b.push(StyleProperty::Brush(c), r),
                Span::Strike => b.push(StyleProperty::Strikethrough(true), r),
                Span::Underline => b.push(StyleProperty::Underline(true), r),
                Span::Scale(s) => b.push(StyleProperty::FontSize(ctx.size * s), r),
                Span::Super => b.push(StyleProperty::VerticalAlign(VerticalAlign::SUPER), r),
                Span::Sub => b.push(StyleProperty::VerticalAlign(VerticalAlign::SUB), r),
            }
        }
        b.build(&inline.text)
    }

    #[allow(clippy::too_many_arguments)]
    fn push_text(
        &mut self,
        inline: Inline,
        ctx: &Ctx,
        x: f32,
        y: f32,
        width: f32,
        align: Option<Alignment>,
        selectable: bool,
        join: Join,
        out: &mut DocLayout,
    ) {
        let mut layout = self.build(&inline, ctx);
        layout.break_all_lines(Some(width.max(1.0)));
        if let Some(a) = align {
            layout.set_alignment(a, AlignmentOptions::default());
        }
        // Inline backgrounds (code spans, ==mark==) come from parley's selection geometry.
        for (range, color) in &inline.backgrounds {
            let sel = Selection::new(
                Cursor::from_byte_index(&layout, range.start, Affinity::Downstream),
                Cursor::from_byte_index(&layout, range.end, Affinity::Upstream),
            );
            for (bb, _) in sel.geometry(&layout) {
                out.rects.push(Rect {
                    x: x + bb.x0 as f32,
                    y: y + bb.y0 as f32 + 1.0,
                    w: (bb.x1 - bb.x0) as f32,
                    h: (bb.y1 - bb.y0) as f32 - 2.0,
                    color: *color,
                    radius: 4.0,
                });
            }
        }
        out.blocks.push(TextBlock { layout, text: inline.text, x, y, selectable, join, links: inline.links });
    }
}

// ---------------------------------------------------------------------------
// Document-wide selection
// ---------------------------------------------------------------------------

/// A caret position in the whole document: block index + byte offset into its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DocPos {
    pub block: usize,
    pub byte: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocSelection {
    pub anchor: DocPos,
    pub focus: DocPos,
}

impl DocSelection {
    pub fn ordered(&self) -> (DocPos, DocPos) {
        if self.anchor <= self.focus { (self.anchor, self.focus) } else { (self.focus, self.anchor) }
    }
    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }
}

impl DocLayout {
    /// Map a document point to the nearest caret position in a selectable block.
    pub fn hit(&self, x: f32, y: f32) -> Option<DocPos> {
        let mut best: Option<(f32, usize)> = None;
        for (i, b) in self.blocks.iter().enumerate() {
            if !b.selectable {
                continue;
            }
            let (top, bottom) = (b.y, b.y + b.height());
            let (left, right) = (b.x, b.x + b.layout.full_width().max(b.width()).max(1.0));
            let dy = if y < top { top - y } else if y > bottom { y - bottom } else { 0.0 };
            let dx = if x < left { left - x } else if x > right { x - right } else { 0.0 };
            // Vertical distance dominates so that clicking in a margin picks the same line.
            let d = dy * 1000.0 + dx;
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
            if b.y > y + 2000.0 {
                break;
            }
        }
        let (_, i) = best?;
        let b = &self.blocks[i];
        let cursor = Cursor::from_point(&b.layout, x - b.x, y - b.y);
        Some(DocPos { block: i, byte: cursor.index() })
    }

    pub fn word_at(&self, pos: DocPos, x: f32, y: f32) -> DocSelection {
        let b = &self.blocks[pos.block];
        let sel = Selection::word_from_point(&b.layout, x - b.x, y - b.y);
        let r = sel.text_range();
        DocSelection {
            anchor: DocPos { block: pos.block, byte: r.start },
            focus: DocPos { block: pos.block, byte: r.end },
        }
    }

    pub fn block_selection(&self, block: usize) -> DocSelection {
        DocSelection {
            anchor: DocPos { block, byte: 0 },
            focus: DocPos { block, byte: self.blocks[block].text.len() },
        }
    }

    pub fn select_all(&self) -> Option<DocSelection> {
        let first = self.blocks.iter().position(|b| b.selectable)?;
        let last = self.blocks.iter().rposition(|b| b.selectable)?;
        Some(DocSelection {
            anchor: DocPos { block: first, byte: 0 },
            focus: DocPos { block: last, byte: self.blocks[last].text.len() },
        })
    }

    /// Byte range of `block` covered by the selection, if any.
    pub fn block_range(&self, sel: &DocSelection, block: usize) -> Option<Range<usize>> {
        let (start, end) = sel.ordered();
        if block < start.block || block > end.block || !self.blocks[block].selectable {
            return None;
        }
        let len = self.blocks[block].text.len();
        let a = if block == start.block { start.byte } else { 0 };
        let b = if block == end.block { end.byte } else { len };
        (a < b).then_some(a..b)
    }

    /// Plain-text contents of the selection, joining blocks by paragraph/line/tab.
    pub fn selected_text(&self, sel: &DocSelection) -> String {
        let (start, end) = sel.ordered();
        let mut out = String::new();
        let mut first = true;
        for i in start.block..=end.block.min(self.blocks.len().saturating_sub(1)) {
            let Some(r) = self.block_range(sel, i) else { continue };
            if !first {
                out.push_str(match self.blocks[i].join {
                    Join::Paragraph => "\n\n",
                    Join::Line => "\n",
                    Join::Tab => "\t",
                });
            }
            first = false;
            out.push_str(&self.blocks[i].text[r]);
        }
        out
    }

    pub fn link_at(&self, pos: DocPos) -> Option<&str> {
        self.blocks[pos.block]
            .links
            .iter()
            .find(|(r, _)| r.contains(&pos.byte))
            .map(|(_, url)| url.as_str())
    }
}
