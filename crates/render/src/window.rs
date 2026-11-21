//! Window-surface renderer.
#![cfg_attr(
    not(any(feature = "gpu", feature = "soft-window")),
    allow(unused_variables)
)]

use alloy_ui_core::{Color, DrawList};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;

use crate::error::RenderError;
#[cfg(feature = "gpu")]
use crate::gpu::{request_adapter, GpuContext, Painter};
use crate::image::Image;
use crate::Renderer;

/// Renders to an OS window surface.
///
/// With the `gpu` feature (default) it draws through wgpu; when no adapter is usable, or
/// `ALLOY_RENDERER=cpu` is set, it falls back to the software rasteriser presented through
/// the `soft-window` feature. A build with only `soft-window` has no wgpu at all.
pub struct WindowRenderer(Inner);

enum Inner {
    #[cfg(feature = "gpu")]
    Gpu(Box<GpuWindow>),
    #[cfg(feature = "soft-window")]
    Soft(Box<crate::soft::SoftWindow>),
    /// Never built: keeps the enum inhabited-in-name when both backends are compiled out, so a
    /// plugin crate can still type-check against `alloy-ui` with `default-features = false`.
    #[allow(dead_code)]
    Never(std::convert::Infallible),
}

/// Which backend a [`WindowRenderer`] ended up on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// wgpu on a real or software adapter.
    Gpu,
    /// The CPU rasteriser, blitted to the window.
    Cpu,
}

impl WindowRenderer {
    /// Create a renderer for `window`. `width`/`height` are physical pixels.
    ///
    /// `ALLOY_RENDERER=cpu` forces the software path, `=gpu` forbids the fallback.
    pub fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, RenderError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        #[cfg(any(feature = "gpu", feature = "soft-window"))]
        let want = std::env::var("ALLOY_RENDERER").unwrap_or_default();
        #[cfg(feature = "gpu")]
        let gpu_err = if want == "cpu" && cfg!(feature = "soft-window") {
            RenderError::NoAdapter("disabled by ALLOY_RENDERER=cpu".into())
        } else {
            match GpuWindow::new(window.clone(), width, height) {
                Ok(g) => return Ok(Self(Inner::Gpu(Box::new(g)))),
                Err(e) => e,
            }
        };
        #[cfg(not(feature = "gpu"))]
        let gpu_err = RenderError::NoAdapter("built without the `gpu` feature".into());
        #[cfg(feature = "soft-window")]
        if want != "gpu" {
            if cfg!(feature = "gpu") {
                log::warn!("GPU renderer unavailable ({gpu_err}); using the CPU renderer");
            }
            return crate::soft::SoftWindow::new(window, width, height)
                .map(|s| Self(Inner::Soft(Box::new(s))));
        }
        let _ = (&window, width, height);
        Err(gpu_err)
    }

    /// Which backend is drawing.
    pub fn backend(&self) -> Backend {
        match &self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(_) => Backend::Gpu,
            #[cfg(feature = "soft-window")]
            Inner::Soft(_) => Backend::Cpu,
            Inner::Never(n) => match *n {},
        }
    }

    /// A measurement handle sharing this renderer's fonts.
    pub fn text_measurer(&self) -> crate::TextMeasurer {
        match &self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.painter.text.measurer(),
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.text_measurer(),
            Inner::Never(n) => match *n {},
        }
    }

    /// Reconfigure the surface after a window resize (physical pixels).
    pub fn resize(&mut self, width: u32, height: u32) {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.resize(width, height),
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.resize(width, height),
            Inner::Never(n) => match *n {},
        }
    }

    /// Let the window show what is behind it where the frame is transparent
    /// (`true`), or make it opaque again (`false`). Returns whether the
    /// surface can do it: with `true` and `false` returned the window stays
    /// opaque, so the caller should not rely on see-through colours. The CPU
    /// path presents opaque pixels, so it always answers `false`.
    pub fn set_transparent(&mut self, on: bool) -> bool {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.set_transparent(on),
            #[cfg(feature = "soft-window")]
            Inner::Soft(_) => false,
            Inner::Never(n) => match *n {},
        }
    }

    /// True when the surface is currently composited with the desktop.
    pub fn is_transparent(&self) -> bool {
        match &self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.is_transparent(),
            #[cfg(feature = "soft-window")]
            Inner::Soft(_) => false,
            Inner::Never(n) => match *n {},
        }
    }

    /// Set the clear colour.
    pub fn set_background(&mut self, color: Color) {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.background = color,
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.set_background(color),
            Inner::Never(n) => match *n {},
        }
    }
}

impl Renderer for WindowRenderer {
    fn render(&mut self, list: &DrawList, scale_factor: f32) {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.render(list, scale_factor),
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.render(list, scale_factor),
            Inner::Never(n) => match *n {},
        }
    }
    fn register_image(&mut self, image: &Image) -> u64 {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.painter.register_image(image),
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.register_image(image),
            Inner::Never(n) => match *n {},
        }
    }
    fn unregister_image(&mut self, id: u64) {
        match &mut self.0 {
            #[cfg(feature = "gpu")]
            Inner::Gpu(g) => g.painter.unregister_image(id),
            #[cfg(feature = "soft-window")]
            Inner::Soft(s) => s.unregister_image(id),
            Inner::Never(n) => match *n {},
        }
    }
}

#[cfg(feature = "gpu")]
struct GpuWindow {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ctx: GpuContext,
    painter: Painter,
    background: Color,
}

#[cfg(feature = "gpu")]
impl GpuWindow {
    fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, RenderError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let mut t = std::time::Instant::now();
        let instance = crate::take_instance();
        crate::perf_mark("gpu.instance(wait)", &mut t);
        let surface = instance
            .create_surface(window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        crate::perf_mark("gpu.surface", &mut t);
        Self::finish(instance, surface, width, height)
    }

    fn finish(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let mut t = std::time::Instant::now();
        let adapter = request_adapter(&instance, Some(&surface), false)
            .or_else(|| request_adapter(&instance, Some(&surface), true))
            .ok_or_else(|| RenderError::NoAdapter("none compatible with the surface".into()))?;
        crate::perf_mark("gpu.adapter", &mut t);
        let ctx = GpuContext::from_adapter(instance, adapter)?;
        crate::perf_mark("gpu.device", &mut t);
        let caps = surface.get_capabilities(&ctx.adapter);
        // Prefer a non-sRGB format so colours match the CPU reference.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| RenderError::Surface("surface reports no formats".into()))?;
        if format.is_srgb() {
            log::warn!("surface only offers sRGB formats; colours will appear lighter");
        }
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|m| *m == wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(caps.alpha_modes[0]);
        let config = wgpu::SurfaceConfiguration {
            // COPY_SRC (when offered) lets backdrop blur read what is drawn so far.
            usage: if caps.usages.contains(wgpu::TextureUsages::COPY_SRC) {
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
            } else {
                wgpu::TextureUsages::RENDER_ATTACHMENT
            },
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&ctx.device, &config);
        crate::perf_mark("gpu.surface-config", &mut t);
        let text = crate::take_text_system();
        crate::perf_mark("text-system", &mut t);
        let painter = Painter::new(ctx.device.clone(), ctx.queue.clone(), format, text);
        crate::perf_mark("gpu.pipeline", &mut t);
        Ok(Self {
            surface,
            config,
            ctx,
            painter,
            background: Color::hex(0xFFFFFF),
        })
    }

    /// Reconfigure the surface after a window resize (physical pixels).
    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.ctx.device, &self.config);
    }

    /// Let the window show what is behind it where the frame is transparent
    /// (`true`), or make it opaque again (`false`). Returns whether the
    /// surface can do it: with `true` and `false` returned the window stays
    /// opaque, so the caller should not rely on see-through colours.
    pub fn set_transparent(&mut self, on: bool) -> bool {
        use wgpu::CompositeAlphaMode as M;
        let caps = self.surface.get_capabilities(&self.ctx.adapter);
        let want = if on {
            // Premultiplied is what the painter produces; post-multiplied and
            // inherit (the OS decides) are the next best.
            [M::PreMultiplied, M::PostMultiplied, M::Inherit]
                .into_iter()
                .find(|m| caps.alpha_modes.contains(m))
        } else {
            Some(M::Opaque).filter(|m| caps.alpha_modes.contains(m))
        };
        match want {
            Some(m) => {
                self.config.alpha_mode = m;
                self.surface.configure(&self.ctx.device, &self.config);
                true
            }
            None => false,
        }
    }

    /// True when the surface is currently composited with the desktop.
    pub fn is_transparent(&self) -> bool {
        self.config.alpha_mode != wgpu::CompositeAlphaMode::Opaque
    }

    fn render(&mut self, list: &DrawList, scale_factor: f32) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.ctx.device, &self.config);
                match self.surface.get_current_texture() {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("skipping frame: {e}");
                        return;
                    }
                }
            }
            Err(e) => {
                log::warn!("skipping frame: {e}");
                return;
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let can_copy = self.config.usage.contains(wgpu::TextureUsages::COPY_SRC);
        self.painter.paint(
            &view,
            can_copy.then_some(&frame.texture),
            self.config.width,
            self.config.height,
            list,
            scale_factor,
            self.background,
        );
        frame.present();
    }
}
