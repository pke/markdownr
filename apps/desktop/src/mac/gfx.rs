//! macOS text and drawing: Core Text breaks and hit-tests paragraphs (UTF-16 indices,
//! mapped to the layout's UTF-8 byte offsets as on Windows), Core Graphics draws
//! (`paint::Canvas`) into the view's flipped context, ImageIO decodes images on worker
//! threads, STIX Two Math (shipped with macOS) sets formulas.

use super::ffi::*;
use crate::layout::{BaseStyle, Paragraph, RectF, Span, TextAlign, Typesetter};
use crate::math::MathFont;
use crate::paint::{Affine, Canvas, Frame};
use crate::theme::Ink;
use crate::util::VecMap;
use core::ffi::c_void;
use std::ops::Range;
use std::path::PathBuf;

fn cfstring(s: &str) -> CFStringRef {
    unsafe { CFStringCreateWithBytes(kCFAllocatorDefault, s.as_ptr(), s.len() as CFIndex, kCFStringEncodingUTF8, false) }
}

fn cgcolor(ink: Ink) -> CGColorRef {
    let [r, g, b, a] = ink.0;
    unsafe { CGColorCreateSRGB(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, a as f64 / 255.0) }
}

struct Line {
    line: CTLineRef,
    /// UTF-16 range of the line.
    start: usize,
    len: usize,
    /// Alignment offset, top, baseline, height and width (with trailing white space).
    x: f32,
    top: f32,
    baseline: f32,
    height: f32,
    width: f32,
}

/// A Core Text paragraph: its typesetter, broken into lines at the current width.
pub struct CtPara {
    typesetter: CTTypesetterRef,
    /// UTF-16 index → UTF-8 byte offset (len = utf16 len + 1).
    u16_to_byte: Vec<u32>,
    /// The character at each UTF-16 index (for word and line ends).
    text_chars: Vec<char>,
    lines: Vec<Line>,
    max_width: f32,
    align: Option<TextAlign>,
    /// Uniform line height and the baseline within it.
    line_height: f32,
    baseline: f32,
    /// Inline objects: (UTF-16 index, ascent).
    objects: Vec<(usize, f32)>,
    /// Struck-through UTF-16 ranges with their colour and the font size (Core Text has no strike).
    strikes: Vec<(Range<usize>, Ink, f32)>,
}

impl Drop for CtPara {
    fn drop(&mut self) {
        unsafe {
            for l in &self.lines {
                CFRelease(l.line);
            }
            CFRelease(self.typesetter);
        }
    }
}

impl CtPara {
    fn to_u16(&self, byte: usize) -> usize {
        self.u16_to_byte.partition_point(|&b| (b as usize) < byte)
    }
    fn to_byte(&self, pos: usize) -> usize {
        self.u16_to_byte[pos.min(self.u16_to_byte.len() - 1)] as usize
    }
    fn len16(&self) -> usize {
        self.u16_to_byte.len() - 1
    }

    /// Break into lines at `max_width`.
    fn break_lines(&mut self) {
        unsafe {
            for l in self.lines.drain(..) {
                CFRelease(l.line);
            }
            let (mut start, mut top, n) = (0usize, 0.0f32, self.len16());
            while start < n || (n == 0 && self.lines.is_empty()) {
                let mut count = CTTypesetterSuggestLineBreak(self.typesetter, start as CFIndex, self.max_width as f64).max(if n == 0 { 0 } else { 1 }) as usize;
                count = count.min(n - start);
                let line = CTTypesetterCreateLine(self.typesetter, CFRange { location: start as CFIndex, length: count as CFIndex });
                let (mut asc, mut desc, mut lead) = (0.0, 0.0, 0.0);
                let width = CTLineGetTypographicBounds(line, &mut asc, &mut desc, &mut lead) as f32;
                let has_object = self.objects.iter().any(|o| (start..start + count).contains(&o.0));
                let (height, baseline) = if has_object && (asc + desc) as f32 > self.line_height {
                    ((asc + desc) as f32, asc as f32)
                } else {
                    (self.line_height, self.baseline.max(asc as f32))
                };
                let visible = width - CTLineGetTrailingWhitespaceWidth(line) as f32;
                let x = match self.align {
                    Some(TextAlign::Center) => ((self.max_width - visible) / 2.0).max(0.0),
                    Some(TextAlign::End) => (self.max_width - visible).max(0.0),
                    None => 0.0,
                };
                self.lines.push(Line { line, start, len: count, x, top, baseline: top + baseline, height, width });
                top += height;
                start += count;
                if count == 0 {
                    break;
                }
            }
        }
    }

    fn line_at(&self, y: f32) -> Option<&Line> {
        self.lines.iter().find(|l| y < l.top + l.height).or(self.lines.last())
    }

    fn line_of(&self, pos: usize) -> Option<&Line> {
        self.lines.iter().find(|l| pos < l.start + l.len).or(self.lines.last())
    }

    fn offset(l: &Line, pos: usize) -> f32 {
        unsafe { CTLineGetOffsetForStringIndex(l.line, pos as CFIndex, core::ptr::null_mut()) as f32 }
    }

    /// Width of the UTF-16 range set as one line.
    fn range_width(&self, r: Range<usize>) -> f32 {
        unsafe {
            let line = CTTypesetterCreateLine(self.typesetter, CFRange { location: r.start as CFIndex, length: r.len() as CFIndex });
            let w = (CTLineGetTypographicBounds(line, core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut()) - CTLineGetTrailingWhitespaceWidth(line)) as f32;
            CFRelease(line);
            w
        }
    }
}

impl Paragraph for CtPara {
    fn size(&self) -> (f32, f32) {
        let w = self.lines.iter().map(|l| l.x + l.width).fold(0.0, f32::max);
        (w, self.lines.last().map_or(0.0, |l| l.top + l.height))
    }
    fn content_widths(&self) -> (f32, f32) {
        // Narrowest: the widest word; widest: the widest hard line, unbroken.
        let (mut min, mut max) = (0.0f32, 0.0f32);
        let (mut word, mut line) = (0usize, 0usize);
        let n = self.len16();
        for i in 0..=n {
            if i == n || matches!(self.text_chars.get(i), Some(' ' | '\t' | '\n')) {
                if i > word {
                    min = min.max(self.range_width(word..i));
                }
                word = i + 1;
                if i == n || self.text_chars.get(i) == Some(&'\n') {
                    if i > line {
                        max = max.max(self.range_width(line..i));
                    }
                    line = i + 1;
                }
            }
        }
        (min, max.max(min))
    }
    fn set_width(&mut self, width: f32) {
        self.max_width = width.max(1.0);
        self.break_lines();
    }
    fn set_align(&mut self, align: TextAlign) {
        self.align = Some(align);
        self.break_lines();
    }
    fn hit(&self, x: f32, y: f32) -> usize {
        let Some(l) = self.line_at(y) else { return 0 };
        let i = unsafe { CTLineGetStringIndexForPosition(l.line, CGPoint { x: (x - l.x) as f64, y: 0.0 }) };
        let i = if i < 0 { l.start } else { (i as usize).clamp(l.start, l.start + l.len) };
        self.to_byte(i)
    }
    fn range_rects(&self, range: Range<usize>) -> Vec<RectF> {
        let (s, e) = (self.to_u16(range.start), self.to_u16(range.end));
        self.lines
            .iter()
            .filter(|l| s < l.start + l.len && e > l.start)
            .map(|l| {
                let (a, b) = (Self::offset(l, s.max(l.start)), Self::offset(l, e.min(l.start + l.len)));
                RectF { x: l.x + a, y: l.top, w: b - a, h: l.height }
            })
            .collect()
    }
    fn object_origin(&self, byte: usize) -> (f32, f32) {
        let i = self.to_u16(byte);
        let asc = self.objects.iter().find(|o| o.0 == i).map_or(0.0, |o| o.1);
        let Some(l) = self.line_of(i) else { return (0.0, 0.0) };
        (l.x + Self::offset(l, i), l.baseline - asc)
    }
}

/// An inline object's metrics for Core Text (boxed, freed by `dealloc`).
struct Metrics {
    asc: f64,
    desc: f64,
    width: f64,
}

unsafe extern "C" fn rd_dealloc(p: *mut c_void) {
    drop(unsafe { Box::from_raw(p as *mut Metrics) });
}
unsafe extern "C" fn rd_ascent(p: *mut c_void) -> CGFloat {
    unsafe { (*(p as *const Metrics)).asc }
}
unsafe extern "C" fn rd_descent(p: *mut c_void) -> CGFloat {
    unsafe { (*(p as *const Metrics)).desc }
}
unsafe extern "C" fn rd_width(p: *mut c_void) -> CGFloat {
    unsafe { (*(p as *const Metrics)).width }
}

static RUN_DELEGATE: CTRunDelegateCallbacks = CTRunDelegateCallbacks { version: 1, dealloc: rd_dealloc, ascent: rd_ascent, descent: rd_descent, width: rd_width };

struct MathFace {
    font: MathFont,
    ct: CTFontRef,
    /// The face at each size used.
    sized: VecMap<u32, CTFontRef>,
}

/// STIX Two Math, which macOS ships.
fn load_math_face() -> Option<MathFace> {
    const PATHS: [&str; 2] = ["/System/Library/Fonts/Supplemental/STIXTwoMath.otf", "/Library/Fonts/STIXTwoMath-Regular.otf"];
    for p in PATHS {
        let Some(data) = crate::util::fs::read(std::path::Path::new(p)) else { continue };
        let Some(&off) = crate::math::otf::faces(&data).iter().find(|&&o| crate::math::otf::has_math(&data, o)) else { continue };
        let Some(mut font) = MathFont::new(data, off) else { continue };
        unsafe {
            let url = CFURLCreateFromFileSystemRepresentation(kCFAllocatorDefault, p.as_ptr(), p.len() as CFIndex, false);
            let provider = CGDataProviderCreateWithURL(url);
            CFRelease(url);
            if provider.is_null() {
                continue;
            }
            let cg = CGFontCreateWithDataProvider(provider);
            CFRelease(provider);
            if cg.is_null() {
                continue;
            }
            let ct = CTFontCreateWithGraphicsFont(cg, 16.0, core::ptr::null(), core::ptr::null());
            // STIX Two Math has CFF outlines (no glyf table): Core Text measures the glyphs, at
            // the em size so the boxes come out in font units.
            if font.needs_bboxes() {
                let upem = CTFontCreateWithGraphicsFont(cg, font.upem() as CGFloat, core::ptr::null(), core::ptr::null());
                let n = CTFontGetGlyphCount(upem).max(0) as usize;
                let glyphs: Vec<CGGlyph> = (0..n as u32).map(|g| g as CGGlyph).collect();
                let mut rects = vec![CGRect::default(); n];
                CTFontGetBoundingRectsForGlyphs(upem, 0, glyphs.as_ptr(), rects.as_mut_ptr(), n as CFIndex);
                CFRelease(upem);
                let b = |v: CGFloat| v.round() as i16;
                font.set_bboxes(rects.iter().map(|r| if r.size.width <= 0.0 || r.size.height <= 0.0 { [0; 4] } else { [b(r.origin.x), b(r.origin.y), b(r.origin.x + r.size.width), b(r.origin.y + r.size.height)] }).collect());
            }
            CFRelease(cg);
            return Some(MathFace { font, ct, sized: VecMap::new() });
        }
    }
    None
}

/// Fonts, images, the math font and the frame's context.
pub struct Gfx {
    fonts: VecMap<(u32, bool, bool, bool), CTFontRef>,
    colors: VecMap<Ink, CGColorRef>,
    /// Particle glyph lines by (text, size).
    sprites: VecMap<(&'static str, u32), CTLineRef>,
    math: Option<MathFace>,
    pub images: super::images::Images,
    /// The view's context during drawRect:.
    pub ctx: CGContextRef,
}

const BOLD: u32 = 1 << 1;
const ITALIC: u32 = 1 << 0;

impl Gfx {
    pub fn new(images: super::images::Images) -> Gfx {
        Gfx { fonts: VecMap::new(), colors: VecMap::new(), sprites: VecMap::new(), math: load_math_face(), images, ctx: core::ptr::null_mut() }
    }

    fn font(&mut self, size: f32, mono: bool, bold: bool, italic: bool) -> CTFontRef {
        *self.fonts.or_insert_with((size.to_bits(), mono, bold, italic), || unsafe {
            let base = if mono {
                let name = cfstring("Menlo");
                let f = CTFontCreateWithName(name, size as f64, core::ptr::null());
                CFRelease(name);
                f
            } else {
                CTFontCreateUIFontForLanguage(2, size as f64, core::ptr::null()) // kCTFontUIFontSystem
            };
            let traits = if bold { BOLD } else { 0 } | if italic { ITALIC } else { 0 };
            if traits == 0 {
                return base;
            }
            let styled = CTFontCreateCopyWithSymbolicTraits(base, 0.0, core::ptr::null(), traits, traits);
            if styled.is_null() {
                base
            } else {
                CFRelease(base);
                styled
            }
        })
    }

    fn color(&mut self, ink: Ink) -> CGColorRef {
        *self.colors.or_insert_with(ink, || cgcolor(ink))
    }

    fn sprite_line(&mut self, text: &'static str, size: f32) -> CTLineRef {
        if let Some(&l) = self.sprites.get(&(text, size.to_bits())) {
            return l;
        }
        let font = self.font(size, false, false, false);
        unsafe {
            let s = CFAttributedStringCreateMutable(kCFAllocatorDefault, 0);
            let cs = cfstring(text);
            CFAttributedStringReplaceString(s, CFRange { location: 0, length: 0 }, cs);
            let all = CFRange { location: 0, length: CFStringGetLength(cs) };
            CFRelease(cs);
            CFAttributedStringSetAttribute(s, all, kCTFontAttributeName, font);
            let ts = CTTypesetterCreateWithAttributedString(s);
            let line = CTTypesetterCreateLine(ts, all);
            CFRelease(ts);
            CFRelease(s);
            self.sprites.insert((text, size.to_bits()), line);
            line
        }
    }
}

impl Typesetter for Gfx {
    type Para = CtPara;

    fn image_size(&self, url: &str) -> Option<(f32, f32)> {
        self.images.size(url)
    }

    fn math_font(&self) -> Option<&MathFont> {
        self.math.as_ref().map(|m| &m.font)
    }

    fn paragraph(&mut self, text: &str, spans: &[(Range<usize>, Span)], base: &BaseStyle, max_width: f32) -> CtPara {
        let mut u16_to_byte = Vec::with_capacity(text.len() + 1);
        let mut text_chars = Vec::with_capacity(text.len());
        for (i, c) in text.char_indices() {
            for _ in 0..c.len_utf16() {
                u16_to_byte.push(i as u32);
                text_chars.push(c);
            }
        }
        u16_to_byte.push(text.len() as u32);
        let to16 = |byte: usize| u16_to_byte.partition_point(|&b| (b as usize) < byte);
        let font = self.font(base.size, base.mono, base.bold, base.italic);
        let base_color = self.color(base.color);
        let mut objects = Vec::new();
        let mut strikes = Vec::new();
        unsafe {
            let s = CFAttributedStringCreateMutable(kCFAllocatorDefault, 0);
            let cs = cfstring(text);
            CFAttributedStringReplaceString(s, CFRange { location: 0, length: 0 }, cs);
            CFRelease(cs);
            let n = (u16_to_byte.len() - 1) as CFIndex;
            let all = CFRange { location: 0, length: n };
            CFAttributedStringSetAttribute(s, all, kCTFontAttributeName, font);
            CFAttributedStringSetAttribute(s, all, kCTForegroundColorAttributeName, base_color);
            for (r, span) in spans {
                let (a, b) = (to16(r.start), to16(r.end));
                if b <= a {
                    continue;
                }
                let range = CFRange { location: a as CFIndex, length: (b - a) as CFIndex };
                // Fonts of a span combine with the base style.
                let styled = |g: &mut Gfx, size: f32, mono: bool, bold: bool, italic: bool| g.font(size, mono, bold, italic);
                match *span {
                    Span::Bold => CFAttributedStringSetAttribute(s, range, kCTFontAttributeName, styled(self, base.size, base.mono, true, base.italic)),
                    Span::Italic => CFAttributedStringSetAttribute(s, range, kCTFontAttributeName, styled(self, base.size, base.mono, base.bold, true)),
                    Span::Mono => CFAttributedStringSetAttribute(s, range, kCTFontAttributeName, styled(self, base.size, true, base.bold, base.italic)),
                    Span::Scale(k) => CFAttributedStringSetAttribute(s, range, kCTFontAttributeName, styled(self, base.size * k, base.mono, base.bold, base.italic)),
                    Span::Color(c) => CFAttributedStringSetAttribute(s, range, kCTForegroundColorAttributeName, self.color(c)),
                    Span::Underline => {
                        let one = 1i32;
                        let v = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &one as *const i32 as *const c_void);
                        CFAttributedStringSetAttribute(s, range, kCTUnderlineStyleAttributeName, v);
                        CFRelease(v);
                    }
                    Span::Super | Span::Sub => {
                        let v: i32 = if matches!(span, Span::Super) { 1 } else { -1 };
                        let n = CFNumberCreate(kCFAllocatorDefault, kCFNumberSInt32Type, &v as *const i32 as *const c_void);
                        CFAttributedStringSetAttribute(s, range, kCTSuperscriptAttributeName, n);
                        CFRelease(n);
                    }
                    Span::Strike => {
                        let color = spans.iter().rev().find_map(|(cr, cs)| match cs {
                            Span::Color(c) if cr.start <= r.start && cr.end >= r.end => Some(*c),
                            _ => None,
                        });
                        strikes.push((a..b, color.unwrap_or(base.color), base.size));
                    }
                    Span::Object { w, asc, desc } => {
                        let m = Box::into_raw(Box::new(Metrics { asc: asc as f64, desc: desc as f64, width: w as f64 }));
                        let d = CTRunDelegateCreate(&RUN_DELEGATE, m as *mut c_void);
                        CFAttributedStringSetAttribute(s, range, kCTRunDelegateAttributeName, d);
                        CFRelease(d);
                        objects.push((a, asc));
                    }
                }
            }
            let typesetter = CTTypesetterCreateWithAttributedString(s);
            CFRelease(s);
            let line_height = (base.size * base.line_height).round();
            let mut p = CtPara { typesetter, u16_to_byte, text_chars, lines: Vec::new(), max_width: max_width.max(1.0), align: None, line_height, baseline: (line_height * 0.78).round(), objects, strikes };
            p.break_lines();
            p
        }
    }
}

impl crate::app::Gfx for Gfx {
    fn set_image_base(&mut self, dir: Option<PathBuf>) {
        self.images.set_base(dir);
    }

    fn draw(&mut self, frame: &Frame<CtPara>) -> bool {
        if self.ctx.is_null() {
            return true;
        }
        let ctx = self.ctx;
        unsafe {
            CGContextSaveGState(ctx);
            CGContextSetTextMatrix(ctx, CGAffineTransform { a: 1.0, b: 0.0, c: 0.0, d: -1.0, tx: 0.0, ty: 0.0 });
            crate::paint::frame(&mut CgCanvas { gfx: self, ctx, current: crate::paint::IDENTITY }, frame);
            CGContextRestoreGState(ctx);
        }
        true
    }
}

/// `paint::Canvas` on the view's (flipped) Core Graphics context.
struct CgCanvas<'a> {
    gfx: &'a mut Gfx,
    ctx: CGContextRef,
    /// The transform set last (Core Graphics only concatenates).
    current: Affine,
}

fn inverse(m: Affine) -> Affine {
    let [a, b, c, d, e, f] = m;
    let det = a * d - b * c;
    if det == 0.0 {
        return crate::paint::IDENTITY;
    }
    let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
    [ia, ib, ic, id, -(e * ia + f * ic), -(e * ib + f * id)]
}

fn cg(m: Affine) -> CGAffineTransform {
    let [a, b, c, d, tx, ty] = m.map(f64::from);
    CGAffineTransform { a, b, c, d, tx, ty }
}

impl CgCanvas<'_> {
    fn fill_color(&self, ink: Ink) {
        let [r, g, b, a] = ink.0;
        unsafe { CGContextSetRGBFillColor(self.ctx, r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, a as f64 / 255.0) };
    }
}

impl Canvas for CgCanvas<'_> {
    type Para = CtPara;

    fn clear(&mut self, ink: Ink) {
        self.fill_color(ink);
        unsafe { CGContextFillRect(self.ctx, CGRect::new(-1e6, -1e6, 2e6, 2e6)) };
    }

    fn transform(&mut self, m: Affine) {
        // Undo the current transform, then apply `m` (CG concatenates: the new one acts first).
        unsafe { CGContextConcatCTM(self.ctx, cg(crate::paint::then(m, inverse(self.current)))) };
        self.current = m;
    }

    fn fill(&mut self, r: RectF, ink: Ink, radius: f32) {
        self.fill_color(ink);
        let rect = CGRect::new(r.x as f64, r.y as f64, r.w as f64, r.h as f64);
        unsafe {
            if radius > 0.0 {
                let rad = (radius as f64).min(rect.size.width / 2.0).min(rect.size.height / 2.0);
                let path = CGPathCreateWithRoundedRect(rect, rad, rad, core::ptr::null());
                CGContextBeginPath(self.ctx);
                CGContextAddPath(self.ctx, path);
                CGContextFillPath(self.ctx);
                CGPathRelease(path);
            } else {
                CGContextFillRect(self.ctx, rect);
            }
        }
    }

    fn fill_path(&mut self, pts: &[(f32, f32)], ink: Ink) {
        self.fill_color(ink);
        let c = self.ctx;
        let p = |i: usize| (pts[i].0 as f64, pts[i].1 as f64);
        unsafe {
            CGContextBeginPath(c);
            CGContextMoveToPoint(c, p(0).0, p(0).1);
            let mut i = 1;
            while i + 2 < pts.len() {
                CGContextAddCurveToPoint(c, p(i).0, p(i).1, p(i + 1).0, p(i + 1).1, p(i + 2).0, p(i + 2).1);
                i += 3;
            }
            for j in i..pts.len() {
                CGContextAddLineToPoint(c, p(j).0, p(j).1);
            }
            CGContextClosePath(c);
            CGContextFillPath(c);
        }
    }

    fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, ink: Ink) {
        self.fill_color(ink);
        unsafe { CGContextFillEllipseInRect(self.ctx, CGRect::new((cx - rx) as f64, (cy - ry) as f64, 2.0 * rx as f64, 2.0 * ry as f64)) };
    }

    fn push_opacity(&mut self, a: f32) {
        unsafe {
            CGContextSaveGState(self.ctx);
            CGContextSetAlpha(self.ctx, a as f64);
            CGContextBeginTransparencyLayer(self.ctx, core::ptr::null());
        }
    }

    fn pop_opacity(&mut self) {
        unsafe {
            CGContextEndTransparencyLayer(self.ctx);
            CGContextRestoreGState(self.ctx);
        }
    }

    fn text(&mut self, para: &CtPara, x: f32, y: f32, _ink: Ink) {
        let c = self.ctx;
        unsafe {
            for l in &para.lines {
                CGContextSetTextPosition(c, (x + l.x) as f64, (y + l.baseline) as f64);
                CTLineDraw(l.line, c);
            }
        }
        for (r, ink, size) in &para.strikes {
            for l in para.lines.iter().filter(|l| r.start < l.start + l.len && r.end > l.start) {
                let (a, b) = (CtPara::offset(l, r.start.max(l.start)), CtPara::offset(l, r.end.min(l.start + l.len)));
                let t = (size / 14.0).max(1.0);
                self.fill(RectF { x: x + l.x + a, y: y + l.baseline - size * 0.3 - t / 2.0, w: b - a, h: t }, *ink, 0.0);
            }
        }
    }

    fn image(&mut self, url: &str, r: RectF) {
        let Some(img) = self.gfx.images.image(url) else { return };
        let c = self.ctx;
        unsafe {
            CGContextSaveGState(c);
            CGContextSetInterpolationQuality(c, 3); // kCGInterpolationHigh
            CGContextTranslateCTM(c, r.x as f64, (r.y + r.h) as f64);
            CGContextScaleCTM(c, 1.0, -1.0);
            CGContextDrawImage(c, CGRect::new(0.0, 0.0, r.w as f64, r.h as f64), img);
            CGContextRestoreGState(c);
        }
    }

    fn glyph(&mut self, id: u16, x: f32, y: f32, size: f32, ink: Ink) {
        let Some(m) = self.gfx.math.as_mut() else { return };
        let ct = m.ct;
        let font = *m.sized.or_insert_with(size.to_bits(), || unsafe { CTFontCreateCopyWithAttributes(ct, size as f64, core::ptr::null(), core::ptr::null()) });
        self.fill_color(ink);
        let c = self.ctx;
        // The text matrix is already flipped for the flipped context; its translation is the text
        // position, so the glyph goes there at the origin of text space.
        unsafe {
            CGContextSetTextPosition(c, x as f64, y as f64);
            CTFontDrawGlyphs(font, &id, &CGPoint::default(), 1, c);
        }
    }

    fn sprite(&mut self, text: &'static str, size: f32, ink: Ink, alpha: f32, x: f32, y: f32, rotation: f32) {
        let line = self.gfx.sprite_line(text, size);
        let c = self.ctx;
        let (cx, cy) = ((x + size / 2.0) as f64, (y + size / 2.0) as f64);
        self.fill_color(ink);
        unsafe {
            CGContextSaveGState(c);
            CGContextTranslateCTM(c, cx, cy);
            CGContextRotateCTM(c, (rotation as f64).to_radians());
            CGContextTranslateCTM(c, -cx, -cy);
            CGContextSetAlpha(c, alpha as f64);
            CGContextSetTextPosition(c, x as f64, (y + size * 0.9) as f64);
            CTLineDraw(line, c);
            CGContextRestoreGState(c);
        }
    }
}
