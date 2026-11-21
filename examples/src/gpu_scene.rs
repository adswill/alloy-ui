//! A real wgpu scene inside an Alloy window: a fragment-shader plasma that you steer by
//! dragging. The scene renders into a texture shared with the window renderer, so nothing
//! is copied or read back. Needs the `gpu` feature and a GPU (or software Vulkan) adapter;
//! on the CPU fallback renderer it shows a note instead.
use alloy_ui::prelude::*;
use alloy_ui::render::{shared_gpu, wgpu, SharedGpu};
use alloy_ui::GpuInput;
use std::{cell::RefCell, sync::Arc, time::Instant};

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (640.0, 520.0);

const SHADER: &str = r#"
struct U { time: f32, yaw: f32, pitch: f32, _p: f32 };
@group(0) @binding(0) var<uniform> u: U;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let uv = (pos.xy / 400.0) * 3.0 + vec2<f32>(u.yaw, u.pitch);
    let t = u.time;
    var v = sin(uv.x + t) + sin((uv.y + t) * 0.7) + sin((uv.x + uv.y + t) * 0.5);
    v = v + sin(length(uv - vec2<f32>(3.0, 3.0)) * 1.5 - t * 1.3);
    let c = 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.1, 4.2) + v * 1.2);
    return vec4<f32>(c * vec3<f32>(0.35, 0.8, 1.0), 1.0);
}
"#;

const TEX: (u32, u32) = (640, 400);

struct Scene {
    gpu: Arc<SharedGpu>,
    texture: wgpu::Texture,
    id: u64,
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind: wgpu::BindGroup,
    start: Instant,
}

thread_local! {
    static SCENE: RefCell<Option<Option<Scene>>> = const { RefCell::new(None) };
}

fn build() -> Option<Scene> {
    let gpu = shared_gpu()?;
    let (texture, id) = gpu.create_target(TEX.0, TEX.1);
    let d = &gpu.device;
    let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("plasma"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let uniform = d.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 16,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: None,
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::TextureFormat::Rgba8Unorm.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });
    let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    Some(Scene {
        gpu,
        texture,
        id,
        pipeline,
        uniform,
        bind,
        start: Instant::now(),
    })
}

impl Scene {
    fn render(&self, yaw: f32, pitch: f32) {
        let u = [self.start.elapsed().as_secs_f32(), yaw, pitch, 0.0f32];
        let bytes: Vec<u8> = u.iter().flat_map(|f| f.to_ne_bytes()).collect();
        self.gpu.queue.write_buffer(&self.uniform, 0, &bytes);
        let view = self.texture.create_view(&Default::default());
        let mut enc = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        self.gpu.queue.submit([enc.finish()]);
    }
}

/// With the scene's texture id (making the scene on first use), or `None` without a GPU.
fn scene_id() -> Option<u64> {
    SCENE.with(|s| {
        let mut s = s.borrow_mut();
        s.get_or_insert_with(build).as_ref().map(|sc| sc.id)
    })
}

#[derive(Default)]
struct State {
    yaw: f32,
    pitch: f32,
}

/// The app.
pub fn app() -> impl App {
    sketch(State::default(), |s| {
        let (yaw, pitch) = (s.yaw, s.pitch);
        let scene: Node<_> = match scene_id() {
            Some(id) => gpu_view(id)
                .height(400.0)
                .live()
                .paint(move |_| {
                    SCENE.with(|sc| {
                        if let Some(Some(sc)) = sc.borrow().as_ref() {
                            sc.render(yaw, pitch);
                        }
                    })
                })
                .act(|s: &mut State, i| {
                    if let GpuInput::Drag { delta, .. } = i {
                        s.yaw -= delta.x * 0.02;
                        s.pitch -= delta.y * 0.02;
                    }
                })
                .into(),
            None => text("No GPU adapter: this view needs the wgpu renderer.").into(),
        };
        col![
            col![
                heading("wgpu viewport"),
                text("Your own wgpu shader, drawn straight into the window. Drag to steer.").dim()
            ]
            .gap(2.0),
            scene,
        ]
        .gap(14.0)
        .pad(20.0)
        .fill()
    })
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("wgpu viewport", SIZE.0, SIZE.1)
}
