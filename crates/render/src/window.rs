//! Window-surface renderer.

use alloy_ui_core::{Color, DrawList};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

use crate::error::RenderError;
use crate::gpu::{request_adapter, GpuContext, Painter};
use crate::image::Image;
use crate::text::TextSystem;
use crate::Renderer;

/// Renders to an OS window surface.
pub struct WindowRenderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ctx: GpuContext,
    painter: Painter,
    background: Color,
}

impl WindowRenderer {
    /// A measurement handle sharing this renderer's fonts.
    pub fn text_measurer(&self) -> crate::TextMeasurer {
        self.painter.text.measurer()
    }

    /// Create a renderer for a window. `window` is anything wgpu accepts as a
    /// surface target, typically an `Arc<W>` where `W` implements
    /// `HasWindowHandle + HasDisplayHandle`. `width`/`height` are physical pixels.
    pub fn new(
        window: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        Self::finish(instance, surface, width, height)
    }

    /// Create a renderer from raw window and display handles.
    ///
    /// # Safety
    /// The handles must be valid for the lifetime of the returned renderer:
    /// the window must outlive it and must not be destroyed before it is
    /// dropped. Must be called on the thread the platform requires for the
    /// window system (the main thread on macOS).
    pub unsafe fn from_raw_handles(
        display: RawDisplayHandle,
        window: RawWindowHandle,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        // SAFETY: upheld by the caller per this function's contract.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: display,
                raw_window_handle: window,
            })
        }
        .map_err(|e| RenderError::Surface(e.to_string()))?;
        Self::finish(instance, surface, width, height)
    }

    /// Convenience for callers holding a borrowed handle provider.
    ///
    /// # Safety
    /// `window` must outlive the returned renderer.
    pub unsafe fn from_handle_provider<W: HasWindowHandle + HasDisplayHandle>(
        window: &W,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let d = window
            .display_handle()
            .map_err(|e| RenderError::Surface(e.to_string()))?
            .as_raw();
        let w = window
            .window_handle()
            .map_err(|e| RenderError::Surface(e.to_string()))?
            .as_raw();
        // SAFETY: forwarded caller contract.
        unsafe { Self::from_raw_handles(d, w, width, height) }
    }

    fn finish(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<Self, RenderError> {
        let adapter = request_adapter(&instance, Some(&surface), false)
            .or_else(|| request_adapter(&instance, Some(&surface), true))
            .ok_or_else(|| RenderError::NoAdapter("none compatible with the surface".into()))?;
        let ctx = GpuContext::from_adapter(instance, adapter)?;
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };
        surface.configure(&ctx.device, &config);
        let painter = Painter::new(
            ctx.device.clone(),
            ctx.queue.clone(),
            format,
            TextSystem::new(),
        );
        Ok(Self {
            surface,
            config,
            ctx,
            painter,
            background: Color::hex(0xFFFFFF),
        })
    }

    /// Reconfigure the surface after a window resize (physical pixels).
    pub fn resize(&mut self, width: u32, height: u32) {
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

    /// Set the clear colour.
    pub fn set_background(&mut self, color: Color) {
        self.background = color;
    }
}

impl Renderer for WindowRenderer {
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
        self.painter.paint(
            &view,
            self.config.width,
            self.config.height,
            list,
            scale_factor,
            self.background,
        );
        frame.present();
    }

    fn register_image(&mut self, image: &Image) -> u64 {
        self.painter.register_image(image)
    }

    fn unregister_image(&mut self, id: u64) {
        self.painter.unregister_image(id);
    }
}
