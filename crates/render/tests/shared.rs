//! Sharing an app-owned wgpu texture with the renderer: no copy, live updates.
#![cfg(feature = "gpu")]

use alloy_ui_core::{DrawCommand, DrawList, Point, Rectangle, Size};
use alloy_ui_render::testing::gpu_or_skip;
use alloy_ui_render::{Renderer, ShareError};

fn clear(gpu: &alloy_ui_render::SharedGpu, tex: &wgpu::Texture, c: [f64; 4]) {
    use wgpu as w;
    let view = tex.create_view(&w::TextureViewDescriptor::default());
    let mut enc = gpu
        .device
        .create_command_encoder(&w::CommandEncoderDescriptor::default());
    drop(enc.begin_render_pass(&w::RenderPassDescriptor {
        label: None,
        color_attachments: &[Some(w::RenderPassColorAttachment {
            view: &view,
            resolve_target: None,
            ops: w::Operations {
                load: w::LoadOp::Clear(w::Color {
                    r: c[0],
                    g: c[1],
                    b: c[2],
                    a: c[3],
                }),
                store: w::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    }));
    gpu.queue.submit([enc.finish()]);
}

fn list(id: u64) -> DrawList {
    let mut l = DrawList::default();
    l.push(DrawCommand::Image {
        bounds: Rectangle::new(Point::new(4.0, 4.0), Size::new(8.0, 8.0)),
        id,
    });
    l
}

fn px(img: &alloy_ui_render::Image, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * img.width + x) * 4) as usize;
    [
        img.rgba[i],
        img.rgba[i + 1],
        img.rgba[i + 2],
        img.rgba[i + 3],
    ]
}

#[test]
fn shared_texture_is_sampled_live() {
    let Some(mut r) = gpu_or_skip(16, 16) else {
        return;
    };
    let gpu = r.shared_gpu();
    let (tex, id) = gpu.create_target(32, 32);
    assert!(id >= alloy_ui_render::SHARED_ID_BASE);
    assert_eq!(gpu.size_of(id), Some((32, 32)));

    clear(&gpu, &tex, [1.0, 0.0, 0.0, 1.0]);
    r.render(&list(id), 1.0);
    assert_eq!(px(r.last_image().unwrap(), 8, 8), [255, 0, 0, 255]);
    assert_eq!(px(r.last_image().unwrap(), 0, 0), [255, 255, 255, 255]);

    // The app draws again; the same id shows the new frame with no re-registration.
    clear(&gpu, &tex, [0.0, 0.0, 1.0, 1.0]);
    r.render(&list(id), 1.0);
    assert_eq!(px(r.last_image().unwrap(), 8, 8), [0, 0, 255, 255]);

    gpu.unpublish(id);
    r.render(&list(id), 1.0);
    assert_eq!(px(r.last_image().unwrap(), 8, 8), [255, 255, 255, 255]);
}

#[test]
fn rejects_unsuitable_textures() {
    use wgpu as w;
    let Some(r) = gpu_or_skip(8, 8) else { return };
    let gpu = r.shared_gpu();
    let make = |format, usage| {
        gpu.device.create_texture(&w::TextureDescriptor {
            label: None,
            size: w::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: w::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let srgb = make(
        w::TextureFormat::Rgba8UnormSrgb,
        w::TextureUsages::TEXTURE_BINDING,
    );
    assert!(matches!(gpu.publish(&srgb), Err(ShareError::Format(_))));
    let nobind = make(w::TextureFormat::Rgba8Unorm, w::TextureUsages::COPY_DST);
    assert_eq!(gpu.publish(&nobind), Err(ShareError::Usage));
}

// ---- streamed pixel surfaces (every renderer)

#[test]
fn pixel_surfaces_draw_on_cpu_and_gpu_and_update_live() {
    use alloy_ui_render::{CpuRenderer, HeadlessRenderer, PixelSurface};
    let s = PixelSurface::new(2, 2);
    s.update_opaque(2, 2, &[255, 0, 0, 255].repeat(4));
    let l = list(s.id());
    let cpu = CpuRenderer::new(16, 16).render_to_image(&l, 16, 16, 1.0);
    assert_eq!(px(&cpu, 8, 8), [255, 0, 0, 255]);
    assert_eq!(px(&cpu, 0, 0), [255, 255, 255, 255]);
    let v0 = s.version();

    let gpu = gpu_or_skip(16, 16);
    let gpu_ran = gpu.is_some();
    if let Some(mut g) = gpu {
        g.render(&l, 1.0);
        assert_eq!(px(g.last_image().unwrap(), 8, 8), [255, 0, 0, 255]);
        // A new frame, same size: uploaded in place.
        s.update_opaque(2, 2, &[0, 200, 0, 255].repeat(4));
        g.render(&l, 1.0);
        assert_eq!(px(g.last_image().unwrap(), 8, 8), [0, 200, 0, 255]);
        // A different size: a fresh texture.
        s.update_opaque(1, 1, &[0, 0, 255, 255]);
        g.render(&l, 1.0);
        assert_eq!(px(g.last_image().unwrap(), 8, 8), [0, 0, 255, 255]);
    }
    // Only the GPU half uploads, so the version moves only where an adapter was found.
    if gpu_ran {
        assert!(s.version() > v0);
    }
    let id = s.id();
    drop(s);
    let gone = CpuRenderer::new(16, 16).render_to_image(&list(id), 16, 16, 1.0);
    assert_eq!(
        px(&gone, 8, 8),
        [255, 255, 255, 255],
        "dropped surface draws nothing"
    );
}
