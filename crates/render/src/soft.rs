//! Software window presenter: the CPU rasteriser blitted to an OS window through softbuffer.
//! Used when there is no GPU adapter, or as the whole renderer in a build without `gpu`.

use std::num::NonZeroU32;
use std::sync::Arc;

use alloy_ui_core::{Color, DrawList};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::error::RenderError;
use crate::image::Image;
use crate::{CpuRenderer, HeadlessRenderer, Renderer};

/// Type-erased softbuffer surface, so `SoftWindow` does not carry the window type.
trait Blit {
    fn blit(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<(), String>;
}

struct Surf<W: HasWindowHandle + HasDisplayHandle> {
    surface: softbuffer::Surface<Arc<W>, Arc<W>>,
}

impl<W: HasWindowHandle + HasDisplayHandle> Blit for Surf<W> {
    fn blit(&mut self, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
        let (Some(nw), Some(nh)) = (NonZeroU32::new(w), NonZeroU32::new(h)) else {
            return Ok(());
        };
        self.surface.resize(nw, nh).map_err(|e| e.to_string())?;
        let mut buf = self.surface.buffer_mut().map_err(|e| e.to_string())?;
        // softbuffer wants 0x00RRGGBB; the image is straight RGBA over an opaque background.
        for (dst, px) in buf.iter_mut().zip(rgba.chunks_exact(4)) {
            *dst = (px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32;
        }
        buf.present().map_err(|e| e.to_string())
    }
}

pub(crate) struct SoftWindow {
    cpu: CpuRenderer,
    surface: Box<dyn Blit>,
    width: u32,
    height: u32,
}

impl SoftWindow {
    pub(crate) fn new<W>(window: Arc<W>, width: u32, height: u32) -> Result<Self, RenderError>
    where
        W: HasWindowHandle + HasDisplayHandle + Send + Sync + 'static,
    {
        let ctx = softbuffer::Context::new(window.clone())
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let surface = softbuffer::Surface::new(&ctx, window)
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        Ok(Self {
            cpu: CpuRenderer::with_text_system(
                width.max(1),
                height.max(1),
                crate::take_text_system(),
            ),
            surface: Box::new(Surf { surface }),
            width: width.max(1),
            height: height.max(1),
        })
    }
    pub(crate) fn text_measurer(&self) -> crate::TextMeasurer {
        self.cpu.text_measurer()
    }
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
    }
    pub(crate) fn set_background(&mut self, color: Color) {
        self.cpu.set_background(color);
    }
    pub(crate) fn render(&mut self, list: &DrawList, scale_factor: f32) {
        let img = self
            .cpu
            .render_to_image(list, self.width, self.height, scale_factor);
        if let Err(e) = self.surface.blit(img.width, img.height, &img.rgba) {
            log::warn!("skipping frame: {e}");
        }
    }
    pub(crate) fn register_image(&mut self, image: &Image) -> u64 {
        self.cpu.register_image(image)
    }
    pub(crate) fn unregister_image(&mut self, id: u64) {
        self.cpu.unregister_image(id);
    }
}
