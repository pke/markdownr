//! Platform-neutral document layout.
//!
//! Turns the AST into positioned paragraphs plus decoration rectangles. The actual
//! text shaping, line breaking and hit testing is done by the platform's text engine
//! behind the [`Typesetter`] / [`Paragraph`] traits (DirectWrite on Windows; Core Text
//! and Pango later). Everything is in logical pixels (DIPs), y grows downwards.

use crate::ast::{AlertKind, Align, Kind, Node};
use crate::math::{MathBox, MathFont};
use crate::theme::{Ink, Theme};
use std::ops::Range;

// ---------------------------------------------------------------------------
// Platform interface
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Start,
    Center,
    End,
}

/// Style of a whole paragraph; `Span`s refine ranges of it.
#[derive(Debug, Clone, Copy)]
pub struct BaseStyle {
    pub size: f32,
    pub color: Ink,
    pub mono: bool,
    pub bold: bool,
    pub italic: bool,
    /// Line height as a multiple of `size`.
    pub line_height: f32,
}

/// A style applied to a byte range of a paragraph's text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Span {
    Bold,
    Italic,
    Mono,
    Color(Ink),
    Strike,
    Underline,
    /// Font size relative to the paragraph's base size.
    Scale(f32),
    Super,
    Sub,
    /// An inline box (math formula) replacing a U+FFFC character.
    Object { w: f32, asc: f32, desc: f32 },
}

/// Placeholder character for inline objects in paragraph text.
pub const OBJECT_CHAR: char = '\u{FFFC}';

/// A laid-out paragraph owned by the platform text engine. Byte offsets refer to
/// the UTF-8 text the paragraph was built from.
pub trait Paragraph {
    /// (width, height) of the laid-out text.
    fn size(&self) -> (f32, f32);
    /// (minimum, maximum) content width, for table column sizing.
    fn content_widths(&self) -> (f32, f32);
    fn set_width(&mut self, width: f32);
    fn set_align(&mut self, align: TextAlign);
    /// Caret byte offset nearest to the point (paragraph-local coordinates).
    fn hit(&self, x: f32, y: f32) -> usize;
    /// Rectangles covering a byte range (paragraph-local coordinates).
    fn range_rects(&self, range: Range<usize>) -> Vec<RectF>;
    /// Top-left of the inline object at `byte` (paragraph-local coordinates).
    fn object_origin(&self, byte: usize) -> (f32, f32);
}

pub trait Typesetter {
    type Para: Paragraph;
    fn paragraph(&mut self, text: &str, spans: &[(Range<usize>, Span)], base: &BaseStyle, max_width: f32) -> Self::Para;
    /// Pixel size of an image, if it is already loaded. Platforms load images
    /// asynchronously and request a relayout once they arrive.
    fn image_size(&self, _url: &str) -> Option<(f32, f32)> {
        None
    }
    /// The OpenType MATH font used for formulas, if the platform has one.
    fn math_font(&self) -> Option<&MathFont> {
        None
    }
}

/// A decoded image positioned in the document.
pub struct ImagePlacement {
    pub url: String,
    pub r: RectF,
}

/// What an inline object (U+FFFC) stands for.
enum Obj {
    Math(MathBox),
    Image(String, f32, f32),
}

/// A formula positioned in the document: `y` is its baseline.
pub struct MathPlacement {
    pub x: f32,
    pub y: f32,
    pub mbox: MathBox,
    pub color: Ink,
}

// ---------------------------------------------------------------------------
// Document layout
// ---------------------------------------------------------------------------

/// How a block's text is joined to the previous block when copying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    Paragraph,
    Line,
    Tab,
}

pub struct TextBlock<P> {
    pub para: P,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub selectable: bool,
    pub join: Join,
    pub links: Vec<(Range<usize>, String)>,
    /// Source text for each inline object, used when copying (`$x^2$`).
    pub object_sources: Vec<(usize, String)>,
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub r: RectF,
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

pub struct DocLayout<P> {
    pub blocks: Vec<TextBlock<P>>,
    pub rects: Vec<Rect>,
    pub math: Vec<MathPlacement>,
    pub images: Vec<ImagePlacement>,
    /// Santa hats on headings (Christmas theme): (x, y, size), top-left corner.
    pub hats: Vec<(f32, f32, f32)>,
    /// Jump targets besides headings: (id, y).
    pub anchors: Vec<(String, f32)>,
    /// `:::quotes` groups: per quote the block and rect index ranges.
    pub cycles: Vec<Vec<(Range<usize>, Range<usize>)>>,
    pub toc: Vec<TocEntry>,
    pub front_matter: Option<String>,
    pub height: f32,
}

impl<P> Default for DocLayout<P> {
    fn default() -> Self {
        DocLayout { blocks: Vec::new(), rects: Vec::new(), math: Vec::new(), images: Vec::new(), hats: Vec::new(), anchors: Vec::new(), cycles: Vec::new(), toc: Vec::new(), front_matter: None, height: 0.0 }
    }
}

#[derive(Clone)]
struct Ctx {
    x: f32,
    width: f32,
    style: BaseStyle,
    gap: f32,
    depth: usize,
}

#[derive(Default)]
struct Inline {
    text: String,
    spans: Vec<(Range<usize>, Span)>,
    backgrounds: Vec<(Range<usize>, Ink)>,
    links: Vec<(Range<usize>, String)>,
    /// (byte of the U+FFFC, laid-out formula, copy text)
    objects: Vec<(usize, Obj, String)>,
    /// (byte, anchor id) — jump targets inside the paragraph (footnote references).
    anchors: Vec<(usize, String)>,
}

impl Inline {
    fn plain(text: impl Into<String>) -> Self {
        Inline { text: text.into(), ..Default::default() }
    }
}

pub struct Layouter<'a, T: Typesetter> {
    pub ts: &'a mut T,
    pub theme: &'a Theme,
    /// Base font size in DIPs.
    pub base: f32,
}

impl<T: Typesetter> Layouter<'_, T> {
    pub fn layout(&mut self, doc: &Node, x: f32, width: f32, top: f32) -> DocLayout<T::Para> {
        let mut out = DocLayout::default();
        let ctx = Ctx {
            x,
            width,
            style: BaseStyle { size: self.base, color: self.theme.text, mono: false, bold: false, italic: false, line_height: 1.6 },
            gap: self.base,
            depth: 0,
        };
        let mut y = top;
        self.blocks(&doc.children, &ctx, &mut y, &mut out, Join::Paragraph);
        out.height = y + self.base * 2.0;
        out
    }

    fn blocks(&mut self, nodes: &[Node], ctx: &Ctx, y: &mut f32, out: &mut DocLayout<T::Para>, first_join: Join) {
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

    fn paragraph(&mut self, inlines: &[Node], ctx: &Ctx, y: &mut f32, out: &mut DocLayout<T::Para>, join: Join) {
        if let [Node { kind: Kind::Image { url }, children, .. }] = inlines {
            let alt: String = children.iter().map(Node::plain_text).collect();
            self.image_placeholder(url, &alt, ctx, y, out, join);
            return;
        }
        let mut inline = Inline::default();
        self.flatten(inlines, &mut inline, ctx.style.size);
        let trimmed = inline.text.trim_end().len();
        inline.text.truncate(trimmed);
        if inline.text.is_empty() {
            return;
        }
        let h = self.push(inline, &ctx.style, ctx.x, *y, ctx.width, None, true, join, out);
        *y += h + ctx.gap;
    }

    fn image_placeholder(&mut self, url: &str, alt: &str, ctx: &Ctx, y: &mut f32, out: &mut DocLayout<T::Para>, join: Join) {
        if let Some((iw, ih)) = self.ts.image_size(url) {
            // Natural size in DIPs, shrunk to the column.
            let w = iw.min(ctx.width);
            let h = ih * w / iw.max(1.0);
            out.images.push(ImagePlacement { url: url.to_string(), r: RectF { x: ctx.x, y: *y, w, h } });
            *y += h + ctx.gap;
            return;
        }
        let h = self.base * 6.0;
        let w = ctx.width.min(self.base * 24.0);
        out.rects.push(Rect { r: RectF { x: ctx.x, y: *y, w, h }, color: self.theme.code_bg, radius: 8.0 });
        let label = format!("🖼 {}", if alt.is_empty() { url } else { alt });
        let style = BaseStyle { color: self.theme.muted, size: ctx.style.size * 0.9, ..ctx.style };
        let pad = self.base;
        self.push(Inline::plain(label), &style, ctx.x + pad, *y + pad, w - 2.0 * pad, None, true, join, out);
        *y += h + ctx.gap;
    }

    fn block(&mut self, node: &Node, ctx: &Ctx, y: &mut f32, out: &mut DocLayout<T::Para>, join: Join) {
        let theme = self.theme;
        let base = self.base;
        match &node.kind {
            Kind::FrontMatter => out.front_matter = Some(node.text.clone()),
            // Via `blocks`, so display math (`$$…$$`, an inline event inside a paragraph) becomes its own block.
            Kind::Paragraph => self.blocks(&node.children, ctx, y, out, join),
            Kind::Heading { level, id } => {
                let scale = match level { 1 => 2.0, 2 => 1.5, 3 => 1.25, 4 => 1.0, 5 => 0.875, _ => 0.85 };
                if !out.blocks.is_empty() {
                    *y += base * 0.5;
                }
                let style = BaseStyle { size: base * scale, bold: true, color: theme.heading, line_height: 1.25, ..ctx.style };
                let mut inline = Inline::default();
                self.flatten(&node.children, &mut inline, style.size);
                out.toc.push(TocEntry { level: *level, title: inline.text.clone(), id: id.clone(), y: *y });
                if let Some(hat) = theme.santa_hat.filter(|_| *level <= 2) {
                    // Perched on the first letter, scaled with the heading.
                    let s = hat * scale / 2.0;
                    out.hats.push((ctx.x - s * 0.3, *y - s * 0.55, s));
                }
                *y += self.push(inline, &style, ctx.x, *y, ctx.width, None, true, join, out);
                if *level <= 2 {
                    *y += base * 0.3;
                    out.rects.push(Rect { r: RectF { x: ctx.x, y: *y, w: ctx.width, h: 1.0 }, color: theme.border, radius: 0.0 });
                }
                *y += base * 0.75;
            }
            Kind::DisplayMath if self.ts.math_font().is_some() => {
                let mbox = self.ts.math_font().unwrap().layout(&node.text, ctx.style.size * 1.1, true);
                let mut inline = Inline::plain(OBJECT_CHAR.to_string());
                inline.spans.push((0..inline.text.len(), Span::Object { w: mbox.w, asc: mbox.asc, desc: mbox.desc }));
                inline.objects.push((0, Obj::Math(mbox), format!("$$\n{}\n$$", node.text.trim())));
                *y += base * 0.25;
                let h = self.push(inline, &ctx.style, ctx.x, *y, ctx.width, Some(TextAlign::Center), true, join, out);
                *y += h + ctx.gap + base * 0.25;
            }
            Kind::CodeBlock { .. } | Kind::HtmlBlock | Kind::DisplayMath => {
                let pad = base;
                let y0 = *y;
                let style = BaseStyle {
                    mono: true,
                    size: base * 0.875,
                    color: if node.kind == Kind::HtmlBlock { theme.muted } else { theme.code_text },
                    italic: node.kind == Kind::DisplayMath,
                    line_height: 1.45,
                    bold: false,
                };
                let bg = out.rects.len();
                out.rects.push(Rect { r: RectF { x: ctx.x, y: y0, w: ctx.width, h: 0.0 }, color: theme.code_bg, radius: 6.0 });
                let align = (node.kind == Kind::DisplayMath).then_some(TextAlign::Center);
                let text = node.text.trim_end_matches('\n').to_string();
                let mut inline = Inline::plain(text);
                if let Kind::CodeBlock { lang } = &node.kind {
                    for (r, tok) in crate::highlight::highlight(lang, &inline.text) {
                        use crate::highlight::Tok;
                        let ink = match tok {
                            Tok::Keyword => theme.syn_keyword,
                            Tok::Literal | Tok::Number => theme.syn_number,
                            Tok::Type => theme.syn_type,
                            Tok::String => theme.syn_string,
                            Tok::Comment => theme.syn_comment,
                            Tok::Function => theme.syn_function,
                            Tok::Tag => theme.syn_tag,
                            Tok::Attr => theme.syn_attr,
                            Tok::Punct => continue,
                        };
                        inline.spans.push((r.clone(), Span::Color(ink)));
                        if tok == Tok::Comment {
                            inline.spans.push((r, Span::Italic));
                        }
                    }
                }
                let h = self.push(inline, &style, ctx.x + pad, y0 + pad, ctx.width - 2.0 * pad, align, true, join, out) + 2.0 * pad;
                out.rects[bg].r.h = h;
                *y += h + ctx.gap;
            }
            Kind::BlockQuote(alert) => {
                let bar = match alert {
                    None => theme.quote_bar,
                    Some(AlertKind::Note) => theme.alert_note,
                    Some(AlertKind::Tip) => theme.alert_tip,
                    Some(AlertKind::Important) => theme.alert_important,
                    Some(AlertKind::Warning) => theme.alert_warning,
                    Some(AlertKind::Caution) => theme.alert_caution,
                };
                let indent = base + 4.0;
                let y0 = *y;
                let inner = Ctx {
                    x: ctx.x + indent,
                    width: ctx.width - indent,
                    style: BaseStyle { color: if alert.is_some() { ctx.style.color } else { theme.muted }, ..ctx.style },
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
                    let style = BaseStyle { color: bar, bold: true, ..inner.style };
                    *y += self.push(Inline::plain(title), &style, inner.x, *y, inner.width, None, true, join, out) + base * 0.5;
                    first = Join::Line;
                }
                self.blocks(&node.children, &inner, y, out, first);
                *y -= ctx.gap;
                out.rects.push(Rect { r: RectF { x: ctx.x, y: y0, w: 4.0, h: *y - y0 }, color: bar, radius: 2.0 });
                *y += ctx.gap;
            }
            Kind::List { start } => {
                let indent = base * 1.75;
                let tight = node.children.iter().all(|item| item.children.iter().all(|c| c.kind != Kind::Paragraph));
                let item_gap = if tight { base * 0.3 } else { ctx.gap };
                let inner = Ctx { x: ctx.x + indent, width: ctx.width - indent, depth: ctx.depth + 1, gap: item_gap, ..ctx.clone() };
                let mut item_join = join;
                for (idx, item) in node.children.iter().enumerate() {
                    let marker = match (&item.kind, start) {
                        (Kind::Item { task: Some(true) }, _) => "☑".to_string(),
                        (Kind::Item { task: Some(false) }, _) => "☐".to_string(),
                        (_, Some(s)) => format!("{}.", s + idx as u64),
                        _ => ["•", "◦", "▪"][ctx.depth % 3].to_string(),
                    };
                    let style = BaseStyle { color: theme.muted, bold: false, ..ctx.style };
                    self.push(Inline::plain(marker), &style, ctx.x, *y, indent - base * 0.4, Some(TextAlign::End), false, Join::Line, out);
                    self.blocks(&item.children, &inner, y, out, item_join);
                    item_join = Join::Line;
                }
                *y += ctx.gap - item_gap;
            }
            Kind::FootnoteDef { label } => {
                let style = BaseStyle { size: base * 0.85, color: theme.muted, ..ctx.style };
                let indent = base * 2.0;
                out.anchors.push((format!("fn-{label}"), *y - base * 0.5));
                self.push(Inline::plain(format!("{label}.")), &style, ctx.x, *y, indent - 6.0, Some(TextAlign::End), false, Join::Line, out);
                let inner = Ctx { x: ctx.x + indent, width: ctx.width - indent, gap: base * 0.4, style, ..ctx.clone() };
                self.blocks(&node.children, &inner, y, out, join);
            }
            Kind::DefList => {
                for child in &node.children {
                    if child.kind == Kind::DefTitle {
                        let c = Ctx { style: BaseStyle { bold: true, ..ctx.style }, gap: base * 0.25, ..ctx.clone() };
                        self.blocks(&child.children, &c, y, out, join);
                    } else {
                        let indent = base * 1.5;
                        let c = Ctx { x: ctx.x + indent, width: ctx.width - indent, gap: base * 0.5, ..ctx.clone() };
                        self.blocks(&child.children, &c, y, out, Join::Line);
                    }
                }
                *y += ctx.gap * 0.5;
            }
            Kind::Table { aligns } => self.table(node, aligns, ctx, y, out, join),
            Kind::QuoteCycle => {
                // All quotes are laid out at the same place; the renderer shows one
                // at a time (10 s each, 600 ms crossfade, like the iOS QuoteCycler).
                let y0 = *y;
                let mut max_y = y0;
                let mut group = Vec::new();
                for q in &node.children {
                    let (b0, r0) = (out.blocks.len(), out.rects.len());
                    let mut qy = y0;
                    self.block(q, ctx, &mut qy, out, join);
                    max_y = max_y.max(qy);
                    group.push((b0..out.blocks.len(), r0..out.rects.len()));
                }
                out.cycles.push(group);
                *y = max_y;
            }
            Kind::Rule if theme.hr_emojis.is_some() => {
                let text = theme.hr_emojis.unwrap().join("   ");
                let style = BaseStyle { size: base * 1.25, line_height: 1.4, ..ctx.style };
                *y += self.push(Inline::plain(text), &style, ctx.x, *y, ctx.width, Some(TextAlign::Center), false, Join::Paragraph, out) + ctx.gap;
            }
            Kind::Rule => {
                out.rects.push(Rect { r: RectF { x: ctx.x, y: *y + base * 0.5, w: ctx.width, h: 2.0 }, color: theme.border, radius: 1.0 });
                *y += base + ctx.gap;
            }
            _ => self.blocks(&node.children, ctx, y, out, join),
        }
    }

    fn table(&mut self, node: &Node, aligns: &[Align], ctx: &Ctx, y: &mut f32, out: &mut DocLayout<T::Para>, join: Join) {
        let theme = self.theme;
        let pad = self.base * 0.6;
        let ncols = aligns.len().max(1);
        let mut rows: Vec<(bool, &[Node])> = Vec::new();
        for part in &node.children {
            match part.kind {
                Kind::TableHead => rows.extend(part.children.iter().map(|r| (true, r.children.as_slice()))),
                Kind::TableRow => rows.push((false, part.children.as_slice())),
                _ => {}
            }
        }
        // Build every cell once at "infinite" width to measure, then re-wrap.
        let mut natural = vec![0f32; ncols];
        let mut minimum = vec![0f32; ncols];
        let mut cells: Vec<Vec<(T::Para, Inline)>> = Vec::new();
        for (header, row) in &rows {
            let mut built = Vec::new();
            for (ci, cell) in row.iter().enumerate().take(ncols) {
                let mut inline = Inline::default();
                self.flatten(&cell.children, &mut inline, ctx.style.size);
                let style = BaseStyle { bold: *header, ..ctx.style };
                let para = self.ts.paragraph(&inline.text, &inline.spans, &style, 1.0e6);
                let (min, max) = para.content_widths();
                // Round up: laying out at exactly the measured width can wrap the last letter.
                natural[ci] = natural[ci].max((max + 1.0).ceil() + 2.0 * pad);
                minimum[ci] = minimum[ci].max((min + 1.0).ceil() + 2.0 * pad);
                built.push((para, inline));
            }
            cells.push(built);
        }
        let total: f32 = natural.iter().sum();
        let widths: Vec<f32> = if total <= ctx.width {
            natural.clone()
        } else {
            let min_total: f32 = minimum.iter().sum();
            let spare = (ctx.width - min_total).max(0.0);
            let flex = natural.iter().zip(&minimum).map(|(n, m)| n - m).sum::<f32>().max(1.0);
            natural.iter().zip(&minimum).map(|(n, m)| m + spare * (n - m) / flex).collect()
        };
        let table_w: f32 = widths.iter().sum();
        let y0 = *y;
        for (ri, ((header, _), row)) in rows.iter().zip(cells).enumerate() {
            let row_y = *y;
            let bg = out.rects.len();
            let mut x = ctx.x;
            let mut row_h: f32 = 0.0;
            for (ci, (mut para, inline)) in row.into_iter().enumerate() {
                para.set_width(widths[ci] - 2.0 * pad);
                match aligns.get(ci) {
                    Some(Align::Center) => para.set_align(TextAlign::Center),
                    Some(Align::Right) => para.set_align(TextAlign::End),
                    _ => {}
                }
                let cell_join = match (ci, ri) {
                    (0, 0) => join,
                    (0, _) => Join::Line,
                    _ => Join::Tab,
                };
                let h = self.place(para, inline, ctx.style.color, x + pad, row_y + pad, true, cell_join, out);
                row_h = row_h.max(h + 2.0 * pad);
                x += widths[ci];
            }
            if *header || ri % 2 == 0 {
                out.rects.insert(bg, Rect { r: RectF { x: ctx.x, y: row_y, w: table_w, h: row_h }, color: theme.table_stripe, radius: 0.0 });
            }
            out.rects.push(Rect { r: RectF { x: ctx.x, y: row_y, w: table_w, h: 1.0 }, color: theme.border, radius: 0.0 });
            *y += row_h;
        }
        out.rects.push(Rect { r: RectF { x: ctx.x, y: *y, w: table_w, h: 1.0 }, color: theme.border, radius: 0.0 });
        let mut x = ctx.x;
        for w in widths.iter().chain(std::iter::once(&0.0)) {
            out.rects.push(Rect { r: RectF { x, y: y0, w: 1.0, h: *y - y0 + 1.0 }, color: theme.border, radius: 0.0 });
            x += w;
        }
        *y += ctx.gap;
    }

    fn flatten(&self, nodes: &[Node], out: &mut Inline, size: f32) {
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
                Kind::InlineMath if self.ts.math_font().is_some() => {
                    let mbox = self.ts.math_font().unwrap().layout(&node.text, size, false);
                    out.text.push(OBJECT_CHAR);
                    out.spans.push((start..out.text.len(), Span::Object { w: mbox.w, asc: mbox.asc, desc: mbox.desc }));
                    out.objects.push((start, Obj::Math(mbox), format!("${}$", node.text)));
                }
                Kind::InlineMath => {
                    out.text.push_str(&node.text);
                    let r = start..out.text.len();
                    out.spans.push((r.clone(), Span::Italic));
                    out.spans.push((r, Span::Mono));
                }
                Kind::InlineHtml => {
                    if node.text.trim().to_ascii_lowercase().starts_with("<br") {
                        out.text.push('\n');
                    }
                }
                Kind::FootnoteRef { label } => {
                    match unicode_script(label, true) {
                        Some(s) => out.text.push_str(&s),
                        None => out.text.push_str(label),
                    }
                    let r = start..out.text.len();
                    if unicode_script(label, true).is_none() {
                        out.spans.push((r.clone(), Span::Super));
                    }
                    out.links.push((r.clone(), format!("#fn-{label}")));
                    out.anchors.push((start, format!("fnref-{label}")));
                    out.spans.push((r, Span::Color(theme.link)));
                }
                Kind::Image { url } if self.ts.image_size(url).is_some() => {
                    let (iw, ih) = self.ts.image_size(url).unwrap();
                    // Inline images (badges, icons) keep their natural size, capped to ~4 lines.
                    let scale = (size * 6.0 / ih.max(1.0)).min(1.0);
                    let (w, h) = (iw * scale, ih * scale);
                    out.text.push(OBJECT_CHAR);
                    out.spans.push((start..out.text.len(), Span::Object { w, asc: h * 0.85, desc: h * 0.15 }));
                    let alt: String = node.children.iter().map(Node::plain_text).collect();
                    out.objects.push((start, Obj::Image(url.clone(), w, h), format!("![{alt}]({url})")));
                }
                Kind::Image { url } => {
                    let alt: String = node.children.iter().map(Node::plain_text).collect();
                    out.text.push_str(&format!("🖼 {}", if alt.is_empty() { url.as_str() } else { alt.as_str() }));
                    out.spans.push((start..out.text.len(), Span::Color(theme.muted)));
                }
                kind => {
                    self.flatten(&node.children, out, size);
                    let r = start..out.text.len();
                    match kind {
                        Kind::Emph => out.spans.push((r, Span::Italic)),
                        Kind::Strong => out.spans.push((r, Span::Bold)),
                        Kind::Strike => out.spans.push((r, Span::Strike)),
                        Kind::Sup | Kind::Sub => {
                            let sup = matches!(kind, Kind::Sup);
                            // Only plain text inside: swap for Unicode script characters.
                            let plain = !out.spans.iter().any(|(s, _)| s.start >= r.start && s.start < r.end.max(r.start + 1))
                                && !out.links.iter().any(|(s, _)| s.start >= r.start)
                                && !out.objects.iter().any(|(b, _, _)| *b >= r.start);
                            match unicode_script(&out.text[r.clone()], sup).filter(|_| plain) {
                                Some(s) => out.text.replace_range(r, &s),
                                None => {
                                    out.spans.push((r.clone(), Span::Scale(0.75)));
                                    out.spans.push((r, if sup { Span::Super } else { Span::Sub }));
                                }
                            }
                        }
                        Kind::Mark => out.backgrounds.push((r, theme.mark_bg)),
                        Kind::Ins => {
                            out.spans.push((r.clone(), Span::Underline));
                            out.spans.push((r, Span::Color(theme.ins)));
                        }
                        Kind::Abbr { title } => {
                            // Dotted-underline look via underline + muted colour; hover shows the expansion.
                            out.spans.push((r.clone(), Span::Underline));
                            out.links.push((r, format!("abbr:{title}")));
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

    /// Build a paragraph and add it as a block; returns its height.
    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, inline: Inline, style: &BaseStyle, x: f32, y: f32, width: f32, align: Option<TextAlign>, selectable: bool, join: Join, out: &mut DocLayout<T::Para>) -> f32 {
        let mut para = self.ts.paragraph(&inline.text, &inline.spans, style, width.max(1.0));
        if let Some(a) = align {
            para.set_align(a);
        }
        self.place(para, inline, style.color, x, y, selectable, join, out)
    }

    #[allow(clippy::too_many_arguments)]
    fn place(&mut self, para: T::Para, inline: Inline, color: Ink, x: f32, y: f32, selectable: bool, join: Join, out: &mut DocLayout<T::Para>) -> f32 {
        for (range, bg) in &inline.backgrounds {
            // Skip slivers: a code span's leading thin space can stay at the end of
            // the previous line and would draw as a stray vertical mark.
            for r in para.range_rects(range.clone()).into_iter().filter(|r| r.w > 3.0) {
                out.rects.push(Rect { r: RectF { x: x + r.x, y: y + r.y + 1.0, w: r.w, h: r.h - 2.0 }, color: *bg, radius: 4.0 });
            }
        }
        let mut object_sources = Vec::with_capacity(inline.objects.len());
        for (byte, obj, source) in inline.objects {
            let (ox, oy) = para.object_origin(byte);
            match obj {
                Obj::Math(mbox) => out.math.push(MathPlacement { x: x + ox, y: y + oy + mbox.asc, mbox, color }),
                Obj::Image(url, w, h) => out.images.push(ImagePlacement { url, r: RectF { x: x + ox, y: y + oy, w, h } }),
            }
            object_sources.push((byte, source));
        }
        let (w, h) = para.size();
        for (b, id) in &inline.anchors {
            let ry = para.range_rects(*b..*b + 1).first().map_or(0.0, |r| r.y);
            out.anchors.push((id.clone(), y + ry - self.base * 2.0));
        }
        out.blocks.push(TextBlock { para, text: inline.text, x, y, w, h, selectable, join, links: inline.links, object_sources });
        h
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
    pub fn caret(pos: DocPos) -> Self {
        DocSelection { anchor: pos, focus: pos }
    }
    pub fn ordered(&self) -> (DocPos, DocPos) {
        if self.anchor <= self.focus { (self.anchor, self.focus) } else { (self.focus, self.anchor) }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl<P: Paragraph> DocLayout<P> {
    /// Nearest caret position in a selectable block for a document point.
    pub fn hit(&self, x: f32, y: f32) -> Option<DocPos> {
        let mut best: Option<(f32, usize)> = None;
        for (i, b) in self.blocks.iter().enumerate() {
            if !b.selectable {
                continue;
            }
            let dy = if y < b.y { b.y - y } else if y > b.y + b.h { y - b.y - b.h } else { 0.0 };
            let right = b.x + b.w.max(1.0);
            let dx = if x < b.x { b.x - x } else if x > right { x - right } else { 0.0 };
            let d = dy * 1000.0 + dx; // same line first, then nearest column
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, i));
            }
            if b.y > y + 2000.0 {
                break;
            }
        }
        let (_, i) = best?;
        let b = &self.blocks[i];
        Some(DocPos { block: i, byte: b.para.hit(x - b.x, y - b.y).min(b.text.len()) })
    }

    pub fn word_at(&self, pos: DocPos) -> DocSelection {
        let text = &self.blocks[pos.block].text;
        let mut start = pos.byte;
        let mut end = pos.byte;
        let next = text[end..].chars().next();
        if next.is_some_and(is_word_char) {
            while let Some(c) = text[..start].chars().next_back().filter(|c| is_word_char(*c)) {
                start -= c.len_utf8();
            }
            while let Some(c) = text[end..].chars().next().filter(|c| is_word_char(*c)) {
                end += c.len_utf8();
            }
        } else if let Some(c) = next {
            end += c.len_utf8();
        }
        DocSelection { anchor: DocPos { block: pos.block, byte: start }, focus: DocPos { block: pos.block, byte: end } }
    }

    pub fn block_selection(&self, block: usize) -> DocSelection {
        DocSelection { anchor: DocPos { block, byte: 0 }, focus: DocPos { block, byte: self.blocks[block].text.len() } }
    }

    pub fn select_all(&self) -> Option<DocSelection> {
        let first = self.blocks.iter().position(|b| b.selectable)?;
        let last = self.blocks.iter().rposition(|b| b.selectable)?;
        Some(DocSelection { anchor: DocPos { block: first, byte: 0 }, focus: DocPos { block: last, byte: self.blocks[last].text.len() } })
    }

    pub fn block_range(&self, sel: &DocSelection, block: usize) -> Option<Range<usize>> {
        let (start, end) = sel.ordered();
        if block < start.block || block > end.block || !self.blocks[block].selectable {
            return None;
        }
        let a = if block == start.block { start.byte } else { 0 };
        let b = if block == end.block { end.byte } else { self.blocks[block].text.len() };
        (a < b).then_some(a..b)
    }

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
            let block = &self.blocks[i];
            let slice = &block.text[r.clone()];
            if block.object_sources.is_empty() {
                out.push_str(slice);
            } else {
                // Inline objects copy as their source (`$x^2$`).
                for (off, c) in slice.char_indices() {
                    match block.object_sources.iter().find(|(b, _)| *b == r.start + off) {
                        Some((_, src)) if c == OBJECT_CHAR => out.push_str(src),
                        _ => out.push(c),
                    }
                }
            }
        }
        out
    }

    pub fn link_at(&self, x: f32, y: f32) -> Option<&str> {
        let pos = self.hit(x, y)?;
        let b = &self.blocks[pos.block];
        if y < b.y || y > b.y + b.h || x < b.x || x > b.x + b.w {
            return None;
        }
        b.links.iter().find(|(r, _)| r.contains(&pos.byte)).map(|(_, u)| u.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monospace fake typesetter: 1 char = 10 px, 20 px lines, no wrapping.
    struct Fake;
    struct FakePara(String);
    impl Paragraph for FakePara {
        fn size(&self) -> (f32, f32) {
            (self.0.chars().count() as f32 * 10.0, 20.0)
        }
        fn content_widths(&self) -> (f32, f32) {
            let s = self.size().0;
            (s, s)
        }
        fn set_width(&mut self, _: f32) {}
        fn set_align(&mut self, _: TextAlign) {}
        fn hit(&self, x: f32, _: f32) -> usize {
            self.0.char_indices().nth((x / 10.0).round().max(0.0) as usize).map_or(self.0.len(), |(i, _)| i)
        }
        fn range_rects(&self, r: Range<usize>) -> Vec<RectF> {
            vec![RectF { x: r.start as f32 * 10.0, y: 0.0, w: (r.end - r.start) as f32 * 10.0, h: 20.0 }]
        }
        fn object_origin(&self, byte: usize) -> (f32, f32) {
            (byte as f32 * 10.0, 0.0)
        }
    }
    impl Typesetter for Fake {
        type Para = FakePara;
        fn paragraph(&mut self, text: &str, _: &[(Range<usize>, Span)], _: &BaseStyle, _: f32) -> FakePara {
            FakePara(text.to_string())
        }
    }

    fn lay(src: &str) -> DocLayout<FakePara> {
        let doc = crate::ast::parse(src);
        Layouter { ts: &mut Fake, theme: &Theme::light(), base: 16.0 }.layout(&doc, 0.0, 600.0, 0.0)
    }

    #[test]
    fn copy_joins_blocks() {
        let l = lay("Hello world\n\nSecond para\n\n| a | b |\n|---|---|\n| 1 | 2 |\n");
        let all = l.select_all().unwrap();
        assert_eq!(l.selected_text(&all), "Hello world\n\nSecond para\n\na\tb\n1\t2");
    }

    #[test]
    fn word_selection() {
        let l = lay("Hello world");
        let s = l.word_at(DocPos { block: 0, byte: 7 });
        assert_eq!(l.selected_text(&s), "world");
    }

    #[test]
    fn hit_picks_nearest_block() {
        let l = lay("aaa\n\nbbb");
        let p = l.hit(15.0, l.blocks[1].y + 5.0).unwrap();
        assert_eq!(p, DocPos { block: 1, byte: 2 });
    }

    #[test]
    fn list_markers_not_selectable() {
        let l = lay("- one\n- two");
        let all = l.select_all().unwrap();
        assert_eq!(l.selected_text(&all), "one\ntwo");
    }
}

/// Unicode super-/subscript form of `s`, if every character has one. Used for
/// `x^2^`, `H~2~O` and footnote marks, because common UI fonts (Segoe UI) do
/// not implement the OpenType `sups`/`subs` features.
fn unicode_script(s: &str, sup: bool) -> Option<String> {
    const SUP: [(char, char); 22] = [
        ('0', '⁰'), ('1', '¹'), ('2', '²'), ('3', '³'), ('4', '⁴'), ('5', '⁵'), ('6', '⁶'), ('7', '⁷'), ('8', '⁸'), ('9', '⁹'), ('+', '⁺'),
        ('-', '⁻'), ('=', '⁼'), ('(', '⁽'), (')', '⁾'), ('n', 'ⁿ'), ('i', 'ⁱ'), ('a', 'ᵃ'), ('b', 'ᵇ'), ('c', 'ᶜ'), ('d', 'ᵈ'), ('e', 'ᵉ'),
    ];
    const SUB: [(char, char); 25] = [
        ('0', '₀'), ('1', '₁'), ('2', '₂'), ('3', '₃'), ('4', '₄'), ('5', '₅'), ('6', '₆'), ('7', '₇'), ('8', '₈'), ('9', '₉'), ('+', '₊'),
        ('-', '₋'), ('=', '₌'), ('(', '₍'), (')', '₎'), ('a', 'ₐ'), ('e', 'ₑ'), ('o', 'ₒ'), ('x', 'ₓ'), ('h', 'ₕ'), ('k', 'ₖ'), ('l', 'ₗ'),
        ('m', 'ₘ'), ('n', 'ₙ'), ('i', 'ᵢ'),
    ];
    let table: &[(char, char)] = if sup { &SUP } else { &SUB };
    s.chars().map(|c| table.iter().find(|(a, _)| *a == c).map(|(_, b)| *b)).collect()
}

#[cfg(test)]
mod script_tests {
    #[test]
    fn scripts() {
        assert_eq!(super::unicode_script("2", false).as_deref(), Some("₂"));
        assert_eq!(super::unicode_script("10", true).as_deref(), Some("¹⁰"));
        assert_eq!(super::unicode_script("xyz", true), None);
    }
}
