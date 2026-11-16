//! TeX-style math layout driven by an OpenType MATH table (Cambria Math on
//! Windows, STIX Two / Latin Modern Math elsewhere). Produces positioned glyph
//! ids and rules; the platform draws them with its own glyph renderer.
//!
//! Coordinates: x to the right, y DOWN, relative to the formula's baseline.

use super::parse::{self, Class, M};
use super::symbols::styled;
use crate::layout::RectF;
use super::otf::Font;

/// Named access to the MathConstants table (only the fields layout uses).
#[derive(Clone, Copy)]
struct Constants(Font);

#[rustfmt::skip]
#[allow(dead_code)]
impl Constants {
    fn script_percent_scale_down(&self) -> i16 { self.0.const_i16(0) }
    fn script_script_percent_scale_down(&self) -> i16 { self.0.const_i16(2) }
    fn display_operator_min_height(&self) -> u16 { self.0.const_i16(6) as u16 }
    fn radical_degree_bottom_raise_percent(&self) -> i16 { self.0.const_i16(212) }
    fn axis_height(&self) -> i16 {
        self.0.value(1)
    }
    fn accent_base_height(&self) -> i16 {
        self.0.value(2)
    }
    fn subscript_shift_down(&self) -> i16 {
        self.0.value(4)
    }
    fn subscript_top_max(&self) -> i16 {
        self.0.value(5)
    }
    fn subscript_baseline_drop_min(&self) -> i16 {
        self.0.value(6)
    }
    fn superscript_shift_up(&self) -> i16 {
        self.0.value(7)
    }
    fn superscript_shift_up_cramped(&self) -> i16 {
        self.0.value(8)
    }
    fn superscript_bottom_min(&self) -> i16 {
        self.0.value(9)
    }
    fn superscript_baseline_drop_max(&self) -> i16 {
        self.0.value(10)
    }
    fn sub_superscript_gap_min(&self) -> i16 {
        self.0.value(11)
    }
    fn superscript_bottom_max_with_subscript(&self) -> i16 {
        self.0.value(12)
    }
    fn space_after_script(&self) -> i16 {
        self.0.value(13)
    }
    fn upper_limit_gap_min(&self) -> i16 {
        self.0.value(14)
    }
    fn upper_limit_baseline_rise_min(&self) -> i16 {
        self.0.value(15)
    }
    fn lower_limit_gap_min(&self) -> i16 {
        self.0.value(16)
    }
    fn lower_limit_baseline_drop_min(&self) -> i16 {
        self.0.value(17)
    }
    fn stack_top_shift_up(&self) -> i16 {
        self.0.value(18)
    }
    fn stack_top_display_style_shift_up(&self) -> i16 {
        self.0.value(19)
    }
    fn stack_bottom_shift_down(&self) -> i16 {
        self.0.value(20)
    }
    fn stack_bottom_display_style_shift_down(&self) -> i16 {
        self.0.value(21)
    }
    fn stack_gap_min(&self) -> i16 {
        self.0.value(22)
    }
    fn stack_display_style_gap_min(&self) -> i16 {
        self.0.value(23)
    }
    fn fraction_numerator_shift_up(&self) -> i16 {
        self.0.value(28)
    }
    fn fraction_numerator_display_style_shift_up(&self) -> i16 {
        self.0.value(29)
    }
    fn fraction_denominator_shift_down(&self) -> i16 {
        self.0.value(30)
    }
    fn fraction_denominator_display_style_shift_down(&self) -> i16 {
        self.0.value(31)
    }
    fn fraction_numerator_gap_min(&self) -> i16 {
        self.0.value(32)
    }
    fn fraction_num_display_style_gap_min(&self) -> i16 {
        self.0.value(33)
    }
    fn fraction_rule_thickness(&self) -> i16 {
        self.0.value(34)
    }
    fn fraction_denominator_gap_min(&self) -> i16 {
        self.0.value(35)
    }
    fn fraction_denom_display_style_gap_min(&self) -> i16 {
        self.0.value(36)
    }
    fn overbar_vertical_gap(&self) -> i16 {
        self.0.value(39)
    }
    fn overbar_rule_thickness(&self) -> i16 {
        self.0.value(40)
    }
    fn overbar_extra_ascender(&self) -> i16 {
        self.0.value(41)
    }
    fn underbar_vertical_gap(&self) -> i16 {
        self.0.value(42)
    }
    fn underbar_rule_thickness(&self) -> i16 {
        self.0.value(43)
    }
    fn underbar_extra_descender(&self) -> i16 {
        self.0.value(44)
    }
    fn radical_vertical_gap(&self) -> i16 {
        self.0.value(45)
    }
    fn radical_display_style_vertical_gap(&self) -> i16 {
        self.0.value(46)
    }
    fn radical_rule_thickness(&self) -> i16 {
        self.0.value(47)
    }
    fn radical_extra_ascender(&self) -> i16 {
        self.0.value(48)
    }
    fn radical_kern_before_degree(&self) -> i16 {
        self.0.value(49)
    }
    fn radical_kern_after_degree(&self) -> i16 {
        self.0.value(50)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Item {
    Glyph { id: u16, x: f32, y: f32, size: f32, color: Option<[u8; 3]> },
    Rule { r: RectF, color: Option<[u8; 3]> },
}

#[derive(Debug, Clone, Default)]
pub struct MathBox {
    pub w: f32,
    pub asc: f32,
    pub desc: f32,
    pub items: Vec<Item>,
}

pub struct MathFont {
    face: Font,
    upem: f32,
    /// Laid-out formulas by (hash of source, size, display), sorted by that hash: a document
    /// repeats formulas (big.md: 1480, 36 distinct) and every relayout (resize, zoom back,
    /// idle completion) needs them all again.
    cache: core::cell::RefCell<Vec<(u64, (Box<str>, u32, bool), MathBox)>>,
}

/// Entries kept at most; beyond that the cache starts over.
const CACHE_MAX: usize = 4096;

impl MathFont {
    /// Font file data and face offset (see [`super::otf::faces`]); leaked once per process.
    pub fn new(data: Vec<u8>, face: usize) -> Option<Self> {
        let face = Font::new(data, face)?;
        Some(MathFont { upem: face.upem, face, cache: Default::default() })
    }

    /// Whether the font lacks glyf outlines (CFF), so the platform must measure the glyphs.
    #[cfg_attr(windows, allow(dead_code))]
    pub fn needs_bboxes(&self) -> bool {
        self.face.bbox_source_missing()
    }

    /// Glyph bounding boxes measured by the platform, for a CFF font (see `needs_bboxes`).
    #[cfg_attr(windows, allow(dead_code))]
    pub fn set_bboxes(&mut self, b: Vec<[i16; 4]>) {
        self.face.set_bboxes(Box::leak(b.into_boxed_slice()));
    }

    #[cfg_attr(windows, allow(dead_code))]
    pub fn upem(&self) -> f32 {
        self.upem
    }

    /// `src` laid out at `size` (display or inline style); repeated formulas come from the cache.
    pub fn layout(&self, src: &str, size: f32, display: bool) -> MathBox {
        let key = src.bytes().chain(size.to_bits().to_le_bytes()).chain([display as u8]).fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
        let cacheable = {
            let cache = self.cache.borrow();
            match cache.binary_search_by_key(&key, |e| e.0) {
                Ok(i) if (&*cache[i].1.0, cache[i].1.1, cache[i].1.2) == (src, size.to_bits(), display) => return cache[i].2.clone(),
                Ok(_) => false, // another formula with the same hash: not cached
                Err(_) => true,
            }
        };
        let b = self.layout_uncached(src, size, display);
        if cacheable {
            let mut cache = self.cache.borrow_mut();
            if cache.len() >= CACHE_MAX {
                cache.clear();
            }
            if let Err(i) = cache.binary_search_by_key(&key, |e| e.0) {
                cache.insert(i, (key, (src.into(), size.to_bits(), display), b.clone()));
            }
        }
        b
    }

    fn layout_uncached(&self, src: &str, size: f32, display: bool) -> MathBox {
        let tree = super::parse::parse(src);
        let mut l = Ctx { f: self, base: size };
        let st = Style { level: if display { 0 } else { 1 }, cramped: false };
        let b = l.node(&tree, st);
        MathBox { w: b.w, asc: b.asc, desc: b.desc, items: b.items }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Style {
    /// 0 = display, 1 = text, 2 = script, 3 = scriptscript
    level: u8,
    cramped: bool,
}

impl Style {
    fn display(self) -> bool {
        self.level == 0
    }
    fn sup(self) -> Style {
        Style { level: if self.level < 2 { 2 } else { 3 }, cramped: self.cramped }
    }
    fn sub(self) -> Style {
        Style { cramped: true, ..self.sup() }
    }
    fn frac_num(self) -> Style {
        Style { level: match self.level { 0 => 1, 1 => 2, _ => 3 }, cramped: self.cramped }
    }
    fn frac_den(self) -> Style {
        Style { cramped: true, ..self.frac_num() }
    }
    fn cramp(self) -> Style {
        Style { cramped: true, ..self }
    }
}

#[derive(Clone, Default, Debug)]
struct B {
    w: f32,
    asc: f32,
    desc: f32,
    italic: f32,
    items: Vec<Item>,
    /// Glyph id if this box is a single glyph (for accent attachment / italic correction).
    glyph: Option<u16>,
}

impl B {
    fn shift(&mut self, dx: f32, dy: f32) {
        for it in &mut self.items {
            match it {
                Item::Glyph { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                Item::Rule { r, .. } => {
                    r.x += dx;
                    r.y += dy;
                }
            }
        }
    }
    /// Append `other` at horizontal offset `dx` and baseline offset `dy` (down).
    fn add(&mut self, mut other: B, dx: f32, dy: f32) {
        self.asc = self.asc.max(other.asc - dy);
        self.desc = self.desc.max(other.desc + dy);
        self.w = self.w.max(dx + other.w);
        other.shift(dx, dy);
        self.items.extend(other.items);
        self.glyph = None;
    }
    fn rule(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.items.push(Item::Rule { r: RectF { x, y, w, h }, color: None });
        self.asc = self.asc.max(-y);
        self.desc = self.desc.max(y + h);
        self.w = self.w.max(x + w);
    }
    fn colorize(&mut self, c: [u8; 3]) {
        for it in &mut self.items {
            match it {
                Item::Glyph { color, .. } | Item::Rule { color, .. } => {
                    color.get_or_insert(c);
                }
            }
        }
    }
}

struct Ctx<'f> {
    f: &'f MathFont,
    base: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sp {
    None,
    Thin,
    Med,
    Thick,
}

fn spacing(left: Class, right: Class, script: bool) -> Sp {
    use Class::*;
    let sp = match (left, right) {
        (Ord, Op) | (Ord, Inner) | (Op, Ord) | (Op, Op) | (Op, Inner) => Sp::Thin,
        (Ord, Bin) | (Bin, Ord) | (Bin, Op) | (Bin, Open) | (Bin, Inner) | (Close, Bin) | (Inner, Bin) => Sp::Med,
        (Ord, Rel) | (Op, Rel) | (Rel, Ord) | (Rel, Op) | (Rel, Open) | (Rel, Inner) | (Close, Rel) | (Inner, Rel) => Sp::Thick,
        (Close, Op) | (Close, Inner) | (Inner, Ord) | (Inner, Op) | (Inner, Open) | (Inner, Punct) | (Inner, Inner) => Sp::Thin,
        (Punct, Ord) | (Punct, Op) | (Punct, Rel) | (Punct, Open) | (Punct, Close) | (Punct, Punct) | (Punct, Inner) => Sp::Thin,
        _ => Sp::None,
    };
    if script && matches!(sp, Sp::Med | Sp::Thick) { Sp::None } else { sp }
}

fn class_of(m: &M) -> Option<Class> {
    Some(match m {
        M::Sym(_, c) | M::Text(_, c, _) | M::BigDelim(_, _, c) => *c,
        M::BigOp { .. } | M::LimitFunc(_) => Class::Op,
        M::Frac { .. } | M::Delimited { .. } | M::Matrix { .. } => Class::Inner,
        M::Scripts { base, .. } => class_of(base).unwrap_or(Class::Ord),
        M::Space(_) => return None,
        M::Color(_, inner) => class_of(inner).unwrap_or(Class::Ord),
        M::Row(items) if items.len() == 1 => class_of(&items[0]).unwrap_or(Class::Ord),
        _ => Class::Ord,
    })
}

impl Ctx<'_> {
    fn size(&self, st: Style) -> f32 {
        let c = self.consts();
        match st.level {
            0 | 1 => self.base,
            2 => self.base * c.script_percent_scale_down() as f32 / 100.0,
            _ => self.base * c.script_script_percent_scale_down() as f32 / 100.0,
        }
    }
    fn consts(&self) -> Constants {
        Constants(self.f.face)
    }
    /// Font units → px at style size.
    fn u(&self, v: f32, st: Style) -> f32 {
        v * self.size(st) / self.f.upem
    }
    fn c(&self, st: Style, get: impl Fn(&Constants) -> i16) -> f32 {
        self.u(get(&self.consts()) as f32, st)
    }
    fn axis(&self, st: Style) -> f32 {
        self.c(st, |c| c.axis_height())
    }

    fn glyph_id(&self, c: char) -> u16 {
        self.f.face.glyph_index(c).or_else(|| self.f.face.glyph_index('?')).unwrap_or(0)
    }

    fn glyph_box(&self, id: u16, size: f32) -> B {
        let face = &self.f.face;
        let k = size / self.f.upem;
        let adv = face.advance(id).unwrap_or(0) as f32 * k;
        let (asc, desc) = face.bbox(id).map_or((0.0, 0.0), |r| (r[3] as f32 * k, -(r[1] as f32) * k));
        let italic = face.italic_correction(id).map_or(0.0, |v| v as f32 * k);
        B { w: adv, asc, desc, italic, items: vec![Item::Glyph { id, x: 0.0, y: 0.0, size, color: None }], glyph: Some(id) }
    }

    fn char_box(&self, c: char, st: Style) -> B {
        self.glyph_box(self.glyph_id(c), self.size(st))
    }

    /// Glyph stretched to at least `target` (height if vertical, width if not).
    fn stretched(&self, c: char, target: f32, vertical: bool, size: f32) -> B {
        let id = self.glyph_id(c);
        let k = size / self.f.upem;
        let Some(construction) = self.f.face.construction(id, vertical) else { return self.glyph_box(id, size) };
        let mut best = id;
        for &(glyph, advance) in &construction.variants {
            best = glyph;
            if advance as f32 * k >= target {
                return self.glyph_box(best, size);
            }
        }
        let Some((assembly_italic, parts)) = construction.assembly else { return self.glyph_box(best, size) };
        if parts.is_empty() {
            return self.glyph_box(best, size);
        }
        let overlap = self.f.face.min_connector_overlap() as f32 * k;
        // Repeat extenders until the assembly is long enough.
        let mut reps = 0;
        let sequence = loop {
            let seq: Vec<_> = parts
                .iter()
                .flat_map(|p| std::iter::repeat_n(p, if p.extender { reps } else { 1 }))
                .collect();
            let total: f32 = seq.iter().map(|p| p.full_advance as f32 * k).sum::<f32>() - overlap * (seq.len().saturating_sub(1)) as f32;
            if total >= target || reps > 64 || !parts.iter().any(|p| p.extender) {
                break seq;
            }
            reps += 1;
        };
        let mut b = B::default();
        let mut pos = 0.0f32;
        for p in &sequence {
            let g = self.glyph_box(p.glyph, size);
            if vertical {
                // Parts go bottom → top; pos is the distance of the part's bottom from the box bottom.
                let bottom_to_baseline = -g.desc; // glyph's own bottom relative to its baseline (y up)
                let dy = -(pos - bottom_to_baseline); // y down
                b.add(g, 0.0, dy);
            } else {
                b.add(g, pos, 0.0);
            }
            pos += p.full_advance as f32 * k - overlap;
        }
        b.italic = assembly_italic as f32 * k;
        b
    }

    /// Re-centre a box vertically on the math axis.
    fn center_on_axis(&self, mut b: B, st: Style) -> B {
        let h = b.asc + b.desc;
        let a = self.axis(st);
        let new_asc = h / 2.0 + a;
        let dy = new_asc - b.asc; // move up by dy
        b.shift(0.0, -dy);
        b.asc = new_asc;
        b.desc = h - new_asc;
        b
    }

    fn row(&mut self, items: &[M], st: Style) -> B {
        let mut out = B::default();
        let mut x = 0.0;
        let mut prev: Option<Class> = None;
        let script = st.level >= 2;
        let classes: Vec<Option<Class>> = items.iter().map(class_of).collect();
        let single = items.len() == 1;
        for (i, m) in items.iter().enumerate() {
            let mut class = classes[i];
            // A binary operator in a non-binary position becomes ordinary (unary minus).
            if class == Some(Class::Bin) {
                let next = classes[i + 1..].iter().flatten().next().copied();
                if matches!(prev, None | Some(Class::Bin | Class::Op | Class::Rel | Class::Open | Class::Punct))
                    || matches!(next, None | Some(Class::Rel | Class::Close | Class::Punct))
                {
                    class = Some(Class::Ord);
                }
            }
            if let (Some(p), Some(c)) = (prev, class) {
                let em = self.size(st);
                x += match spacing(p, c, script) {
                    Sp::None => 0.0,
                    Sp::Thin => em * 3.0 / 18.0,
                    Sp::Med => em * 4.0 / 18.0,
                    Sp::Thick => em * 5.0 / 18.0,
                };
            }
            let b = self.node(m, st);
            let w = b.w;
            let glyph = b.glyph;
            let italic = b.italic;
            out.add(b, x, 0.0);
            x += w;
            if single {
                out.glyph = glyph;
                out.italic = italic;
            }
            if class.is_some() {
                prev = class;
            }
        }
        out.w = out.w.max(x);
        out
    }

    /// Text run; letters and digits mapped to `font` (Upright leaves them as they are).
    fn text(&mut self, s: &str, font: parse::Font, st: Style) -> B {
        let mut b = B::default();
        let mut x = 0.0;
        for c in s.chars() {
            let g = if c == ' ' { B { w: self.size(st) * 0.25, ..Default::default() } } else { self.char_box(styled(c, font), st) };
            let w = g.w;
            b.add(g, x, 0.0);
            x += w;
        }
        b.w = x;
        b
    }

    fn node(&mut self, m: &M, st: Style) -> B {
        match m {
            M::Row(items) => self.row(items, st),
            M::Sym(c, _) => self.char_box(*c, st),
            M::Text(s, _, font) => self.text(s, *font, st),
            M::Space(em) => B { w: em * self.size(st), ..Default::default() },
            M::Error(s) => {
                let mut b = self.text(s, parse::Font::Upright, st);
                b.colorize([0xd1, 0x24, 0x2f]);
                b
            }
            M::Color(c, inner) => {
                let mut b = self.node(inner, st);
                b.colorize(*c);
                b
            }
            M::Frac { num, den, rule, delims } => {
                let f = self.frac(num, den, *rule, st);
                match delims {
                    Some((l, r)) => self.delimited(Some(*l), Some(*r), f, st),
                    None => f,
                }
            }
            M::Sqrt { degree, body } => self.sqrt(degree.as_deref(), body, st),
            M::Scripts { base, sup, sub } => self.scripts(base, sup.as_deref(), sub.as_deref(), st),
            M::BigOp { sym, .. } => self.big_op(*sym, st),
            M::LimitFunc(name) => self.text(name, parse::Font::Upright, st),
            M::Delimited { left, right, body } => {
                let b = self.node(body, st);
                self.delimited(*left, *right, b, st)
            }
            M::BigDelim(c, factor, _) => {
                let b = self.stretched(*c, factor * self.size(st), true, self.size(st));
                self.center_on_axis(b, st)
            }
            M::Accent { accent, body, stretchy } => self.accent(*accent, body, *stretchy, st),
            M::OverLine(body) => {
                let mut b = self.node(body, st.cramp());
                let gap = self.c(st, |c| c.overbar_vertical_gap());
                let t = self.c(st, |c| c.overbar_rule_thickness());
                let extra = self.c(st, |c| c.overbar_extra_ascender());
                let top = b.asc + gap + t;
                let w = b.w;
                b.rule(0.0, -top, w, t);
                b.asc = top + extra;
                b
            }
            M::UnderLine(body) => {
                let mut b = self.node(body, st);
                let gap = self.c(st, |c| c.underbar_vertical_gap());
                let t = self.c(st, |c| c.underbar_rule_thickness());
                let extra = self.c(st, |c| c.underbar_extra_descender());
                let y = b.desc + gap;
                let w = b.w;
                b.rule(0.0, y, w, t);
                b.desc = y + t + extra;
                b
            }
            M::Brace { over, body } => {
                let body = self.node(body, st);
                let brace = self.stretched(if *over { '⏞' } else { '⏟' }, body.w, false, self.size(st));
                let gap = self.size(st) * 0.1;
                let mut b = B::default();
                let bx = (body.w - brace.w) / 2.0;
                let (body_asc, body_desc) = (body.asc, body.desc);
                b.add(body, bx.min(0.0).abs(), 0.0);
                if *over {
                    let dy = -(body_asc + gap + brace.desc);
                    b.add(brace, bx.max(0.0), dy);
                } else {
                    let dy = body_desc + gap + brace.asc;
                    b.add(brace, bx.max(0.0), dy);
                }
                b
            }
            M::Matrix { rows, left, right, align_left, cases } => {
                let b = self.matrix(rows, *align_left, *cases, st);
                if left.is_some() || right.is_some() { self.delimited(*left, *right, b, st) } else { b }
            }
        }
    }

    fn frac(&mut self, num: &M, den: &M, rule: bool, st: Style) -> B {
        let num = self.node(num, st.frac_num());
        let den = self.node(den, st.frac_den());
        let d = st.display();
        let a = self.axis(st);
        let mut b = B::default();
        let (mut up, mut down);
        if rule {
            let t = self.c(st, |c| c.fraction_rule_thickness());
            up = if d { self.c(st, |c| c.fraction_numerator_display_style_shift_up()) } else { self.c(st, |c| c.fraction_numerator_shift_up()) };
            down = if d { self.c(st, |c| c.fraction_denominator_display_style_shift_down()) } else { self.c(st, |c| c.fraction_denominator_shift_down()) };
            let gap_n = if d { self.c(st, |c| c.fraction_num_display_style_gap_min()) } else { self.c(st, |c| c.fraction_numerator_gap_min()) };
            let gap_d = if d { self.c(st, |c| c.fraction_denom_display_style_gap_min()) } else { self.c(st, |c| c.fraction_denominator_gap_min()) };
            up = up.max(gap_n + num.desc + a + t / 2.0);
            down = down.max(gap_d + den.asc - a + t / 2.0);
            let pad = self.size(st) * 0.08;
            let w = num.w.max(den.w) + 2.0 * pad;
            b.rule(0.0, -(a + t / 2.0), w, t);
            let (nw, dw) = (num.w, den.w);
            b.add(num, (w - nw) / 2.0, -up);
            b.add(den, (w - dw) / 2.0, down);
        } else {
            up = if d { self.c(st, |c| c.stack_top_display_style_shift_up()) } else { self.c(st, |c| c.stack_top_shift_up()) };
            down = if d { self.c(st, |c| c.stack_bottom_display_style_shift_down()) } else { self.c(st, |c| c.stack_bottom_shift_down()) };
            let gap = if d { self.c(st, |c| c.stack_display_style_gap_min()) } else { self.c(st, |c| c.stack_gap_min()) };
            let actual = (up - num.desc) - (den.asc - down);
            if actual < gap {
                up += (gap - actual) / 2.0;
                down += (gap - actual) / 2.0;
            }
            let w = num.w.max(den.w);
            let (nw, dw) = (num.w, den.w);
            b.add(num, (w - nw) / 2.0, -up);
            b.add(den, (w - dw) / 2.0, down);
        }
        b
    }

    fn sqrt(&mut self, degree: Option<&M>, body: &M, st: Style) -> B {
        let body = self.node(body, st.cramp());
        let t = self.c(st, |c| c.radical_rule_thickness());
        let mut gap = if st.display() { self.c(st, |c| c.radical_display_style_vertical_gap()) } else { self.c(st, |c| c.radical_vertical_gap()) };
        let extra = self.c(st, |c| c.radical_extra_ascender());
        let target = body.asc + body.desc + gap + t;
        let mut rad = self.stretched('√', target, true, self.size(st));
        let rad_h = rad.asc + rad.desc;
        if rad_h > target {
            gap += (rad_h - target) / 2.0;
        }
        let top = body.asc + gap + t;
        // Move the radical so its top is at `top` above the baseline.
        let dy = top - rad.asc;
        rad.shift(0.0, -dy);
        rad.asc = top;
        rad.desc = rad_h - top;

        let mut b = B::default();
        let mut x = 0.0;
        if let Some(deg) = degree {
            let deg = self.node(deg, Style { level: 3, cramped: true });
            let before = self.c(st, |c| c.radical_kern_before_degree());
            let after = self.c(st, |c| c.radical_kern_after_degree());
            let raise = self.consts().radical_degree_bottom_raise_percent() as f32 / 100.0 * rad_h;
            let deg_w = deg.w;
            let deg_desc = deg.desc;
            b.add(deg, before, -(raise - rad.desc) - deg_desc);
            x = (before + deg_w + after).max(0.0);
        }
        let rw = rad.w;
        b.add(rad, x, 0.0);
        let bw = body.w;
        b.rule(x + rw, -top, bw, t);
        b.add(body, x + rw, 0.0);
        b.asc = b.asc.max(top + extra);
        b
    }

    fn big_op(&mut self, sym: char, st: Style) -> B {
        let size = self.size(st);
        let b = if st.display() {
            // Some fonts (Cambria Math's ∫) already meet DisplayOperatorMinHeight with the
            // base glyph; display operators must still use a larger variant.
            let min = self.u(self.consts().display_operator_min_height() as f32, st);
            let base = self.char_box(sym, st);
            self.stretched(sym, min.max((base.asc + base.desc) * 1.3), true, size)
        } else {
            self.char_box(sym, st)
        };
        self.center_on_axis(b, st)
    }

    fn scripts(&mut self, base: &M, sup: Option<&M>, sub: Option<&M>, st: Style) -> B {
        let limits = match base {
            M::BigOp { limits: Some(l), .. } => *l,
            M::BigOp { integral, .. } => st.display() && !integral,
            M::LimitFunc(_) => st.display(),
            _ => false,
        };
        let is_integral = matches!(base, M::BigOp { integral: true, .. });
        let base = self.node(base, st);
        if limits {
            return self.limits(base, sup, sub, st);
        }
        let mut b = B::default();
        let base_w = base.w;
        let italic = base.italic;
        let (base_asc, base_desc) = (base.asc, base.desc);
        let base_is_glyph = base.glyph.is_some();
        b.add(base, 0.0, 0.0);
        let sup_box = sup.map(|m| self.node(m, st.sup()));
        let sub_box = sub.map(|m| self.node(m, st.sub()));
        // Drops are measured from the base for compound bases only (TeX rule).
        let (drop_max, drop_min) = if base_is_glyph { (0.0, 0.0) } else {
            (self.c(st, |c| c.superscript_baseline_drop_max()), self.c(st, |c| c.subscript_baseline_drop_min()))
        };
        let mut up = 0.0;
        let mut down = 0.0;
        if let Some(s) = &sup_box {
            let shift = if st.cramped { self.c(st, |c| c.superscript_shift_up_cramped()) } else { self.c(st, |c| c.superscript_shift_up()) };
            up = shift.max(base_asc - drop_max).max(s.desc + self.c(st, |c| c.superscript_bottom_min()));
        }
        if let Some(s) = &sub_box {
            down = self.c(st, |c| c.subscript_shift_down()).max(base_desc + drop_min).max(s.asc - self.c(st, |c| c.subscript_top_max()));
        }
        if let (Some(p), Some(q)) = (&sup_box, &sub_box) {
            let gap = (up - p.desc) - (q.asc - down);
            let min_gap = self.c(st, |c| c.sub_superscript_gap_min());
            if gap < min_gap {
                down += min_gap - gap;
                let max_bottom = self.c(st, |c| c.superscript_bottom_max_with_subscript());
                let sup_bottom = up - p.desc;
                if sup_bottom < max_bottom {
                    let lift = (max_bottom - sup_bottom).min(down - (q.asc - self.c(st, |c| c.subscript_top_max())).max(0.0));
                    if lift > 0.0 {
                        up += lift;
                        down -= lift;
                    }
                }
            }
        }
        let space = self.c(st, |c| c.space_after_script());
        let mut w = base_w;
        if let Some(s) = sup_box {
            let x = base_w + if is_integral { 0.0 } else { italic };
            w = w.max(x + s.w);
            b.add(s, x, -up);
        }
        if let Some(s) = sub_box {
            let x = base_w - if is_integral { italic } else { 0.0 };
            w = w.max(x + s.w);
            b.add(s, x, down);
        }
        b.w = w + space;
        b
    }

    fn limits(&mut self, base: B, sup: Option<&M>, sub: Option<&M>, st: Style) -> B {
        let sup_box = sup.map(|m| self.node(m, st.sup()));
        let sub_box = sub.map(|m| self.node(m, st.sub()));
        let italic = base.italic;
        let w = base.w.max(sup_box.as_ref().map_or(0.0, |s| s.w)).max(sub_box.as_ref().map_or(0.0, |s| s.w));
        let (base_asc, base_desc, base_w) = (base.asc, base.desc, base.w);
        let mut b = B::default();
        b.add(base, (w - base_w) / 2.0, 0.0);
        if let Some(s) = sup_box {
            let gap = self.c(st, |c| c.upper_limit_gap_min()).max(self.c(st, |c| c.upper_limit_baseline_rise_min()) - s.desc);
            let sw = s.w;
            let sd = s.desc;
            b.add(s, (w - sw) / 2.0 + italic / 2.0, -(base_asc + gap + sd));
        }
        if let Some(s) = sub_box {
            let gap = self.c(st, |c| c.lower_limit_gap_min()).max(self.c(st, |c| c.lower_limit_baseline_drop_min()) - s.asc);
            let sw = s.w;
            let sa = s.asc;
            b.add(s, (w - sw) / 2.0 - italic / 2.0, base_desc + gap + sa);
        }
        b.w = w;
        b
    }

    fn delimited(&mut self, left: Option<char>, right: Option<char>, body: B, st: Style) -> B {
        let a = self.axis(st);
        let half = (body.asc - a).max(body.desc + a);
        let h = 2.0 * half;
        let size = self.size(st);
        let target = (h * 0.901).max(h - 0.1 * size);
        let mut b = B::default();
        let mut x = 0.0;
        if let Some(l) = left {
            let d = self.stretched(l, target, true, size);
            let d = self.center_on_axis(d, st);
            let w = d.w;
            b.add(d, x, 0.0);
            x += w;
        }
        let bw = body.w;
        b.add(body, x, 0.0);
        x += bw;
        if let Some(r) = right {
            let d = self.stretched(r, target, true, size);
            let d = self.center_on_axis(d, st);
            let w = d.w;
            b.add(d, x, 0.0);
            x += w;
        }
        b.w = x;
        b
    }

    fn accent(&mut self, accent: char, body: &M, stretchy: bool, st: Style) -> B {
        let body = self.node(body, st.cramp());
        let size = self.size(st);
        let acc = if stretchy || body.glyph.is_none() {
            self.stretched(accent, body.w, false, size)
        } else {
            self.char_box(accent, st)
        };
        let k = size / self.f.upem;
        let attach_of = |id: u16| self.f.face.top_accent(id).map(|v| v as f32 * k);
        let base_attach = body.glyph.and_then(attach_of).unwrap_or((body.w + body.italic) / 2.0);
        // Centre the accent's ink over the attachment point.
        let acc_center = match acc.glyph {
            Some(id) => attach_of(id).unwrap_or_else(|| {
                self.f.face.bbox(id).map_or(acc.w / 2.0, |r| (r[0] as f32 + r[2] as f32) / 2.0 * k)
            }),
            None => acc.w / 2.0,
        };
        let accent_base = self.c(st, |c| c.accent_base_height());
        let lift = (body.asc - accent_base).max(0.0);
        let mut b = B::default();
        let bw = body.w;
        b.add(body, 0.0, 0.0);
        b.add(acc, base_attach - acc_center, -lift);
        b.w = bw;
        b
    }

    fn matrix(&mut self, rows: &[Vec<M>], align_left: bool, cases: bool, st: Style) -> B {
        // `aligned` & co. keep display style; matrices use text style.
        let cell_st = if st.level == 0 && !cases && !align_left { Style { level: 1, ..st } } else { st };
        let lead_space = |m: &M| -> Option<Class> {
            let first = match m {
                M::Row(items) => items.iter().find(|i| class_of(i).is_some())?,
                other => other,
            };
            class_of(first)
        };
        let cells: Vec<Vec<B>> = rows
            .iter()
            .map(|r| {
                r.iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let mut b = self.node(c, cell_st);
                        // Like TeX's `{}` before `&=`: a cell on the right of an
                        // alignment point that starts with a relation keeps its spacing.
                        if align_left && !cases && i % 2 == 1 {
                            let em = self.size(cell_st);
                            let pad = match lead_space(c) {
                                Some(Class::Rel) => em * 5.0 / 18.0,
                                Some(Class::Bin) => em * 4.0 / 18.0,
                                _ => 0.0,
                            };
                            if pad > 0.0 {
                                b.shift(pad, 0.0);
                                b.w += pad;
                            }
                        }
                        b
                    })
                    .collect()
            })
            .collect();
        let ncols = cells.iter().map(Vec::len).max().unwrap_or(0);
        let mut col_w = vec![0f32; ncols];
        for r in &cells {
            for (i, c) in r.iter().enumerate() {
                col_w[i] = col_w[i].max(c.w);
            }
        }
        let size = self.size(st);
        let aligned = align_left && !cases;
        // Gap after column i: none inside an `aligned` pair, 2em between pairs.
        let gap_after = |i: usize| if aligned { if i % 2 == 1 { size * 2.0 } else { 0.0 } } else { size };
        let col_gap_total: f32 = (0..ncols.saturating_sub(1)).map(gap_after).sum();
        let strut_asc = size * 0.7;
        let strut_desc = size * 0.3;
        // Baselines 1.2em apart, plus \jot (3pt ≈ 0.3em) for aligned equations.
        let row_gap = if aligned { size * 0.5 } else { size * 0.2 };
        let mut b = B::default();
        let mut y = 0.0;
        let mut first = true;
        for r in cells {
            let asc = r.iter().map(|c| c.asc).fold(strut_asc, f32::max);
            let desc = r.iter().map(|c| c.desc).fold(strut_desc, f32::max);
            if !first {
                y += row_gap;
            }
            first = false;
            y += asc;
            let mut x = if cases { size * 0.2 } else { 0.0 };
            for (i, c) in r.into_iter().enumerate() {
                let cw = c.w;
                let dx = if cases || (align_left && i % 2 == 1) {
                    0.0
                } else if align_left {
                    col_w[i] - cw // right-align left side of `aligned`
                } else {
                    (col_w[i] - cw) / 2.0
                };
                b.add(c, x + dx, y);
                x += col_w[i] + gap_after(i);
            }
            y += desc;
        }
        // Centre on the axis: the block's middle should sit at axis height.
        let total = y;
        let a = self.axis(st);
        let shift = total / 2.0 + a; // distance from block top to baseline
        b.shift(0.0, -shift);
        b.asc = shift;
        b.desc = total - shift;
        b.w = col_w.iter().sum::<f32>() + col_gap_total + if cases { size * 0.2 } else { 0.0 };
        b
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn font() -> Option<MathFont> {
        let data = std::fs::read(r"C:\Windows\Fonts\cambria.ttc").ok()?;
        // Cambria Math is the second face in the collection (`new` takes its offset).
        let face = *super::super::otf::faces(&data).get(1)?;
        MathFont::new(data, face)
    }

    #[test]
    fn lays_out_common_formulas() {
        let Some(f) = font() else { return };
        for src in [r"e^{i\pi} + 1 = 0", r"\frac{a}{b}", r"\sqrt[3]{x^2}", r"\sum_{k=1}^n k", r"\left(\frac{1}{2}\right)"] {
            let b = f.layout(src, 16.0, true);
            assert!(b.w > 0.0 && b.asc > 0.0, "{src}: {b:?}");
            assert!(!b.items.is_empty());
        }
    }

    /// Every corpus formula, display and inline, must lay out to exactly the
    /// recorded boxes (`UPDATE_GOLDEN=1` rewrites the file).
    #[test]
    fn golden_boxes() {
        let Some(f) = font() else { return };
        let mut out = String::new();
        for src in include_str!("../../assets/math-tests/corpus.txt").lines() {
            for display in [true, false] {
                // FNV-1a of the full box dump keeps the file small; a mismatch names the formula.
                let dump = format!("{:?}", f.layout(src, 16.0, display));
                let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
                out.push_str(&format!("{display} {hash:016x} {src}\n"));
            }
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/math-tests/layout-golden.txt");
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(path, &out).unwrap();
        }
        let want = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        for (i, (a, b)) in want.lines().zip(out.lines()).enumerate() {
            assert_eq!(a, b, "layout golden line {}", i + 1);
        }
        assert_eq!(want.lines().count(), out.lines().count());
    }

    #[test]
    fn fraction_is_taller_than_text() {
        let Some(f) = font() else { return };
        let x = f.layout("x", 16.0, false);
        let frac = f.layout(r"\frac{x}{y}", 16.0, true);
        assert!(frac.asc + frac.desc > (x.asc + x.desc) * 1.5);
    }

    #[test]
    fn display_sum_is_bigger_than_inline() {
        let Some(f) = font() else { return };
        let d = f.layout(r"\sum", 16.0, true);
        let t = f.layout(r"\sum", 16.0, false);
        assert!(d.asc + d.desc > t.asc + t.desc);
    }
}



#[cfg(all(test, windows))]
mod math_bench {
    /// Time to lay out every formula of a document (BENCH_DOC) once, as a full layout does:
    /// `BENCH_DOC=... cargo test --release math_layout_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn math_layout_cost() {
        let data = std::fs::read(r"C:\Windows\Fonts\cambria.ttc").unwrap();
        let face = *crate::math::otf::faces(&data).get(1).unwrap();
        let font = super::MathFont::new(data, face).unwrap();
        let src = std::fs::read_to_string(std::env::var("BENCH_DOC").unwrap()).unwrap();
        let doc = crate::ast::parse(&src);
        let mut formulas = Vec::new();
        fn walk(n: &crate::ast::Node, out: &mut Vec<(String, bool)>) {
            match n.kind {
                crate::ast::Kind::InlineMath => out.push((n.text.clone(), false)),
                crate::ast::Kind::DisplayMath => out.push((n.text.clone(), true)),
                _ => n.children.iter().for_each(|c| walk(c, out)),
            }
        }
        walk(&doc, &mut formulas);
        let mut distinct: Vec<_> = formulas.clone();
        distinct.sort();
        distinct.dedup();
        let mut times = Vec::new();
        for _ in 0..5 {
            let t = std::time::Instant::now();
            for (f, display) in &formulas {
                std::hint::black_box(font.layout(f, 16.0, *display));
            }
            times.push(t.elapsed().as_micros() / 1000);
        }
        eprintln!("MATH {} formulas ({} distinct): first layout {} ms, then {:?} ms", formulas.len(), distinct.len(), times[0], &times[1..]);
    }
}

#[cfg(all(test, windows))]
mod cache_tests {
    /// Cached formulas are exactly what a fresh layout gives, per size and style.
    #[test]
    fn cache_returns_the_same_boxes() {
        let data = std::fs::read(r"C:\Windows\Fonts\cambria.ttc").unwrap();
        let face = *crate::math::otf::faces(&data).get(1).unwrap();
        let font = super::MathFont::new(data, face).unwrap();
        for src in [r"\frac{a}{b}", r"\sum_{i=0}^n x_i^2", r"\sqrt{x+1}", "x"] {
            for (size, display) in [(16.0, false), (16.0, true), (24.0, false)] {
                let fresh = font.layout_uncached(src, size, display);
                for _ in 0..2 {
                    let b = font.layout(src, size, display);
                    assert_eq!((b.w, b.asc, b.desc, &b.items), (fresh.w, fresh.asc, fresh.desc, &fresh.items), "{src} {size} {display}");
                }
            }
        }
        assert_eq!(font.cache.borrow().len(), 12);
    }
}
