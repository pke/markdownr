//! Linux text and drawing: Pango lays out and hit-tests paragraphs (UTF-8 byte offsets, as
//! the layout uses them), cairo draws (`paint::Canvas`), gdk-pixbuf decodes images on
//! worker threads, fontconfig finds an OpenType MATH font for formulas.

use super::ffi::*;
use crate::layout::{BaseStyle, Paragraph, RectF, Span, TextAlign, Typesetter};
use crate::math::MathFont;
use crate::paint::{Affine, Canvas, Frame};
use crate::theme::Ink;
use crate::util::VecMap;
use core::ffi::{c_char, c_int};
use std::ffi::CString;
use std::ops::Range;
use std::path::PathBuf;

const BODY: &[u8] = b"Sans\0";
const MONO: &[u8] = b"Monospace\0";

fn pu(v: c_int) -> f32 {
    v as f32 / PANGO_SCALE
}

fn to_pu(v: f32) -> c_int {
    (v * PANGO_SCALE).round() as c_int
}

/// A Pango layout; byte offsets are the layout's own (UTF-8) indices.
pub struct PangoPara {
    layout: *mut PangoLayout,
    text: Box<str>,
    /// Inline objects: (byte, ascent).
    objects: Vec<(usize, f32)>,
}

impl Drop for PangoPara {
    fn drop(&mut self) {
        unsafe { g_object_unref(self.layout) };
    }
}

impl PangoPara {
    fn logical(&self) -> PangoRectangle {
        let mut r = PangoRectangle::default();
        unsafe { pango_layout_get_extents(self.layout, core::ptr::null_mut(), &mut r) };
        r
    }

    /// Calls `f(line, logical extents, baseline)` for each line until it returns true.
    fn lines(&self, mut f: impl FnMut(&mut PangoLayoutLine, PangoRectangle, c_int) -> bool) {
        unsafe {
            let it = pango_layout_get_iter(self.layout);
            loop {
                let line = pango_layout_iter_get_line_readonly(it);
                let mut r = PangoRectangle::default();
                pango_layout_iter_get_line_extents(it, core::ptr::null_mut(), &mut r);
                if !line.is_null() && f(&mut *line, r, pango_layout_iter_get_baseline(it)) {
                    break;
                }
                if pango_layout_iter_next_line(it) == 0 {
                    break;
                }
            }
            pango_layout_iter_free(it);
        }
    }

    /// The width with `wrap` at `width` (Pango units, -1: none) on a copy.
    fn width_at(&self, wrap: c_int, width: c_int) -> f32 {
        unsafe {
            let l = pango_layout_copy(self.layout);
            pango_layout_set_wrap(l, wrap);
            pango_layout_set_width(l, width);
            let mut r = PangoRectangle::default();
            pango_layout_get_extents(l, core::ptr::null_mut(), &mut r);
            g_object_unref(l);
            pu(r.width)
        }
    }
}

impl Paragraph for PangoPara {
    fn size(&self) -> (f32, f32) {
        let r = self.logical();
        (pu(r.width), pu(r.height))
    }
    fn content_widths(&self) -> (f32, f32) {
        // Narrowest: every word on a line of its own; widest: one line.
        (self.width_at(PANGO_WRAP_WORD, 1), self.width_at(PANGO_WRAP_WORD_CHAR, -1))
    }
    fn set_width(&mut self, width: f32) {
        unsafe { pango_layout_set_width(self.layout, to_pu(width.max(1.0))) };
    }
    fn set_align(&mut self, align: TextAlign) {
        let a = match align {
            TextAlign::Center => 1,
            TextAlign::End => 2,
        };
        unsafe { pango_layout_set_alignment(self.layout, a) };
    }
    fn hit(&self, x: f32, y: f32) -> usize {
        let (mut index, mut trailing) = (0, 0);
        unsafe { pango_layout_xy_to_index(self.layout, to_pu(x), to_pu(y), &mut index, &mut trailing) };
        let mut at = (index.max(0) as usize).min(self.text.len());
        for _ in 0..trailing {
            at += self.text[at..].chars().next().map_or(0, char::len_utf8);
        }
        at
    }
    fn range_rects(&self, range: Range<usize>) -> Vec<RectF> {
        let mut out = Vec::new();
        if range.is_empty() {
            return out;
        }
        self.lines(|line, r, _| {
            let (ls, le) = (line.start_index as usize, (line.start_index + line.length) as usize);
            if le < range.start {
                return false;
            }
            if ls > range.end {
                return true;
            }
            let (s, e) = (range.start.max(ls), range.end.min(le));
            let (mut ranges, mut n) = (core::ptr::null_mut(), 0);
            unsafe {
                pango_layout_line_get_x_ranges(line, s as c_int, e as c_int, &mut ranges, &mut n);
                for i in 0..n as usize {
                    let (x0, x1) = (*ranges.add(2 * i), *ranges.add(2 * i + 1));
                    if x1 > x0 {
                        out.push(RectF { x: pu(x0), y: pu(r.y), w: pu(x1 - x0), h: pu(r.height) });
                    }
                }
                g_free(ranges as *mut _);
            }
            false
        });
        out
    }
    fn object_origin(&self, byte: usize) -> (f32, f32) {
        let asc = self.objects.iter().find(|o| o.0 == byte).map_or(0.0, |o| o.1);
        let mut pos = PangoRectangle::default();
        unsafe { pango_layout_index_to_pos(self.layout, byte as c_int, &mut pos) };
        let mut baseline = None;
        self.lines(|line, _, base| {
            let (ls, le) = (line.start_index as usize, (line.start_index + line.length) as usize);
            if (ls..le).contains(&byte) {
                baseline = Some(base);
                return true;
            }
            false
        });
        (pu(pos.x), baseline.map_or(pu(pos.y), |b| pu(b) - asc))
    }
}

struct MathFace {
    font: MathFont,
    face: *mut cairo_font_face_t,
}

/// An OpenType MATH font from fontconfig: the first of these families installed.
fn load_math_face() -> Option<MathFace> {
    const FAMILIES: [&str; 7] = ["STIX Two Math", "Latin Modern Math", "DejaVu Math TeX Gyre", "TeX Gyre Pagella Math", "Libertinus Math", "Noto Sans Math", "Cambria Math"];
    for family in FAMILIES {
        unsafe {
            let name = CString::new(family).ok()?;
            let pat = FcNameParse(name.as_ptr() as *const u8);
            if pat.is_null() {
                continue;
            }
            FcConfigSubstitute(core::ptr::null_mut(), pat, 0);
            FcDefaultSubstitute(pat);
            let mut result = 0;
            let m = FcFontMatch(core::ptr::null_mut(), pat, &mut result);
            FcPatternDestroy(pat);
            if m.is_null() {
                continue;
            }
            let (mut file, mut fam) = (core::ptr::null(), core::ptr::null());
            let mut index = 0;
            let found = FcPatternGetString(m, c"file".as_ptr(), 0, &mut file) == 0
                && FcPatternGetString(m, c"family".as_ptr(), 0, &mut fam) == 0
                && std::ffi::CStr::from_ptr(fam as *const c_char).to_bytes() == family.as_bytes();
            FcPatternGetInteger(m, c"index".as_ptr(), 0, &mut index);
            if found {
                let path = PathBuf::from(<std::ffi::OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(std::ffi::CStr::from_ptr(file as *const c_char).to_bytes()));
                if let Some(data) = crate::util::fs::read(&path) {
                    let offs = crate::math::otf::faces(&data);
                    if let Some(&off) = offs.get(index.max(0) as usize).filter(|&&o| crate::math::otf::has_math(&data, o)) {
                        if let Some(font) = MathFont::new(data, off) {
                            let face = cairo_ft_font_face_create_for_pattern(m);
                            FcPatternDestroy(m);
                            return Some(MathFace { font, face });
                        }
                    }
                }
            }
            FcPatternDestroy(m);
        }
    }
    None
}

/// Pango context and font cache, the frame's target, images and the math font.
pub struct Gfx {
    ctx: *mut PangoContext,
    fonts: VecMap<(u32, bool, bool, bool), *mut PangoFontDescription>,
    /// Particle glyph layouts by (text, size).
    sprites: VecMap<(&'static str, u32), *mut PangoLayout>,
    math: Option<MathFace>,
    pub images: super::images::Images,
    /// The surface frames are drawn on (the window's back buffer).
    pub target: *mut cairo_surface_t,
}

impl Gfx {
    pub fn new(images: super::images::Images) -> Gfx {
        unsafe {
            let ctx = pango_font_map_create_context(pango_cairo_font_map_get_default());
            // Layout in unhinted metrics (positions independent of the display scale, as
            // DirectWrite's natural mode), glyphs slightly hinted and greyscale-smoothed.
            let o = cairo_font_options_create();
            cairo_font_options_set_hint_metrics(o, 2);
            cairo_font_options_set_hint_style(o, 2);
            cairo_font_options_set_antialias(o, 2);
            pango_cairo_context_set_font_options(ctx, o);
            Gfx { ctx, fonts: VecMap::new(), sprites: VecMap::new(), math: load_math_face(), images, target: core::ptr::null_mut() }
        }
    }

    fn font(&mut self, size: f32, mono: bool, bold: bool, italic: bool) -> *mut PangoFontDescription {
        *self.fonts.or_insert_with((size.to_bits(), mono, bold, italic), || unsafe {
            let d = pango_font_description_new();
            pango_font_description_set_family(d, if mono { MONO } else { BODY }.as_ptr() as *const c_char);
            pango_font_description_set_absolute_size(d, (size * PANGO_SCALE) as f64);
            pango_font_description_set_weight(d, if bold { 600 } else { 400 });
            if italic {
                pango_font_description_set_style(d, PANGO_STYLE_ITALIC);
            }
            d
        })
    }

    fn sprite_layout(&mut self, text: &'static str, size: f32) -> *mut PangoLayout {
        let font = self.font(size, false, false, false);
        let ctx = self.ctx;
        *self.sprites.or_insert_with((text, size.to_bits()), || unsafe {
            let l = pango_layout_new(ctx);
            pango_layout_set_font_description(l, font);
            pango_layout_set_text(l, text.as_ptr() as *const c_char, text.len() as c_int);
            l
        })
    }
}

fn attr(list: *mut PangoAttrList, a: *mut PangoAttribute, r: &Range<usize>) {
    if a.is_null() {
        return;
    }
    unsafe {
        (*a).start_index = r.start as u32;
        (*a).end_index = r.end as u32;
        pango_attr_list_insert(list, a);
    }
}

fn foreground(list: *mut PangoAttrList, ink: Ink, r: &Range<usize>) {
    let [cr, cg, cb, ca] = ink.0;
    let c = |v: u8| v as u16 * 257;
    unsafe {
        attr(list, pango_attr_foreground_new(c(cr), c(cg), c(cb)), r);
        if ca < 255 {
            attr(list, pango_attr_foreground_alpha_new(c(ca)), r);
        }
    }
}

impl Typesetter for Gfx {
    type Para = PangoPara;

    fn image_size(&self, url: &str) -> Option<(f32, f32)> {
        self.images.size(url)
    }

    fn math_font(&self) -> Option<&MathFont> {
        self.math.as_ref().map(|m| &m.font)
    }

    fn paragraph(&mut self, text: &str, spans: &[(Range<usize>, Span)], base: &BaseStyle, max_width: f32) -> PangoPara {
        let font = self.font(base.size, base.mono, base.bold, base.italic);
        let mut objects = Vec::new();
        unsafe {
            let layout = pango_layout_new(self.ctx);
            pango_layout_set_text(layout, text.as_ptr() as *const c_char, text.len() as c_int);
            pango_layout_set_font_description(layout, font);
            pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
            pango_layout_set_width(layout, to_pu(max_width.max(1.0)));
            let list = pango_attr_list_new();
            let all = 0..text.len();
            // Uniform line spacing as on Windows; lines with formulas grow to fit them.
            if spans.iter().any(|(_, s)| matches!(s, Span::Object { .. })) {
                attr(list, pango_attr_line_height_new((base.line_height / 1.33) as f64), &all);
            } else {
                attr(list, pango_attr_line_height_new_absolute(to_pu((base.size * base.line_height).round())), &all);
            }
            foreground(list, base.color, &all);
            for (r, span) in spans {
                if r.is_empty() {
                    continue;
                }
                match *span {
                    Span::Bold => attr(list, pango_attr_weight_new(600), r),
                    Span::Italic => attr(list, pango_attr_style_new(PANGO_STYLE_ITALIC), r),
                    Span::Mono => attr(list, pango_attr_family_new(MONO.as_ptr() as *const c_char), r),
                    Span::Color(c) => foreground(list, c, r),
                    Span::Strike => attr(list, pango_attr_strikethrough_new(1), r),
                    Span::Underline => attr(list, pango_attr_underline_new(PANGO_UNDERLINE_SINGLE), r),
                    Span::Scale(s) => attr(list, pango_attr_size_new_absolute(to_pu(base.size * s)), r),
                    Span::Super => attr(list, pango_attr_font_features_new(c"sups".as_ptr()), r),
                    Span::Sub => attr(list, pango_attr_font_features_new(c"subs".as_ptr()), r),
                    Span::Object { w, asc, desc } => {
                        let rect = PangoRectangle { x: 0, y: -to_pu(asc), width: to_pu(w), height: to_pu(asc + desc) };
                        attr(list, pango_attr_shape_new(&rect, &rect), r);
                        objects.push((r.start, asc));
                    }
                }
            }
            pango_layout_set_attributes(layout, list);
            pango_attr_list_unref(list);
            PangoPara { layout, text: text.into(), objects }
        }
    }
}

impl crate::app::Gfx for Gfx {
    fn set_image_base(&mut self, dir: Option<PathBuf>) {
        self.images.set_base(dir);
    }

    fn draw(&mut self, frame: &Frame<PangoPara>) -> bool {
        if self.target.is_null() {
            return true;
        }
        unsafe {
            let cr = cairo_create(self.target);
            crate::paint::frame(&mut CairoCanvas { gfx: self, cr, groups: Vec::new() }, frame);
            cairo_destroy(cr);
            cairo_surface_flush(self.target);
        }
        true
    }
}

/// `paint::Canvas` on a cairo context during a frame.
struct CairoCanvas<'a> {
    gfx: &'a mut Gfx,
    cr: *mut cairo_t,
    /// Opacities of the open groups.
    groups: Vec<f32>,
}

impl CairoCanvas<'_> {
    fn source(&self, ink: Ink) {
        let [r, g, b, a] = ink.0;
        unsafe { cairo_set_source_rgba(self.cr, r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, a as f64 / 255.0) };
    }
}

impl Canvas for CairoCanvas<'_> {
    type Para = PangoPara;

    fn clear(&mut self, ink: Ink) {
        self.source(ink);
        unsafe { cairo_paint(self.cr) };
    }

    fn transform(&mut self, m: Affine) {
        let [a, b, c, d, e, f] = m.map(f64::from);
        unsafe { cairo_set_matrix(self.cr, &cairo_matrix_t { xx: a, yx: b, xy: c, yy: d, x0: e, y0: f }) };
    }

    fn fill(&mut self, r: RectF, ink: Ink, radius: f32) {
        let cr = self.cr;
        let (x, y, w, h) = (r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        unsafe {
            cairo_new_path(cr);
            if radius > 0.0 {
                let rad = (radius as f64).min(w / 2.0).min(h / 2.0);
                let pi = core::f64::consts::PI;
                cairo_new_sub_path(cr);
                cairo_arc(cr, x + w - rad, y + rad, rad, -pi / 2.0, 0.0);
                cairo_arc(cr, x + w - rad, y + h - rad, rad, 0.0, pi / 2.0);
                cairo_arc(cr, x + rad, y + h - rad, rad, pi / 2.0, pi);
                cairo_arc(cr, x + rad, y + rad, rad, pi, 1.5 * pi);
                cairo_close_path(cr);
            } else {
                cairo_rectangle(cr, x, y, w, h);
            }
            self.source(ink);
            cairo_fill(cr);
        }
    }

    fn fill_path(&mut self, pts: &[(f32, f32)], ink: Ink) {
        let cr = self.cr;
        let p = |i: usize| (pts[i].0 as f64, pts[i].1 as f64);
        unsafe {
            cairo_new_path(cr);
            cairo_move_to(cr, p(0).0, p(0).1);
            let mut i = 1;
            while i + 2 < pts.len() {
                cairo_curve_to(cr, p(i).0, p(i).1, p(i + 1).0, p(i + 1).1, p(i + 2).0, p(i + 2).1);
                i += 3;
            }
            for j in i..pts.len() {
                cairo_line_to(cr, p(j).0, p(j).1);
            }
            cairo_close_path(cr);
            self.source(ink);
            cairo_fill(cr);
        }
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, ink: Ink) {
        let cr = self.cr;
        unsafe {
            cairo_new_path(cr);
            cairo_save(cr);
            cairo_translate(cr, cx as f64, cy as f64);
            cairo_scale(cr, rx as f64, ry as f64);
            cairo_arc(cr, 0.0, 0.0, 1.0, 0.0, 2.0 * core::f64::consts::PI);
            cairo_restore(cr);
            self.source(ink);
            cairo_fill(cr);
        }
    }

    fn push_opacity(&mut self, a: f32) {
        self.groups.push(a);
        unsafe { cairo_push_group(self.cr) };
    }

    fn pop_opacity(&mut self) {
        let a = self.groups.pop().unwrap_or(1.0);
        unsafe {
            cairo_pop_group_to_source(self.cr);
            cairo_paint_with_alpha(self.cr, a as f64);
        }
    }

    fn text(&mut self, para: &PangoPara, x: f32, y: f32, ink: Ink) {
        self.source(ink);
        unsafe {
            cairo_move_to(self.cr, x as f64, y as f64);
            pango_cairo_show_layout(self.cr, para.layout);
        }
    }

    fn image(&mut self, url: &str, r: RectF) {
        let Some((s, iw, ih)) = self.gfx.images.surface(url) else { return };
        let cr = self.cr;
        unsafe {
            cairo_save(cr);
            cairo_translate(cr, r.x as f64, r.y as f64);
            cairo_scale(cr, r.w as f64 / iw as f64, r.h as f64 / ih as f64);
            cairo_set_source_surface(cr, s, 0.0, 0.0);
            cairo_pattern_set_filter(cairo_get_source(cr), CAIRO_FILTER_GOOD);
            cairo_paint(cr);
            cairo_restore(cr);
        }
    }

    fn glyph(&mut self, id: u16, x: f32, y: f32, size: f32, ink: Ink) {
        let Some(face) = self.gfx.math.as_ref().map(|m| m.face) else { return };
        self.source(ink);
        unsafe {
            cairo_set_font_face(self.cr, face);
            cairo_set_font_size(self.cr, size as f64);
            cairo_show_glyphs(self.cr, &cairo_glyph_t { index: id as _, x: x as f64, y: y as f64 }, 1);
        }
    }

    fn sprite(&mut self, text: &'static str, size: f32, ink: Ink, alpha: f32, x: f32, y: f32, rotation: f32) {
        let layout = self.gfx.sprite_layout(text, size);
        let cr = self.cr;
        let (cx, cy) = ((x + size / 2.0) as f64, (y + size / 2.0) as f64);
        unsafe {
            cairo_save(cr);
            cairo_translate(cr, cx, cy);
            cairo_rotate(cr, (rotation as f64).to_radians());
            cairo_translate(cr, -cx, -cy);
            cairo_push_group(cr);
            self.source(ink);
            cairo_move_to(cr, x as f64, y as f64);
            pango_cairo_show_layout(cr, layout);
            cairo_pop_group_to_source(cr);
            cairo_paint_with_alpha(cr, alpha as f64);
            cairo_restore(cr);
        }
    }
}
