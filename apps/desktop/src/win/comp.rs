//! Particle overlay on DirectComposition: each particle is a visual showing its glyph,
//! rendered once; its fall, sway and rocking are composition animations, so the
//! compositor moves the particles at the display's rate and the app draws no frames
//! for them (before, every particle frame redrew the whole document).

use super::dwrite::{Gfx, particle_ink, particle_tint};
use crate::particles::{Particles, Track, Wave};
use crate::theme::{Ink, Theme};
use crate::util::VecMap;
use windows::Win32::Foundation::{HMODULE, HWND, POINT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_WARP;
use windows::Win32::Graphics::Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice};
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::core::{Interface, Result};

type Surfaces = VecMap<(&'static str, u32, u32, u32), (IDCompositionSurface, f32)>;

pub struct Overlay {
    device: IDCompositionDesktopDevice,
    _d2d: ID2D1Device,
    _target: IDCompositionTarget,
    root: IDCompositionVisual2,
    /// Glyph surfaces by (text, size, ink, scale): (surface, padding in DIPs).
    surfaces: Surfaces,
    /// What the visuals show: (particle generation, height, scale, tint).
    shown: Option<(u32, u32, u32, u32)>,
}

impl Overlay {
    /// The composition device for `hwnd` on WARP (a software device starts in a few
    /// ms; it only renders each glyph once, the compositor moves them).
    pub fn new(hwnd: HWND) -> Result<Overlay> {
        unsafe {
            let mut d3d = None;
            D3D11CreateDevice(None, D3D_DRIVER_TYPE_WARP, HMODULE::default(), D3D11_CREATE_DEVICE_BGRA_SUPPORT, None, D3D11_SDK_VERSION, Some(&mut d3d), None, None)?;
            let dxgi: IDXGIDevice = d3d.ok_or_else(windows::core::Error::empty)?.cast()?;
            let factory: ID2D1Factory1 = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let d2d = factory.CreateDevice(&dxgi)?;
            let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(&d2d)?;
            let target = device.CreateTargetForHwnd(hwnd, true)?;
            let root = device.CreateVisual()?;
            root.SetBitmapInterpolationMode(DCOMPOSITION_BITMAP_INTERPOLATION_MODE_LINEAR)?;
            target.SetRoot(&root)?;
            Ok(Overlay { device, _d2d: d2d, _target: target, root, surfaces: VecMap::new(), shown: None })
        }
    }

    /// Show `ps` (generation `generation`) animated from `ms` on, in a window `height`
    /// DIPs high at `scale` pixels per DIP. Nothing to do while all of that stays.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, gfx: &mut Gfx, ps: &Particles, generation: u32, theme: &Theme, height: f32, scale: f32, ms: f32) -> Result<()> {
        let tint = particle_tint(ps, theme);
        let key = (generation, height.to_bits(), scale.to_bits(), u32::from_le_bytes(tint.0));
        if self.shown == Some(key) {
            return Ok(());
        }
        // Surfaces still in use carry over; the rest (another theme, scale) go.
        let mut old = std::mem::take(&mut self.surfaces);
        unsafe {
            self.root.RemoveAllVisuals()?;
            for (p, track) in ps.items.iter().zip(ps.tracks(ms, height)) {
                let (ink, fade) = particle_ink(tint, p);
                let Some((surface, pad)) = self.surface(gfx, &mut old, p.text, p.size, ink, scale)? else { continue };
                let v = self.device.CreateVisual()?;
                v.SetContent(&surface)?;
                // Offsets in pixels; the glyph's origin sits `pad` inside its surface.
                v.SetOffsetX(&self.wave(track.x, -pad, scale)?)?;
                v.SetOffsetY(&self.fall(&track, -pad, scale)?)?;
                let rotate = self.device.CreateRotateTransform()?;
                let centre = (pad + p.size / 2.0) * scale;
                rotate.SetCenterX2(centre)?;
                rotate.SetCenterY2(centre)?;
                rotate.SetAngle(&self.wave(track.rotation, 0.0, 1.0)?)?;
                v.SetTransform(&rotate)?;
                if fade < 1.0 {
                    let effect = self.device.CreateEffectGroup()?;
                    effect.SetOpacity2(fade)?;
                    v.SetEffect(&effect)?;
                }
                self.root.AddVisual(&v, true, None::<&IDCompositionVisual>)?;
            }
            self.device.Commit()?;
        }
        self.shown = Some(key);
        Ok(())
    }

    /// Remove the particles from the window (the overlay is about to go).
    pub fn clear(&self) {
        unsafe {
            let _ = self.root.RemoveAllVisuals();
            let _ = self.device.Commit();
        }
    }

    /// `(w.at(τ) + offset) · scale`, τ in seconds.
    fn wave(&self, w: Wave, offset: f32, scale: f32) -> Result<IDCompositionAnimation> {
        unsafe {
            let a = self.device.CreateAnimation()?;
            a.AddSinusoidal(0.0, (w.bias + offset) * scale, w.amplitude * scale, w.frequency * 1000.0, w.phase)?;
            Ok(a)
        }
    }

    /// The track's y, `(y + offset) · scale`: wait above the window, then fall at a
    /// constant speed and start over at the top, for ever.
    fn fall(&self, t: &Track, offset: f32, scale: f32) -> Result<IDCompositionAnimation> {
        let secs = |ms: f32| (ms / 1000.0) as f64;
        let y = |v: f32| (v + offset) * scale;
        let speed = t.speed * 1000.0 * scale;
        unsafe {
            let a = self.device.CreateAnimation()?;
            let mut at = 0.0;
            if t.wait > 0.0 {
                a.AddCubic(0.0, y(t.top), 0.0, 0.0, 0.0)?;
                at = secs(t.wait);
            }
            a.AddCubic(at, y(t.top + t.phase * t.speed), speed, 0.0, 0.0)?;
            let wrap = at + secs(t.fall - t.phase);
            a.AddCubic(wrap, y(t.top), speed, 0.0, 0.0)?;
            a.AddRepeat(wrap + secs(t.fall), secs(t.fall))?;
            Ok(a)
        }
    }

    /// The glyph rendered once onto a surface of its own (transparent, grayscale text).
    fn surface(&mut self, gfx: &mut Gfx, old: &mut Surfaces, text: &'static str, size: f32, ink: Ink, scale: f32) -> Result<Option<(IDCompositionSurface, f32)>> {
        let key = (text, size.to_bits(), u32::from_le_bytes(ink.0), scale.to_bits());
        if let Some(s) = self.surfaces.get(&key) {
            return Ok(Some(s.clone()));
        }
        if let Some(s) = old.remove(&key) {
            self.surfaces.insert(key, s.clone());
            return Ok(Some(s));
        }
        // Rendered by the window's own software rasteriser; this device only copies it.
        let Some((pixels, w, h, pad)) = gfx.sprite_pixels(text, size, ink, scale) else { return Ok(None) };
        unsafe {
            let surface = self.device.CreateSurface(w, h, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_ALPHA_MODE_PREMULTIPLIED)?;
            let mut origin = POINT::default();
            let dc: ID2D1DeviceContext = surface.BeginDraw(None, &mut origin)?;
            dc.SetDpi(96.0, 96.0);
            let format = D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED };
            let props = D2D1_BITMAP_PROPERTIES { pixelFormat: format, dpiX: 96.0, dpiY: 96.0 };
            let bitmap = (*dc).CreateBitmap(D2D_SIZE_U { width: w, height: h }, Some(pixels.as_ptr() as *const _), w * 4, &props)?;
            // The surface may share its texture with others: write only this glyph's part.
            let rect = D2D_RECT_F { left: origin.x as f32, top: origin.y as f32, right: (origin.x as u32 + w) as f32, bottom: (origin.y as u32 + h) as f32 };
            dc.PushAxisAlignedClip(&rect, D2D1_ANTIALIAS_MODE_ALIASED);
            dc.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            dc.DrawBitmap(&bitmap, Some(&rect), 1.0, D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR, None, None);
            dc.PopAxisAlignedClip();
            surface.EndDraw()?;
            self.surfaces.insert(key, (surface.clone(), pad));
            Ok(Some((surface, pad)))
        }
    }
}
