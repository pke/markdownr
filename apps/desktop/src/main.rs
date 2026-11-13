//! Markdownr prototype: winit window + vello (wgpu) renderer + parley layout.

mod ast;
mod layout;
mod render;
mod theme;
mod transforms;

use layout::{DocLayout, DocSelection, Layouter};
use parley::{FontContext, LayoutContext};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use theme::{Ink, Theme};
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, Renderer, RendererOptions, Scene};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, Fullscreen, Window, WindowId};

const DEFAULT_FONT_SIZE: f32 = 16.0;
const MAX_COLUMN: f32 = 46.0; // in multiples of the font size

struct Gpu {
    window: Arc<Window>,
    surface: RenderSurface<'static>,
    renderer: Renderer,
}

struct App {
    render_cx: RenderContext,
    gpu: Option<Gpu>,
    font_cx: FontContext,
    layout_cx: LayoutContext<Ink>,
    scene: Scene,

    source_name: String,
    doc: ast::Node,
    theme: Theme,
    font_size: f32,
    layout: DocLayout,
    layout_width: f32,
    layout_font_size: f32,
    layout_dark: bool,
    dark: bool,

    scroll: f32,
    cursor: (f32, f32),
    modifiers: ModifiersState,
    selection: Option<DocSelection>,
    dragging: bool,
    last_click: Option<(Instant, (f32, f32), u8)>,
    press_pos: (f32, f32),
}

impl App {
    fn new(source_name: String, text: &str) -> Self {
        App {
            render_cx: RenderContext::new(),
            gpu: None,
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            scene: Scene::new(),
            source_name,
            doc: parse(text),
            theme: Theme::light(),
            font_size: DEFAULT_FONT_SIZE,
            layout: DocLayout::default(),
            layout_width: 0.0,
            layout_font_size: 0.0,
            layout_dark: false,
            dark: false,
            scroll: 0.0,
            cursor: (0.0, 0.0),
            modifiers: ModifiersState::empty(),
            selection: None,
            dragging: false,
            last_click: None,
            press_pos: (0.0, 0.0),
        }
    }

    fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.doc = parse(&text);
                self.source_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.scroll = 0.0;
                self.selection = None;
                self.layout_width = 0.0; // force relayout
                if let Some(gpu) = &self.gpu {
                    gpu.window.set_title(&format!("{} – Markdownr", self.source_name));
                    gpu.window.request_redraw();
                }
            }
            Err(e) => eprintln!("cannot open {}: {e}", path.display()),
        }
    }

    fn scale(&self) -> f64 {
        self.gpu.as_ref().map_or(1.0, |g| g.window.scale_factor())
    }

    fn logical_size(&self) -> (f32, f32) {
        let Some(gpu) = &self.gpu else { return (800.0, 600.0) };
        let s = gpu.window.inner_size().to_logical::<f32>(gpu.window.scale_factor());
        (s.width, s.height)
    }

    fn ensure_layout(&mut self) {
        let (w, _) = self.logical_size();
        if w == self.layout_width && self.font_size == self.layout_font_size && self.dark == self.layout_dark {
            return;
        }
        // Keep the reading position when relayouting: remember the relative position.
        let ratio = if self.layout.height > 0.0 { self.scroll / self.layout.height } else { 0.0 };
        self.theme = if self.dark { Theme::dark() } else { Theme::light() };
        let margin = (self.font_size * 2.0).min(w * 0.06).max(12.0);
        let column = (w - 2.0 * margin).min(self.font_size * MAX_COLUMN).max(100.0);
        let x = ((w - column) / 2.0).max(margin);
        let top = if cfg!(target_os = "macos") { 28.0 + self.font_size } else { self.font_size * 1.5 };
        let started = Instant::now();
        let mut layouter = Layouter { fcx: &mut self.font_cx, lcx: &mut self.layout_cx, theme: &self.theme, base: self.font_size };
        self.layout = layouter.layout(&self.doc, x, column, top);
        eprintln!("layout: {} blocks in {:?}", self.layout.blocks.len(), started.elapsed());
        self.layout_width = w;
        self.layout_font_size = self.font_size;
        self.layout_dark = self.dark;
        self.selection = None;
        self.scroll = ratio * self.layout.height;
        self.clamp_scroll();
    }

    fn clamp_scroll(&mut self) {
        let (_, h) = self.logical_size();
        let max = (self.layout.height - h).max(0.0);
        self.scroll = self.scroll.clamp(0.0, max);
    }

    fn scroll_by(&mut self, dy: f32) {
        self.scroll += dy;
        self.clamp_scroll();
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn doc_point(&self) -> (f32, f32) {
        (self.cursor.0, self.cursor.1 + self.scroll)
    }

    fn copy_selection(&self) {
        let Some(sel) = &self.selection else { return };
        let text = self.layout.selected_text(sel);
        if text.is_empty() {
            return;
        }
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(text)) {
            Ok(()) => {}
            Err(e) => eprintln!("clipboard: {e}"),
        }
    }

    fn toggle_fullscreen(&self) {
        if let Some(gpu) = &self.gpu {
            let w = &gpu.window;
            if w.fullscreen().is_some() {
                w.set_fullscreen(None);
            } else {
                // Borderless covers the taskbar / menu bar.
                w.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    }

    fn redraw(&mut self) {
        self.ensure_layout();
        let (_, h) = self.logical_size();
        let scale = self.scale();
        self.scene.reset();
        render::paint(
            &mut self.scene,
            &self.layout,
            self.selection.as_ref(),
            &self.theme,
            &render::Viewport { scroll: self.scroll, height: h, scale },
        );
        let Some(gpu) = &mut self.gpu else { return };
        let surface = &gpu.surface;
        let (width, height) = (surface.config.width, surface.config.height);
        let device = &self.render_cx.devices[surface.dev_id];
        gpu.renderer
            .render_to_texture(
                &device.device,
                &device.queue,
                &self.scene,
                &surface.target_view,
                &vello::RenderParams {
                    base_color: self.theme.background.color(),
                    width,
                    height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .expect("render to texture");
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => {
                eprintln!("surface texture unavailable: {other:?}");
                self.render_cx.configure_surface(&gpu.surface);
                gpu.window.request_redraw();
                return;
            }
        };
        let mut encoder = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("blit") });
        surface.blitter.copy(
            &device.device,
            &mut encoder,
            &surface.target_view,
            &frame.texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device.queue.submit([encoder.finish()]);
        gpu.window.pre_present_notify();
        device.queue.present(frame);
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, key: &Key) {
        let (_, h) = self.logical_size();
        let line = self.font_size * 3.0;
        let page = (h - self.font_size * 4.0).max(line);
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();
        let shift = self.modifiers.shift_key();
        match key {
            Key::Named(NamedKey::PageDown) => self.scroll_by(page),
            Key::Named(NamedKey::PageUp) => self.scroll_by(-page),
            Key::Named(NamedKey::Space) => self.scroll_by(if shift { -page } else { page }),
            Key::Named(NamedKey::ArrowDown) => self.scroll_by(line),
            Key::Named(NamedKey::ArrowUp) => self.scroll_by(-line),
            Key::Named(NamedKey::Home) => self.scroll_by(-f32::MAX / 4.0),
            Key::Named(NamedKey::End) => self.scroll_by(f32::MAX / 4.0),
            Key::Named(NamedKey::F11) => self.toggle_fullscreen(),
            Key::Named(NamedKey::Escape) => {
                if let Some(gpu) = &self.gpu {
                    if gpu.window.fullscreen().is_some() {
                        gpu.window.set_fullscreen(None);
                    } else if self.selection.take().is_some() {
                        self.request_redraw();
                    }
                }
            }
            Key::Character(c) if ctrl => match c.as_str() {
                "c" | "C" => self.copy_selection(),
                "a" | "A" => {
                    self.selection = self.layout.select_all();
                    self.request_redraw();
                }
                "+" | "=" => self.set_font_size(self.font_size + 1.0),
                "-" => self.set_font_size(self.font_size - 1.0),
                "0" => self.set_font_size(DEFAULT_FONT_SIZE),
                "q" | "w" => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
    }

    fn set_font_size(&mut self, size: f32) {
        self.font_size = size.clamp(10.0, 40.0);
        self.request_redraw();
    }

    fn on_press(&mut self) {
        let (x, y) = self.doc_point();
        self.press_pos = self.cursor;
        let now = Instant::now();
        let count = match self.last_click {
            Some((t, p, n)) if now - t < Duration::from_millis(450) && (p.0 - x).abs() < 4.0 && (p.1 - y).abs() < 4.0 => n % 3 + 1,
            _ => 1,
        };
        self.last_click = Some((now, (x, y), count));
        let Some(pos) = self.layout.hit(x, y) else { return };
        self.selection = Some(match count {
            2 => self.layout.word_at(pos, x, y),
            3 => self.layout.block_selection(pos.block),
            _ if self.modifiers.shift_key() && self.selection.is_some() => {
                DocSelection { focus: pos, ..self.selection.unwrap() }
            }
            _ => DocSelection { anchor: pos, focus: pos },
        });
        self.dragging = count == 1;
        self.request_redraw();
    }

    fn on_release(&mut self) {
        self.dragging = false;
        let moved = (self.cursor.0 - self.press_pos.0).abs() > 3.0 || (self.cursor.1 - self.press_pos.1).abs() > 3.0;
        if moved {
            return;
        }
        let (x, y) = self.doc_point();
        let Some(pos) = self.layout.hit(x, y) else { return };
        if let Some(url) = self.layout.link_at(pos).map(str::to_owned) {
            self.follow_link(&url);
        }
    }

    fn follow_link(&mut self, url: &str) {
        if let Some(id) = url.strip_prefix('#') {
            if let Some(entry) = self.layout.toc.iter().find(|e| e.id == id) {
                self.scroll = entry.y - self.font_size;
                self.clamp_scroll();
                self.selection = None;
                self.request_redraw();
            }
        } else {
            // External / relative links are handled in a later step (open in browser,
            // relative .md navigation like the iOS app).
            eprintln!("link: {url}");
        }
    }

    fn on_cursor_moved(&mut self) {
        let (x, y) = self.doc_point();
        if self.dragging {
            if let (Some(sel), Some(pos)) = (self.selection.as_mut(), self.layout.hit(x, y)) {
                sel.focus = pos;
            }
            // Auto-scroll when dragging beyond the window edge.
            let (_, h) = self.logical_size();
            if self.cursor.1 < 0.0 {
                self.scroll_by(self.cursor.1);
            } else if self.cursor.1 > h {
                self.scroll_by(self.cursor.1 - h);
            }
            self.request_redraw();
        }
        if let Some(gpu) = &self.gpu {
            let over_link = self.layout.hit(x, y).is_some_and(|p| {
                let b = &self.layout.blocks[p.block];
                y >= b.y && y <= b.y + b.height() && self.layout.link_at(p).is_some()
            });
            gpu.window.set_cursor(if over_link { CursorIcon::Pointer } else { CursorIcon::Text });
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(format!("{} – Markdownr", self.source_name))
            .with_inner_size(LogicalSize::new(980.0, 820.0))
            .with_min_inner_size(LogicalSize::new(320.0, 240.0));
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            // Content runs into the title bar area.
            attrs = attrs.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true);
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        self.dark = matches!(window.theme(), Some(winit::window::Theme::Dark));
        let size = window.inner_size();
        let surface = pollster::block_on(self.render_cx.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("create surface");
        let renderer = Renderer::new(
            &self.render_cx.devices[surface.dev_id].device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .expect("create renderer");
        window.set_cursor(CursorIcon::Text);
        self.gpu = Some(Gpu { window, surface, renderer });
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    if size.width > 0 && size.height > 0 {
                        self.render_cx.resize_surface(&mut gpu.surface, size.width, size.height);
                    }
                }
                self.clamp_scroll();
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => self.request_redraw(),
            WindowEvent::ThemeChanged(theme) => {
                self.dark = theme == winit::window::Theme::Dark;
                self.request_redraw();
            }
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                self.on_key(event_loop, &event.logical_key)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * self.font_size * 3.0,
                    MouseScrollDelta::PixelDelta(p) => -(p.y / self.scale()) as f32,
                };
                if self.modifiers.control_key() {
                    self.set_font_size(self.font_size + if dy < 0.0 { 1.0 } else { -1.0 });
                } else {
                    self.scroll_by(dy);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(self.scale());
                self.cursor = (p.x, p.y);
                self.on_cursor_moved();
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => self.on_press(),
                ElementState::Released => self.on_release(),
            },
            WindowEvent::DroppedFile(path) => self.load_file(path),
            _ => {}
        }
    }
}

fn parse(text: &str) -> ast::Node {
    let mut doc = ast::parse(text);
    transforms::compose(&mut doc, &transforms::default_pipeline());
    doc
}

fn main() {
    // Some older Windows Vulkan drivers (e.g. Intel HD 520) crash inside wgpu's
    // Vulkan backend; DX12 is the native, reliable choice on Windows.
    #[cfg(windows)]
    if std::env::var_os("WGPU_BACKEND").is_none() {
        // SAFETY: called before any other thread is started.
        unsafe { std::env::set_var("WGPU_BACKEND", "dx12") };
    }
    let (name, text) = match std::env::args_os().nth(1).map(PathBuf::from) {
        Some(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| format!("# Cannot open file\n\n`{}`: {e}", path.display()));
            (path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), text)
        }
        None => ("Welcome".to_string(), include_str!("../assets/sample.md").to_string()),
    };
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(name, &text);
    event_loop.run_app(&mut app).expect("run app");
}
