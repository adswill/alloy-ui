//! Offscreen GPU renderer (software adapter friendly).

use alloy_ui_core::{Color, DrawList};

use crate::error::RenderError;
use crate::gpu::{GpuContext, Painter, OFFSCREEN_FORMAT};
use crate::image::Image;
use crate::text::TextSystem;
use crate::{HeadlessRenderer, Renderer};

/// Renders draw lists to an RGBA [`Image`] through wgpu, preferring a software
/// adapter so output is reproducible on CI.
pub struct GpuHeadlessRenderer {
    ctx: GpuContext,
    painter: Painter,
    width: u32,
    height: u32,
    background: Color,
    last: Option<Image>,
}

impl GpuHeadlessRenderer {
    /// A measurement handle sharing this renderer's fonts.
    pub fn text_measurer(&self) -> crate::TextMeasurer {
        self.painter.text.measurer()
    }

    /// Create a renderer, or an error if no adapter is available.
    pub fn try_new(width: u32, height: u32) -> Result<Self, RenderError> {
        Self::try_with_text_system(width, height, TextSystem::new())
    }

    /// As [`GpuHeadlessRenderer::try_new`] with a caller-provided text system.
    pub fn try_with_text_system(
        width: u32,
        height: u32,
        text: TextSystem,
    ) -> Result<Self, RenderError> {
        let ctx = GpuContext::new_headless()?;
        let painter = Painter::new(
            ctx.device.clone(),
            ctx.queue.clone(),
            OFFSCREEN_FORMAT,
            text,
        );
        Ok(Self {
            ctx,
            painter,
            width,
            height,
            background: Color::hex(0xFFFFFF),
            last: None,
        })
    }

    /// The shared-texture registry of this renderer's device, for rendering your own wgpu
    /// scene into a texture that this renderer can draw.
    pub fn shared_gpu(&self) -> std::sync::Arc<crate::shared::SharedGpu> {
        self.painter.shared()
    }

    /// Set the colour the target is cleared to before drawing.
    pub fn set_background(&mut self, color: Color) {
        self.background = color;
    }

    /// Description of the adapter in use, for logs.
    pub fn adapter_name(&self) -> String {
        let i = self.ctx.adapter.get_info();
        format!("{} ({:?}, {:?})", i.name, i.device_type, i.backend)
    }

    /// The image produced by the most recent [`Renderer::render`] call.
    pub fn last_image(&self) -> Option<&Image> {
        self.last.as_ref()
    }
}

impl Renderer for GpuHeadlessRenderer {
    fn render(&mut self, list: &DrawList, scale_factor: f32) {
        let img = self.render_to_image(list, self.width, self.height, scale_factor);
        self.last = Some(img);
    }

    fn register_image(&mut self, image: &Image) -> u64 {
        self.painter.register_image(image)
    }

    fn unregister_image(&mut self, id: u64) {
        self.painter.unregister_image(id);
    }
}

impl HeadlessRenderer for GpuHeadlessRenderer {
    fn render_to_image(
        &mut self,
        list: &DrawList,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Image {
        let (width, height) = (width.max(1), height.max(1));
        let device = &self.ctx.device;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        self.painter.paint(
            &view,
            Some(&target),
            width,
            height,
            list,
            scale_factor,
            self.background,
        );

        let row = width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: padded as u64 * height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("readback"),
        });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.ctx.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = device.poll(wgpu::PollType::Wait);
        rx.recv()
            .expect("map callback dropped")
            .expect("readback map failed");
        let data = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity(row as usize * height as usize);
        for y in 0..height as usize {
            let line = &data[y * padded as usize..y * padded as usize + row as usize];
            for px in line.chunks_exact(4) {
                // Target holds premultiplied colour; convert to straight alpha.
                let a = px[3] as u32;
                if a == 0 || a == 255 {
                    rgba.extend_from_slice(px);
                } else {
                    let un = |c: u8| ((c as u32 * 255 + a / 2) / a).min(255) as u8;
                    rgba.extend_from_slice(&[un(px[0]), un(px[1]), un(px[2]), px[3]]);
                }
            }
        }
        drop(data);
        buf.unmap();
        Image {
            width,
            height,
            rgba,
        }
    }
}
