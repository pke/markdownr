//! Paints a `DocLayout` into a vello `Scene`, culling everything outside the viewport.

use crate::layout::{DocLayout, DocSelection};
use crate::theme::{Ink, Theme};
use parley::PositionedLayoutItem;
use vello::kurbo::{Affine, Rect, RoundedRect};
use vello::peniko::Fill;
use vello::{Glyph, Scene};

pub struct Viewport {
    /// Scroll offset in logical px.
    pub scroll: f32,
    /// Viewport height in logical px.
    pub height: f32,
    /// Logical → physical px.
    pub scale: f64,
}

fn fill_rect(scene: &mut Scene, t: Affine, x: f32, y: f32, w: f32, h: f32, color: Ink, radius: f32) {
    let rect = Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64);
    if radius > 0.0 {
        scene.fill(Fill::NonZero, t, color.color(), None, &RoundedRect::from_rect(rect, radius as f64));
    } else {
        scene.fill(Fill::NonZero, t, color.color(), None, &rect);
    }
}

pub fn paint(scene: &mut Scene, doc: &DocLayout, selection: Option<&DocSelection>, theme: &Theme, vp: &Viewport) {
    let t = Affine::scale(vp.scale) * Affine::translate((0.0, -vp.scroll as f64));
    let (top, bottom) = (vp.scroll - 50.0, vp.scroll + vp.height + 50.0);
    let visible = |y: f32, h: f32| y + h >= top && y <= bottom;

    for r in &doc.rects {
        if visible(r.y, r.h) {
            fill_rect(scene, t, r.x, r.y, r.w, r.h, r.color, r.radius);
        }
    }

    for (i, block) in doc.blocks.iter().enumerate() {
        if !visible(block.y, block.height()) {
            continue;
        }
        let bt = t * Affine::translate((block.x as f64, block.y as f64));

        if let Some(range) = selection.and_then(|s| doc.block_range(s, i)) {
            let sel = parley::Selection::new(
                parley::Cursor::from_byte_index(&block.layout, range.start, parley::Affinity::Downstream),
                parley::Cursor::from_byte_index(&block.layout, range.end, parley::Affinity::Upstream),
            );
            for (bb, _) in sel.geometry(&block.layout) {
                fill_rect(scene, bt, bb.x0 as f32, bb.y0 as f32, (bb.x1 - bb.x0) as f32, (bb.y1 - bb.y0) as f32, theme.selection, 0.0);
            }
        }

        for line in block.layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else { continue };
                let style = glyph_run.style();
                let run = glyph_run.run();
                let font = run.font();
                let font_size = run.font_size();
                let synthesis = run.synthesis();
                let glyph_xform = synthesis.skew().map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));
                scene
                    .draw_glyphs(font)
                    .brush(style.brush.color())
                    .hint(false)
                    .transform(bt)
                    .glyph_transform(glyph_xform)
                    .font_size(font_size)
                    .normalized_coords(&run.normalized_coords().iter().map(|c| c.to_bits()).collect::<Vec<i16>>())
                    .draw(
                        Fill::NonZero,
                        glyph_run.positioned_glyphs().map(|g| Glyph { id: g.id as _, x: g.x, y: g.y }),
                    );

                let x = glyph_run.offset();
                let w = glyph_run.advance();
                let baseline = glyph_run.baseline();
                let thickness = (font_size / 14.0).max(1.0);
                if let Some(deco) = &style.underline {
                    let offset = deco.offset.unwrap_or(font_size * 0.12);
                    fill_rect(scene, bt, x, baseline + offset, w, deco.size.unwrap_or(thickness), deco.brush, 0.0);
                }
                if let Some(deco) = &style.strikethrough {
                    let offset = deco.offset.unwrap_or(font_size * 0.3);
                    fill_rect(scene, bt, x, baseline - offset, w, deco.size.unwrap_or(thickness), deco.brush, 0.0);
                }
            }
        }
    }
}
