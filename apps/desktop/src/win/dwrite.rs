//! DirectWrite text layout + Direct2D drawing with ClearType.

use crate::layout::{BaseStyle, DocLayout, Paragraph, RectF, Span, TextAlign, Typesetter};
use crate::layout::DocSelection;
use crate::theme::{Ink, Theme};
use std::collections::HashMap;
use std::ops::Range;
use windows::Win32::Foundation::{D2DERR_RECREATE_TARGET, HWND};
use windows::core::HRESULT;

/// HRESULT_FROM_WIN32(ERROR_INSUFFICIENT_BUFFER)
const E_NOT_SUFFICIENT_BUFFER: HRESULT = HRESULT(0x8007007A_u32 as i32);
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::{BOOL, HSTRING, Interface, PCWSTR, Result};

const BODY_FAMILIES: &[&str] = &["Segoe UI Variable Text", "Segoe UI"];
const MONO_FAMILIES: &[&str] = &["Cascadia Mono", "Consolas", "Courier New"];

pub struct DwPara {
    layout: IDWriteTextLayout,
    /// UTF-16 index → UTF-8 byte offset (len = utf16 len + 1).
    u16_to_byte: Vec<u32>,
}

impl DwPara {
    fn to_u16(&self, byte: usize) -> u32 {
        self.u16_to_byte.partition_point(|&b| (b as usize) < byte) as u32
    }
    fn to_byte(&self, pos: u32) -> usize {
        self.u16_to_byte[(pos as usize).min(self.u16_to_byte.len() - 1)] as usize
    }
    fn metrics(&self) -> DWRITE_TEXT_METRICS {
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe { self.layout.GetMetrics(&mut m).ok() };
        m
    }
    pub fn layout(&self) -> &IDWriteTextLayout {
        &self.layout
    }
}

impl Paragraph for DwPara {
    fn size(&self) -> (f32, f32) {
        let m = self.metrics();
        (m.widthIncludingTrailingWhitespace.max(m.width), m.height)
    }
    fn content_widths(&self) -> (f32, f32) {
        let min = unsafe { self.layout.DetermineMinWidth() }.unwrap_or(0.0);
        let m = self.metrics();
        (min, m.widthIncludingTrailingWhitespace)
    }
    fn set_width(&mut self, width: f32) {
        unsafe { self.layout.SetMaxWidth(width.max(1.0)).ok() };
    }
    fn set_align(&mut self, align: TextAlign) {
        let a = match align {
            TextAlign::Start => DWRITE_TEXT_ALIGNMENT_LEADING,
            TextAlign::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            TextAlign::End => DWRITE_TEXT_ALIGNMENT_TRAILING,
        };
        unsafe { self.layout.SetTextAlignment(a).ok() };
    }
    fn hit(&self, x: f32, y: f32) -> usize {
        let mut trailing = BOOL(0);
        let mut inside = BOOL(0);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        if unsafe { self.layout.HitTestPoint(x, y, &mut trailing, &mut inside, &mut m) }.is_err() {
            return 0;
        }
        let pos = m.textPosition + if trailing.as_bool() { m.length } else { 0 };
        self.to_byte(pos)
    }
    fn range_rects(&self, range: Range<usize>) -> Vec<RectF> {
        let start = self.to_u16(range.start);
        let len = self.to_u16(range.end).saturating_sub(start);
        if len == 0 {
            return Vec::new();
        }
        let mut count = 0u32;
        let r = unsafe { self.layout.HitTestTextRange(start, len, 0.0, 0.0, None, &mut count) };
        if let Err(e) = r {
            if e.code() != E_NOT_SUFFICIENT_BUFFER {
                return Vec::new();
            }
        }
        let mut metrics = vec![DWRITE_HIT_TEST_METRICS::default(); count as usize];
        if unsafe { self.layout.HitTestTextRange(start, len, 0.0, 0.0, Some(&mut metrics), &mut count) }.is_err() {
            return Vec::new();
        }
        metrics.truncate(count as usize);
        metrics.iter().map(|m| RectF { x: m.left, y: m.top, w: m.width, h: m.height }).collect()
    }
}

/// Owns the DirectWrite factory, the window's render target and the brush cache.
pub struct Gfx {
    hwnd: HWND,
    d2d: ID2D1Factory,
    dw: IDWriteFactory,
    rt: Option<ID2D1HwndRenderTarget>,
    brushes: HashMap<Ink, ID2D1SolidColorBrush>,
    body: HSTRING,
    mono: HSTRING,
    typo_sup: IDWriteTypography,
    typo_sub: IDWriteTypography,
    formats: HashMap<(u32, bool, bool, bool, u32), IDWriteTextFormat>,
}

fn color(ink: Ink) -> D2D1_COLOR_F {
    let [r, g, b, a] = ink.0;
    D2D1_COLOR_F { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: a as f32 / 255.0 }
}

fn rect(r: RectF) -> D2D_RECT_F {
    D2D_RECT_F { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h }
}

fn range(start: u32, len: u32) -> DWRITE_TEXT_RANGE {
    DWRITE_TEXT_RANGE { startPosition: start, length: len }
}

impl Gfx {
    pub fn new(hwnd: HWND) -> Result<Self> {
        unsafe {
            let d2d: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let mut fonts = None;
            dw.GetSystemFontCollection(&mut fonts, false)?;
            let fonts = fonts.expect("system font collection");
            let pick = |names: &[&str]| -> HSTRING {
                for n in names {
                    let mut index = 0u32;
                    let mut exists = BOOL(0);
                    let name = HSTRING::from(*n);
                    if fonts.FindFamilyName(&name, &mut index, &mut exists).is_ok() && exists.as_bool() {
                        return name;
                    }
                }
                HSTRING::from(names[names.len() - 1])
            };
            let body = pick(BODY_FAMILIES);
            let mono = pick(MONO_FAMILIES);
            let typo = |tag: DWRITE_FONT_FEATURE_TAG| -> Result<IDWriteTypography> {
                let t = dw.CreateTypography()?;
                t.AddFontFeature(DWRITE_FONT_FEATURE { nameTag: tag, parameter: 1 })?;
                Ok(t)
            };
            Ok(Gfx {
                hwnd,
                typo_sup: typo(DWRITE_FONT_FEATURE_TAG_SUPERSCRIPT)?,
                typo_sub: typo(DWRITE_FONT_FEATURE_TAG_SUBSCRIPT)?,
                d2d,
                dw,
                rt: None,
                brushes: HashMap::new(),
                body,
                mono,
                formats: HashMap::new(),
            })
        }
    }

    /// Create the render target if needed. Returns true if it was (re)created, in
    /// which case all layouts must be rebuilt (their brushes belong to the old target).
    pub fn ensure_target(&mut self, width: u32, height: u32, dpi: f32) -> Result<bool> {
        if let Some(rt) = &self.rt {
            unsafe { rt.SetDpi(dpi, dpi) };
            return Ok(false);
        }
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_IGNORE },
            dpiX: dpi,
            dpiY: dpi,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: self.hwnd,
            pixelSize: D2D_SIZE_U { width: width.max(1), height: height.max(1) },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let rt = unsafe { self.d2d.CreateHwndRenderTarget(&props, &hwnd_props)? };
        unsafe { rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE) };
        self.rt = Some(rt);
        self.brushes.clear();
        Ok(true)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(rt) = &self.rt {
            unsafe { rt.Resize(&D2D_SIZE_U { width: width.max(1), height: height.max(1) }).ok() };
        }
    }

    fn brush(&mut self, ink: Ink) -> ID2D1SolidColorBrush {
        if let Some(b) = self.brushes.get(&ink) {
            return b.clone();
        }
        let rt = self.rt.as_ref().expect("render target");
        let b = unsafe { rt.CreateSolidColorBrush(&color(ink), None) }.expect("brush");
        self.brushes.insert(ink, b.clone());
        b
    }

    fn format(&mut self, base: &BaseStyle) -> IDWriteTextFormat {
        let key = (base.size.to_bits(), base.mono, base.bold, base.italic, base.line_height.to_bits());
        if let Some(f) = self.formats.get(&key) {
            return f.clone();
        }
        let family = if base.mono { &self.mono } else { &self.body };
        let f = unsafe {
            self.dw.CreateTextFormat(
                family,
                None,
                if base.bold { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL },
                if base.italic { DWRITE_FONT_STYLE_ITALIC } else { DWRITE_FONT_STYLE_NORMAL },
                DWRITE_FONT_STRETCH_NORMAL,
                base.size,
                &HSTRING::from(""),
            )
        }
        .expect("text format");
        let spacing = (base.size * base.line_height).round();
        unsafe { f.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, spacing, (spacing * 0.78).round()).ok() };
        self.formats.insert(key, f.clone());
        f
    }

    /// Paint the visible part of the document.
    pub fn draw(&mut self, doc: &DocLayout<DwPara>, selection: Option<&DocSelection>, theme: &Theme, scroll: f32, view_h: f32) -> Result<()> {
        let Some(rt) = self.rt.clone() else { return Ok(()) };
        let (top, bottom) = (scroll - 50.0, scroll + view_h + 50.0);
        let visible = |y: f32, h: f32| y + h >= top && y <= bottom;
        let text_brush = self.brush(theme.text);
        let sel_brush = self.brush(theme.selection);
        unsafe {
            rt.BeginDraw();
            rt.SetTransform(&windows_numerics::Matrix3x2::translation(0.0, -scroll.round()));
            rt.Clear(Some(&color(theme.background)));
            for r in &doc.rects {
                if !visible(r.r.y, r.r.h) {
                    continue;
                }
                let b = self.brush(r.color);
                if r.radius > 0.0 {
                    rt.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: rect(r.r), radiusX: r.radius, radiusY: r.radius }, &b);
                } else {
                    rt.FillRectangle(&rect(r.r), &b);
                }
            }
            for (i, block) in doc.blocks.iter().enumerate() {
                if !visible(block.y, block.h) {
                    continue;
                }
                if let Some(range) = selection.and_then(|s| doc.block_range(s, i)) {
                    for r in block.para.range_rects(range) {
                        rt.FillRectangle(&rect(RectF { x: block.x + r.x, y: block.y + r.y, ..r }), &sel_brush);
                    }
                }
                rt.DrawTextLayout(
                    windows_numerics::Vector2 { X: block.x, Y: block.y },
                    block.para.layout(),
                    &text_brush,
                    D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT,
                );
            }
            let r = rt.EndDraw(None, None);
            if let Err(e) = &r {
                if e.code() == D2DERR_RECREATE_TARGET {
                    self.rt = None;
                    self.brushes.clear();
                }
            }
            r
        }
    }
}

impl Typesetter for Gfx {
    type Para = DwPara;

    fn paragraph(&mut self, text: &str, spans: &[(Range<usize>, Span)], base: &BaseStyle, max_width: f32) -> DwPara {
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut u16_to_byte = Vec::with_capacity(wide.len() + 1);
        for (i, c) in text.char_indices() {
            for _ in 0..c.len_utf16() {
                u16_to_byte.push(i as u32);
            }
        }
        u16_to_byte.push(text.len() as u32);
        let format = self.format(base);
        let layout = unsafe { self.dw.CreateTextLayout(&wide, &format, max_width.max(1.0), 1.0e6) }.expect("text layout");
        let para = DwPara { layout, u16_to_byte };
        let all = range(0, wide.len() as u32);
        let base_brush = self.brush(base.color);
        unsafe {
            para.layout.SetDrawingEffect(&base_brush.cast::<windows::core::IUnknown>().unwrap(), all).ok();
            for (r, span) in spans {
                let start = para.to_u16(r.start);
                let rr = range(start, para.to_u16(r.end).saturating_sub(start));
                if rr.length == 0 {
                    continue;
                }
                let l = &para.layout;
                match *span {
                    Span::Bold => l.SetFontWeight(DWRITE_FONT_WEIGHT_SEMI_BOLD, rr).ok(),
                    Span::Italic => l.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, rr).ok(),
                    Span::Mono => l.SetFontFamilyName(PCWSTR(self.mono.as_ptr()), rr).ok(),
                    Span::Color(c) => {
                        let b = self.brush(c);
                        l.SetDrawingEffect(&b.cast::<windows::core::IUnknown>().unwrap(), rr).ok()
                    }
                    Span::Strike => l.SetStrikethrough(true, rr).ok(),
                    Span::Underline => l.SetUnderline(true, rr).ok(),
                    Span::Scale(s) => l.SetFontSize(base.size * s, rr).ok(),
                    Span::Super => l.SetTypography(&self.typo_sup, rr).ok(),
                    Span::Sub => l.SetTypography(&self.typo_sub, rr).ok(),
                };
            }
        }
        para
    }
}
