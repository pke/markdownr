//! Platform-neutral painting. A frame is drawn the same way on every platform: the
//! document (scrolled), particles, the chrome (faded), the overlay on top. The platform
//! only supplies the primitives behind [`Canvas`] (Direct2D on Windows, cairo on Linux,
//! Core Graphics on macOS), so what is drawn where lives here once.

use crate::layout::{DocLayout, DocSelection, Paragraph, RectF};
use crate::math::Item;
use crate::particles::{Particle, Particles, Sprite};
use crate::theme::{Ink, Theme};
use std::ops::Range;

/// A 2-D affine transform (a, b, c, d, e, f): x' = a·x + c·y + e, y' = b·x + d·y + f.
pub type Affine = [f32; 6];

pub const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

pub fn translate(x: f32, y: f32) -> Affine {
    [1.0, 0.0, 0.0, 1.0, x, y]
}

/// `first` then `second`.
pub fn then(first: Affine, second: Affine) -> Affine {
    let [a, b, c, d, e, f] = first;
    let [a2, b2, c2, d2, e2, f2] = second;
    [a * a2 + b * c2, a * b2 + b * d2, c * a2 + d * c2, c * b2 + d * d2, e * a2 + f * c2 + e2, e * b2 + f * d2 + f2]
}

/// Rotation by `deg` degrees (clockwise on screen) around (cx, cy).
pub fn rotate_around(deg: f32, cx: f32, cy: f32) -> Affine {
    let (s, c) = deg.to_radians().sin_cos();
    [c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]
}

/// The drawing primitives a platform provides. Coordinates are DIPs; the current
/// transform (set with [`Canvas::transform`]) applies to everything but `sprite`,
/// which is placed in window coordinates.
pub trait Canvas {
    type Para: Paragraph;
    fn clear(&mut self, ink: Ink);
    /// Replace the current transform.
    fn transform(&mut self, m: Affine);
    /// A rectangle, with rounded corners when `radius` > 0.
    fn fill(&mut self, r: RectF, ink: Ink, radius: f32);
    /// A closed figure: a start point, then cubic Bézier segments (three points each)
    /// while three points remain, then straight lines to the rest.
    fn fill_path(&mut self, pts: &[(f32, f32)], ink: Ink);
    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, ink: Ink);
    /// Draw everything until `pop_opacity` with opacity `a`.
    fn push_opacity(&mut self, a: f32);
    fn pop_opacity(&mut self);
    /// A laid-out paragraph with its top-left at (x, y); `ink` colours text without a colour span.
    fn text(&mut self, para: &Self::Para, x: f32, y: f32, ink: Ink);
    /// A loaded image scaled into `r` (nothing while it is still loading).
    fn image(&mut self, url: &str, r: RectF);
    /// One glyph of the math font, its origin (baseline) at (x, y).
    fn glyph(&mut self, id: u16, x: f32, y: f32, size: f32, ink: Ink);
    /// A particle glyph in window coordinates: its layout box at (x, y), rotated by
    /// `rotation` degrees around its centre, faded by `alpha`.
    fn sprite(&mut self, text: &'static str, size: f32, ink: Ink, alpha: f32, x: f32, y: f32, rotation: f32);
}

/// Everything a frame shows.
pub struct Frame<'a, P> {
    pub doc: &'a DocLayout<P>,
    pub selection: Option<&'a DocSelection>,
    /// (block, byte range, current match)
    pub finds: &'a [(usize, Range<usize>, bool)],
    pub theme: &'a Theme,
    pub scroll: f32,
    pub view_h: f32,
    /// Toolbar and drawer with their fade.
    pub chrome: Option<(&'a DocLayout<P>, f32)>,
    /// Particles drawn into the frame (not where the compositor animates them).
    pub particles: Option<(&'a Particles, &'a [Sprite])>,
    /// Tooltip or help overlay, over everything.
    pub tip: &'a DocLayout<P>,
    /// Animation clock for time-based effects (quote crossfades), ms.
    pub now_ms: f32,
}

pub fn frame<C: Canvas>(c: &mut C, f: &Frame<C::Para>) {
    let doc_m = translate(0.0, -f.scroll.round());
    c.transform(doc_m);
    c.clear(f.theme.background);
    layout(c, f.doc, f.selection, f.finds, f.theme, f.scroll, f.view_h, f.now_ms, doc_m);
    c.transform(IDENTITY);
    if let Some((ps, sprites)) = f.particles {
        let tint = particle_tint(ps, f.theme);
        for s in sprites {
            let p = &ps.items[s.index];
            let (ink, fade) = particle_ink(tint, p);
            c.sprite(p.text, p.size, ink, fade, s.x, s.y, s.rotation);
        }
    }
    if let Some((chrome, alpha)) = f.chrome {
        c.push_opacity(alpha);
        layout(c, chrome, None, &[], f.theme, 0.0, f.view_h, f.now_ms, IDENTITY);
        c.pop_opacity();
    }
    if !f.tip.rects.is_empty() {
        layout(c, f.tip, None, &[], f.theme, 0.0, f.view_h, f.now_ms, IDENTITY);
    }
}

/// The particles' tint: the theme's (light or dark) for monochrome glyphs, else the text colour.
pub fn particle_tint(ps: &Particles, theme: &Theme) -> Ink {
    let [r, g, b, _] = theme.background.0;
    let dark = (r as u32 + g as u32 + b as u32) < 384;
    ps.color.map_or(theme.text, |(l, d)| Ink::rgb(if dark { d } else { l }))
}

/// A particle's ink and fade: the glyph is drawn opaque and the whole sprite fades by the
/// particle's opacity, once, for monochrome glyphs (✦ • ❅) and colour emoji (❄ 🍂) alike.
/// (Before, monochrome glyphs also took it from the brush alpha, so they got it twice;
/// colour emoji ignore brush alpha.)
pub fn particle_ink(tint: Ink, p: &Particle) -> (Ink, f32) {
    let mut ink = tint;
    ink.0[3] = 255;
    (ink, if p.opacity < 0.99 { p.opacity } else { 1.0 })
}

/// Santa hat (port of the iOS SantaHat SVG, 64×64 viewBox) with its top-left at (x, y)
/// in the coordinates of `base`.
fn hat<C: Canvas>(c: &mut C, x: f32, y: f32, size: f32, base: Affine) {
    let k = size / 64.0;
    let m = then(then([k, 0.0, 0.0, k, 0.0, 0.0], rotate_around(-18.0, 32.0 * k, 56.0 * k)), then(translate(x, y), base));
    c.transform(m);
    c.fill_path(&[(0.0, 56.0), (4.0, 30.0), (16.0, 14.0), (32.0, 4.0), (48.0, 14.0), (60.0, 30.0), (64.0, 56.0)], Ink::rgb(0xcc0000));
    c.fill_path(&[(32.0, 4.0), (48.0, 14.0), (60.0, 30.0), (64.0, 56.0), (32.0, 56.0)], Ink::rgba(0xaa0000, 102));
    c.fill_ellipse(32.0, 56.0, 32.0, 8.0, Ink::rgb(0xffffff));
    c.fill_ellipse(32.0, 4.0, 4.0, 4.0, Ink::rgb(0xffffff));
    c.transform(base);
}

#[allow(clippy::too_many_arguments)]
fn layout<C: Canvas>(c: &mut C, doc: &DocLayout<C::Para>, selection: Option<&DocSelection>, finds: &[(usize, Range<usize>, bool)], theme: &Theme, scroll: f32, view_h: f32, now_ms: f32, base: Affine) {
    let (top, bottom) = (scroll - 50.0, scroll + view_h + 50.0);
    let visible = |y: f32, h: f32| y + h >= top && y <= bottom;
    // `:::quotes` crossfade: opacity per quote's block and rect ranges (elsewhere 1;
    // the last range containing an index wins).
    let fades: Vec<(&Range<usize>, &Range<usize>, f32)> = doc
        .cycles
        .iter()
        .flat_map(|group| {
            let n = group.len().max(1);
            let period = 10_000.0;
            let k = (now_ms / period) as usize;
            let (cur, prev) = (k % n, (k + n - 1) % n);
            let f = if k == 0 { 1.0 } else { ((now_ms % period) / 600.0).min(1.0) };
            group.iter().enumerate().map(move |(qi, (br, rr))| (br, rr, if qi == cur { f } else if qi == prev && f < 1.0 { 1.0 - f } else { 0.0 }))
        })
        .collect();
    let block_alpha = |i: usize| fades.iter().rev().find(|(br, _, _)| br.contains(&i)).map_or(1.0, |f| f.2);
    let rect_alpha = |i: usize| fades.iter().rev().find(|(_, rr, _)| rr.contains(&i)).map_or(1.0, |f| f.2);

    for (ri, r) in doc.rects.iter().enumerate() {
        let a = rect_alpha(ri);
        if !visible(r.r.y, r.r.h) || a <= 0.0 {
            continue;
        }
        if a < 1.0 {
            c.push_opacity(a);
        }
        c.fill(r.r, r.color, r.radius);
        if a < 1.0 {
            c.pop_opacity();
        }
    }
    for img in &doc.images {
        if visible(img.r.y, img.r.h) {
            c.image(&img.url, img.r);
        }
    }
    for (i, block) in doc.blocks.iter().enumerate() {
        let a = block_alpha(i);
        if !visible(block.y, block.h) || a <= 0.0 {
            continue;
        }
        if a < 1.0 {
            c.push_opacity(a);
        }
        for (_, range, current) in finds.iter().filter(|f| f.0 == i) {
            let ink = if *current { theme.find_current } else { theme.find };
            for r in block.para.range_rects(range.clone()) {
                c.fill(RectF { x: block.x + r.x - 1.0, y: block.y + r.y, w: r.w + 2.0, h: r.h }, ink, 3.0);
            }
        }
        if let Some(range) = selection.and_then(|s| doc.block_range(s, i)) {
            for r in block.para.range_rects(range) {
                c.fill(RectF { x: block.x + r.x, y: block.y + r.y, ..r }, theme.selection, 0.0);
            }
        }
        c.text(&block.para, block.x, block.y, theme.text);
        if a < 1.0 {
            c.pop_opacity();
        }
    }
    for &(hx, hy, hs) in &doc.hats {
        if visible(hy, hs) {
            hat(c, hx, hy, hs, base);
        }
    }
    for p in &doc.math {
        if !visible(p.y - p.mbox.asc, p.mbox.asc + p.mbox.desc) {
            continue;
        }
        for item in &p.mbox.items {
            match *item {
                Item::Glyph { id, x, y, size, color } => c.glyph(id, p.x + x, p.y + y, size, color.map_or(p.color, |[r, g, b]| Ink([r, g, b, 255]))),
                Item::Rule { r, color } => c.fill(RectF { x: p.x + r.x, y: p.y + r.y, ..r }, color.map_or(p.color, |[r, g, b]| Ink([r, g, b, 255])), 0.0),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine() {
        let m = then(translate(1.0, 2.0), translate(3.0, 4.0));
        assert_eq!(m, translate(4.0, 6.0));
        let r = rotate_around(90.0, 1.0, 1.0);
        // (2, 1) rotated 90° clockwise (y down) around (1, 1) is (1, 2).
        let (x, y) = (r[0] * 2.0 + r[2] * 1.0 + r[4], r[1] * 2.0 + r[3] * 1.0 + r[5]);
        assert!((x - 1.0).abs() < 1e-5 && (y - 2.0).abs() < 1e-5, "{x} {y}");
    }
}
