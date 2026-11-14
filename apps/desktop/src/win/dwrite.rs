//! DirectWrite text layout + Direct2D drawing with ClearType.

use crate::layout::{BaseStyle, DocLayout, DocSelection, Paragraph, RectF, Span, TextAlign, Typesetter};
use crate::math::{Item, MathFont};
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
use windows::core::{BOOL, HSTRING, Interface, PCWSTR, Ref, Result, implement};

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
    fn object_origin(&self, byte: usize) -> (f32, f32) {
        let (mut x, mut y) = (0.0f32, 0.0f32);
        let mut m = DWRITE_HIT_TEST_METRICS::default();
        if unsafe { self.layout.HitTestTextPosition(self.to_u16(byte), false, &mut x, &mut y, &mut m) }.is_err() {
            return (0.0, 0.0);
        }
        (m.left, m.top)
    }
}

/// Reserves space for a formula inside a DirectWrite paragraph. Drawing happens
/// separately (glyph runs from the math layout), so `Draw` is a no-op.
#[implement(IDWriteInlineObject)]
struct InlineBox {
    w: f32,
    asc: f32,
    desc: f32,
}

impl IDWriteInlineObject_Impl for InlineBox_Impl {
    fn Draw(&self, _: *const core::ffi::c_void, _: Ref<IDWriteTextRenderer>, _: f32, _: f32, _: BOOL, _: BOOL, _: Ref<windows::core::IUnknown>) -> Result<()> {
        Ok(())
    }
    fn GetMetrics(&self) -> Result<DWRITE_INLINE_OBJECT_METRICS> {
        Ok(DWRITE_INLINE_OBJECT_METRICS { width: self.w, height: self.asc + self.desc, baseline: self.asc, supportsSideways: BOOL(0) })
    }
    fn GetOverhangMetrics(&self) -> Result<DWRITE_OVERHANG_METRICS> {
        Ok(DWRITE_OVERHANG_METRICS::default())
    }
    fn GetBreakConditions(&self, before: *mut DWRITE_BREAK_CONDITION, after: *mut DWRITE_BREAK_CONDITION) -> Result<()> {
        unsafe {
            *before = DWRITE_BREAK_CONDITION_NEUTRAL;
            *after = DWRITE_BREAK_CONDITION_NEUTRAL;
        }
        Ok(())
    }
}

/// Cambria Math: parsed for layout (math::otf) and as a DirectWrite face for drawing.
struct MathFace {
    font: MathFont,
    face: IDWriteFontFace,
}

fn load_math_face(dw: &IDWriteFactory) -> Option<MathFace> {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let path = std::path::Path::new(&windir).join("Fonts").join("cambria.ttc");
    let data = std::fs::read(&path).ok()?;
    // Cambria Math is face 1 of the collection; fall back to scanning.
    let (index, off) = crate::math::otf::faces(&data).into_iter().enumerate().find(|&(_, off)| crate::math::otf::has_math(&data, off))?;
    let index = index as u32;
    let font = MathFont::new(data, off)?;
    unsafe {
        let file = dw.CreateFontFileReference(&HSTRING::from(path.as_os_str()), None).ok()?;
        let face = dw.CreateFontFace(DWRITE_FONT_FACE_TYPE_TRUETYPE_COLLECTION, &[Some(file)], index, DWRITE_FONT_SIMULATIONS_NONE).ok()?;
        Some(MathFace { font, face })
    }
}

/// Owns the DirectWrite factory, the window's render target and the brush cache.
pub struct Gfx {
    hwnd: HWND,
    d2d: ID2D1Factory,
    dw: IDWriteFactory,
    rt: Option<ID2D1RenderTarget>,
    hwnd_rt: Option<ID2D1HwndRenderTarget>,
    /// Offscreen WIC target (snapshots): (factory, bitmap, size).
    offscreen: Option<(windows::Win32::Graphics::Imaging::IWICImagingFactory, windows::Win32::Graphics::Imaging::IWICBitmap, u32, u32)>,
    pub offscreen_mode: bool,
    brushes: HashMap<Ink, ID2D1SolidColorBrush>,
    body: HSTRING,
    mono: HSTRING,
    typo_sup: IDWriteTypography,
    typo_sub: IDWriteTypography,
    formats: HashMap<(u32, bool, bool, bool, u32), IDWriteTextFormat>,
    sprite_layouts: HashMap<(&'static str, u32), IDWriteTextLayout>,
    math: Option<MathFace>,
    pub images: super::images::Images,
    /// Animation clock for time-based effects (quote crossfades), ms.
    pub now_ms: f32,
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
                math: load_math_face(&dw),
                images: super::images::Images::new(hwnd),
                now_ms: 0.0,
                d2d,
                dw,
                rt: None,
                hwnd_rt: None,
                offscreen: None,
                offscreen_mode: false,
                brushes: HashMap::new(),
                body,
                mono,
                formats: HashMap::new(),
                sprite_layouts: HashMap::new(),
            })
        }
    }

    /// Create the render target if needed. Returns true if it was (re)created, in
    /// which case all layouts must be rebuilt (their brushes belong to the old target).
    pub fn ensure_target(&mut self, width: u32, height: u32, dpi: f32) -> Result<bool> {
        if self.offscreen_mode && self.offscreen.as_ref().is_some_and(|o| (o.2, o.3) != (width, height)) {
            self.rt = None;
        }
        if let Some(rt) = &self.rt {
            unsafe { rt.SetDpi(dpi, dpi) };
            return Ok(false);
        }
        if self.offscreen_mode {
            use windows::Win32::Graphics::Imaging::*;
            use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
            unsafe {
                let wic: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
                let bmp = wic.CreateBitmap(width.max(1), height.max(1), &GUID_WICPixelFormat32bppBGR, WICBitmapCacheOnLoad)?;
                let props = D2D1_RENDER_TARGET_PROPERTIES {
                    r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
                    pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_IGNORE },
                    dpiX: dpi,
                    dpiY: dpi,
                    usage: D2D1_RENDER_TARGET_USAGE_NONE,
                    minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                };
                let rt = self.d2d.CreateWicBitmapRenderTarget(&bmp, &props)?;
                rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
                self.rt = Some(rt);
                self.offscreen = Some((wic, bmp, width, height));
            }
            self.images.reset_device();
            self.brushes.clear();
            return Ok(true);
        }
        // Software rasteriser, not the GPU: creating it takes ~40 ms instead of ~160 ms
        // (driver init), the first frame comes ~150 ms earlier, it uses ~14 MB less
        // memory, and particle frames are cheaper (text rasterises on the CPU anyway).
        // Scroll frames cost more CPU only in large windows (4 MP: ~20 vs ~12 ms).
        // It is also the rasteriser the PNG snapshots use.
        let props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE,
            pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_IGNORE },
            dpiX: dpi,
            dpiY: dpi,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd: self.hwnd,
            pixelSize: D2D_SIZE_U { width: width.max(1), height: height.max(1) },
            // Do not block EndDraw until the next vertical blank (DWM composes windowed
            // output anyway): scrolling measured ~47 ms per frame waiting, ~5 ms without.
            presentOptions: D2D1_PRESENT_OPTIONS_IMMEDIATELY,
        };
        let rt = unsafe { self.d2d.CreateHwndRenderTarget(&props, &hwnd_props)? };
        unsafe { rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE) };
        self.rt = Some(rt.cast()?);
        self.hwnd_rt = Some(rt);
        self.images.reset_device();
        self.brushes.clear();
        Ok(true)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(rt) = &self.hwnd_rt {
            unsafe { rt.Resize(&D2D_SIZE_U { width: width.max(1), height: height.max(1) }).ok() };
        }
    }

    /// Write the offscreen target (snapshot mode) as PNG.
    pub fn save_png(&self, path: &std::path::Path) -> Result<()> {
        use windows::Win32::Graphics::Imaging::*;
        let Some((wic, bmp, _, _)) = &self.offscreen else { return Err(windows::core::Error::empty()) };
        unsafe {
            let stream = wic.CreateStream()?;
            stream.InitializeFromFilename(&HSTRING::from(path.as_os_str()), windows::Win32::Foundation::GENERIC_WRITE.0)?;
            let enc = wic.CreateEncoder(&GUID_ContainerFormatPng, std::ptr::null())?;
            enc.Initialize(&stream, WICBitmapEncoderNoCache)?;
            let mut frame = None;
            enc.CreateNewFrame(&mut frame, std::ptr::null_mut())?;
            let frame = frame.ok_or_else(windows::core::Error::empty)?;
            frame.Initialize(None)?;
            frame.WriteSource(bmp, std::ptr::null())?;
            frame.Commit()?;
            enc.Commit()?;
        }
        Ok(())
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

    /// Paint the visible part of the document, then the chrome overlay (window
    /// coordinates) with the given opacity.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        doc: &DocLayout<DwPara>,
        selection: Option<&DocSelection>,
        finds: &[(usize, std::ops::Range<usize>, bool)],
        theme: &Theme,
        scroll: f32,
        view_h: f32,
        chrome: Option<(&DocLayout<DwPara>, f32)>,
        particles: Option<(&crate::particles::Particles, &[crate::particles::Sprite])>,
        tip: &DocLayout<DwPara>,
    ) -> Result<()> {
        let Some(rt) = self.rt.clone() else { return Ok(()) };
        unsafe {
            rt.BeginDraw();
            rt.SetTransform(&windows_numerics::Matrix3x2::translation(0.0, -scroll.round()));
            rt.Clear(Some(&color(theme.background)));
            self.draw_sprites(&rt, particles, true, theme);
            rt.SetTransform(&windows_numerics::Matrix3x2::translation(0.0, -scroll.round()));
            self.draw_layout(&rt, doc, selection, finds, theme, scroll, view_h);
            rt.SetTransform(&windows_numerics::Matrix3x2::identity());
            self.draw_sprites(&rt, particles, false, theme);
            if let Some((c, alpha)) = chrome {
                rt.SetTransform(&windows_numerics::Matrix3x2::identity());
                let params = D2D1_LAYER_PARAMETERS {
                    contentBounds: D2D_RECT_F { left: -1e6, top: -1e6, right: 1e6, bottom: 1e6 },
                    geometricMask: std::mem::ManuallyDrop::new(None),
                    maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                    maskTransform: windows_numerics::Matrix3x2::identity(),
                    opacity: alpha,
                    opacityBrush: std::mem::ManuallyDrop::new(None),
                    layerOptions: D2D1_LAYER_OPTIONS_INITIALIZE_FOR_CLEARTYPE,
                };
                rt.PushLayer(&params, None);
                self.draw_layout(&rt, c, None, &[], theme, 0.0, view_h);
                rt.PopLayer();
            }
            if !tip.rects.is_empty() {
                self.draw_layout(&rt, tip, None, &[], theme, 0.0, view_h);
            }
            let r = rt.EndDraw(None, None);
            if let Err(e) = &r {
                if e.code() == D2DERR_RECREATE_TARGET {
                    self.rt = None;
                    self.hwnd_rt = None;
                    self.brushes.clear();
                    self.images.reset_device();
                }
            }
            r
        }
    }

    /// Falling particles, in window coordinates. `behind` selects the layer
    /// drawn under the text; the rest goes on top.
    unsafe fn draw_sprites(&mut self, rt: &ID2D1RenderTarget, particles: Option<(&crate::particles::Particles, &[crate::particles::Sprite])>, behind: bool, theme: &Theme) {
        let Some((ps, sprites)) = particles else { return };
        let [r, g, b, _] = theme.background.0;
        let dark = (r as u32 + g as u32 + b as u32) < 384;
        let tint = ps.color.map_or(theme.text, |(l, d)| Ink::rgb(if dark { d } else { l }));
        unsafe {
            for s in sprites {
                let p = &ps.items[s.index];
                if p.behind != behind {
                    continue;
                }
                let key = (p.text, p.size.to_bits());
                let layout = match self.sprite_layouts.get(&key) {
                    Some(l) => l.clone(),
                    None => {
                        let base = BaseStyle { size: p.size, color: tint, mono: false, bold: false, italic: false, line_height: 1.0 };
                        let format = self.format(&base);
                        let wide: Vec<u16> = p.text.encode_utf16().collect();
                        let Ok(l) = self.dw.CreateTextLayout(&wide, &format, 400.0, 400.0) else { continue };
                        self.sprite_layouts.insert(key, l.clone());
                        l
                    }
                };
                let c = windows_numerics::Vector2 { X: s.x + p.size / 2.0, Y: s.y + p.size / 2.0 };
                rt.SetTransform(&windows_numerics::Matrix3x2::rotation_around(s.rotation, c));
                let mut brush_color = tint;
                brush_color.0[3] = (p.opacity * 255.0) as u8;
                let brush = self.brush(brush_color);
                let fade = p.opacity < 0.99 && !p.text.is_ascii();
                if fade {
                    // Colour emoji ignore brush alpha: fade them with a layer.
                    let params = D2D1_LAYER_PARAMETERS {
                        contentBounds: D2D_RECT_F { left: s.x - 4.0, top: s.y - 4.0, right: s.x + p.size * 2.0, bottom: s.y + p.size * 2.0 },
                        geometricMask: std::mem::ManuallyDrop::new(None),
                        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                        maskTransform: windows_numerics::Matrix3x2::identity(),
                        opacity: p.opacity,
                        opacityBrush: std::mem::ManuallyDrop::new(None),
                        layerOptions: D2D1_LAYER_OPTIONS_NONE,
                    };
                    rt.PushLayer(&params, None);
                }
                rt.DrawTextLayout(windows_numerics::Vector2 { X: s.x, Y: s.y }, &layout, &brush, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT);
                if fade {
                    rt.PopLayer();
                }
            }
            rt.SetTransform(&windows_numerics::Matrix3x2::identity());
        }
    }

    /// Santa hat (port of the iOS SantaHat SVG, 64×64 viewBox).
    unsafe fn draw_hat(&mut self, rt: &ID2D1RenderTarget, x: f32, y: f32, size: f32, scroll: f32) {
        unsafe {
            let make = |d2d: &ID2D1Factory, pts: &[(f32, f32)]| -> Option<ID2D1PathGeometry> {
                let geo = d2d.CreatePathGeometry().ok()?;
                let sink = geo.Open().ok()?;
                let v = |p: (f32, f32)| windows_numerics::Vector2 { X: p.0, Y: p.1 };
                sink.BeginFigure(v(pts[0]), D2D1_FIGURE_BEGIN_FILLED);
                let mut i = 1;
                while i + 2 < pts.len() {
                    sink.AddBezier(&D2D1_BEZIER_SEGMENT { point1: v(pts[i]), point2: v(pts[i + 1]), point3: v(pts[i + 2]) });
                    i += 3;
                }
                for p in &pts[i..] {
                    sink.AddLine(v(*p));
                }
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close().ok()?;
                Some(geo)
            };
            let body = make(&self.d2d, &[(0.0, 56.0), (4.0, 30.0), (16.0, 14.0), (32.0, 4.0), (48.0, 14.0), (60.0, 30.0), (64.0, 56.0)]);
            let shade = make(&self.d2d, &[(32.0, 4.0), (48.0, 14.0), (60.0, 30.0), (64.0, 56.0), (32.0, 56.0)]);
            let k = size / 64.0;
            let m = windows_numerics::Matrix3x2::scale(k, k)
                * windows_numerics::Matrix3x2::rotation_around(-18.0, windows_numerics::Vector2 { X: 32.0 * k, Y: 56.0 * k })
                * windows_numerics::Matrix3x2::translation(x, y - scroll.round());
            rt.SetTransform(&m);
            let white = self.brush(Ink::rgb(0xffffff));
            let red = self.brush(Ink::rgb(0xcc0000));
            let dark = self.brush(Ink::rgba(0xaa0000, 102));
            if let Some(g) = &body {
                rt.FillGeometry(g, &red, None);
            }
            if let Some(g) = &shade {
                rt.FillGeometry(g, &dark, None);
            }
            rt.FillEllipse(&D2D1_ELLIPSE { point: windows_numerics::Vector2 { X: 32.0, Y: 56.0 }, radiusX: 32.0, radiusY: 8.0 }, &white);
            rt.FillEllipse(&D2D1_ELLIPSE { point: windows_numerics::Vector2 { X: 32.0, Y: 4.0 }, radiusX: 4.0, radiusY: 4.0 }, &white);
            rt.SetTransform(&windows_numerics::Matrix3x2::translation(0.0, -scroll.round()));
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_layout(
        &mut self,
        rt: &ID2D1RenderTarget,
        doc: &DocLayout<DwPara>,
        selection: Option<&DocSelection>,
        finds: &[(usize, std::ops::Range<usize>, bool)],
        theme: &Theme,
        scroll: f32,
        view_h: f32,
    ) {
        let (top, bottom) = (scroll - 50.0, scroll + view_h + 50.0);
        let visible = |y: f32, h: f32| y + h >= top && y <= bottom;
        let text_brush = self.brush(theme.text);
        let sel_brush = self.brush(theme.selection);
        let find_brush = self.brush(theme.find);
        let current_brush = self.brush(theme.find_current);
        // `:::quotes` crossfade: opacity per quote's block and rect ranges (elsewhere 1;
        // the last range containing an index wins).
        let now_ms = self.now_ms;
        let fades: Vec<(&std::ops::Range<usize>, &std::ops::Range<usize>, f32)> = doc
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
        let fade_layer = |rt: &ID2D1RenderTarget, a: f32| unsafe {
            let params = D2D1_LAYER_PARAMETERS {
                contentBounds: D2D_RECT_F { left: -1e6, top: -1e6, right: 1e6, bottom: 1e6 },
                geometricMask: std::mem::ManuallyDrop::new(None),
                maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                maskTransform: windows_numerics::Matrix3x2::identity(),
                opacity: a,
                opacityBrush: std::mem::ManuallyDrop::new(None),
                layerOptions: D2D1_LAYER_OPTIONS_INITIALIZE_FOR_CLEARTYPE,
            };
            rt.PushLayer(&params, None);
        };
        unsafe {
            for (ri, r) in doc.rects.iter().enumerate() {
                let a = rect_alpha(ri);
                if !visible(r.r.y, r.r.h) || a <= 0.0 {
                    continue;
                }
                if a < 1.0 {
                    fade_layer(rt, a);
                }
                let b = self.brush(r.color);
                if r.radius > 0.0 {
                    rt.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: rect(r.r), radiusX: r.radius, radiusY: r.radius }, &b);
                } else {
                    rt.FillRectangle(&rect(r.r), &b);
                }
                if a < 1.0 {
                    rt.PopLayer();
                }
            }

            for img in &doc.images {
                if !visible(img.r.y, img.r.h) {
                    continue;
                }
                if let Some(bmp) = self.images.bitmap(&img.url, rt) {
                    rt.DrawBitmap(&bmp, Some(&rect(img.r)), 1.0, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR, None);
                }
            }
            for (i, block) in doc.blocks.iter().enumerate() {
                let a = block_alpha(i);
                if !visible(block.y, block.h) || a <= 0.0 {
                    continue;
                }
                if a < 1.0 {
                    fade_layer(rt, a);
                }
                for (_, range, current) in finds.iter().filter(|f| f.0 == i) {
                    let b = if *current { &current_brush } else { &find_brush };
                    for r in block.para.range_rects(range.clone()) {
                        rt.FillRoundedRectangle(&D2D1_ROUNDED_RECT { rect: rect(RectF { x: block.x + r.x - 1.0, y: block.y + r.y, w: r.w + 2.0, h: r.h }), radiusX: 3.0, radiusY: 3.0 }, b);
                    }
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
                if a < 1.0 {
                    rt.PopLayer();
                }
            }
            for &(hx, hy, hs) in &doc.hats {
                if visible(hy, hs) {
                    self.draw_hat(rt, hx, hy, hs, scroll);
                }
            }
            if let Some(math) = &self.math {
                let face = math.face.clone();
                for p in &doc.math {
                    if !visible(p.y - p.mbox.asc, p.mbox.asc + p.mbox.desc) {
                        continue;
                    }
                    for item in &p.mbox.items {
                        match *item {
                            Item::Glyph { id, x, y, size, color } => {
                                let brush = self.brush(color.map_or(p.color, |[r, g, b]| Ink([r, g, b, 255])));
                                let advance = 0.0f32;
                                let run = DWRITE_GLYPH_RUN {
                                    fontFace: std::mem::ManuallyDrop::new(Some(face.clone())),
                                    fontEmSize: size,
                                    glyphCount: 1,
                                    glyphIndices: &id,
                                    glyphAdvances: &advance,
                                    glyphOffsets: std::ptr::null(),
                                    isSideways: BOOL(0),
                                    bidiLevel: 0,
                                };
                                rt.DrawGlyphRun(windows_numerics::Vector2 { X: p.x + x, Y: p.y + y }, &run, &brush, DWRITE_MEASURING_MODE_NATURAL);
                                drop(std::mem::ManuallyDrop::into_inner(run.fontFace));
                            }
                            Item::Rule { r, color } => {
                                let brush = self.brush(color.map_or(p.color, |[r, g, b]| Ink([r, g, b, 255])));
                                rt.FillRectangle(&rect(RectF { x: p.x + r.x, y: p.y + r.y, ..r }), &brush);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Typesetter for Gfx {
    type Para = DwPara;

    fn image_size(&self, url: &str) -> Option<(f32, f32)> {
        self.images.size(url)
    }

    fn math_font(&self) -> Option<&MathFont> {
        self.math.as_ref().map(|m| &m.font)
    }

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
                    Span::Object { w, asc, desc } => {
                        let obj: IDWriteInlineObject = InlineBox { w, asc, desc }.into();
                        l.SetInlineObject(&obj, rr).ok()
                    }
                };
            }
            // Lines with formulas must grow to fit them: switch from uniform to
            // proportional spacing (line height × factor) for those paragraphs.
            if spans.iter().any(|(_, s)| matches!(s, Span::Object { .. })) {
                if let Ok(l3) = para.layout.cast::<IDWriteTextLayout3>() {
                    let factor = base.line_height / 1.33;
                    let spacing = DWRITE_LINE_SPACING {
                        method: DWRITE_LINE_SPACING_METHOD_PROPORTIONAL,
                        height: factor,
                        baseline: factor,
                        leadingBefore: 0.0,
                        fontLineGapUsage: DWRITE_FONT_LINE_GAP_USAGE_DEFAULT,
                    };
                    l3.SetLineSpacing(&spacing).ok();
                }
            }
        }
        para
    }
}
