//! wgpu back end: pipeline, glyph atlas, image store and frame encoding.

use std::collections::HashMap;

use alloy_ui_core::{Color, DrawList};
use bytemuck::{Pod, Zeroable};
use cosmic_text::CacheKey;

use crate::error::RenderError;
use crate::image::Image;
use crate::scene::{flatten, Kind, Prim, Source};
use crate::text::TextSystem;

pub(crate) const SHADER_SRC: &str = include_str!("shader.wgsl");
const ATLAS_SIZE: u32 = 2048;
/// Offscreen / window colour format. Non-sRGB so blending matches the CPU
/// reference (colours are blended as encoded values).
pub(crate) const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// An instance of the shader's primitive; layout matches `Instance` in WGSL.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    rect: [f32; 4],
    color: [f32; 4],
    border_color: [f32; 4],
    uv: [f32; 4],
    params: [f32; 4],
    meta: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    _pad: [f32; 2],
}

/// A device plus queue, optionally with the adapter description for logging.
#[derive(Clone)]
pub struct GpuContext {
    /// wgpu instance.
    pub instance: wgpu::Instance,
    /// Selected adapter.
    pub adapter: wgpu::Adapter,
    /// Logical device.
    pub device: wgpu::Device,
    /// Submission queue.
    pub queue: wgpu::Queue,
}

impl GpuContext {
    /// Create a context for offscreen work. Prefers a software (fallback)
    /// adapter so results are reproducible, then any adapter.
    pub fn new_headless() -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = request_adapter(&instance, None, true)
            .or_else(|| request_adapter(&instance, None, false))
            .ok_or_else(|| RenderError::NoAdapter("no fallback or hardware adapter".into()))?;
        Self::from_adapter(instance, adapter)
    }

    pub(crate) fn from_adapter(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
    ) -> Result<Self, RenderError> {
        let info = adapter.get_info();
        log::info!(
            "alloy-ui-render: using adapter {:?} ({:?}, {:?})",
            info.name,
            info.device_type,
            info.backend
        );
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("alloy-ui device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| RenderError::Device(e.to_string()))?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
        })
    }
}

pub(crate) fn request_adapter(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
    fallback: bool,
) -> Option<wgpu::Adapter> {
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        force_fallback_adapter: fallback,
        compatible_surface: surface,
    }))
    .ok()
}

struct Atlas {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    entries: HashMap<CacheKey, [f32; 4]>,
    cursor: (u32, u32),
    row_h: u32,
}

struct GpuImage {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Atlas,
    Image(u64),
    /// Copy of the target taken for a backdrop blur.
    Snap,
    /// Horizontally blurred scratch of a backdrop blur.
    Tmp,
}

/// Batches that need a pass of their own (backdrop blur).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Special {
    /// Blur the target copy horizontally into the scratch texture.
    BlurH,
    /// Blur the scratch vertically back into the target, under the tint.
    BlurV,
}

struct Batch {
    clip: [i32; 4],
    binding: Binding,
    range: std::ops::Range<u32>,
    special: Option<Special>,
}

/// Full-size textures a backdrop blur works through.
struct Scratch {
    w: u32,
    h: u32,
    snap: wgpu::Texture,
    tmp: wgpu::Texture,
    snap_bg: wgpu::BindGroup,
    tmp_bg: wgpu::BindGroup,
}

/// Encodes draw lists into render passes against any colour target.
pub(crate) struct Painter {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_buf: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instances: wgpu::Buffer,
    instance_cap: u64,
    format: wgpu::TextureFormat,
    scratch: Option<Scratch>,
    atlas: Atlas,
    images: HashMap<u64, GpuImage>,
    next_id: u64,
    pub(crate) text: TextSystem,
}

impl Painter {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        text: TextSystem,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("alloy-ui shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("alloy-ui pipeline layout"),
            bind_group_layouts: &[&globals_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![
            0 => Float32x4, 1 => Float32x4, 2 => Float32x4,
            3 => Float32x4, 4 => Float32x4, 5 => Float32x4
        ];
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("alloy-ui pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("alloy-ui sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let atlas_tex = make_texture(&device, ATLAS_SIZE, ATLAS_SIZE, "glyph atlas");
        let atlas = Atlas {
            bind_group: make_bind_group(&device, &tex_layout, &atlas_tex, &sampler),
            texture: atlas_tex,
            entries: HashMap::new(),
            cursor: (0, 0),
            row_h: 0,
        };
        let instance_cap = 1024;
        let instances = make_instance_buffer(&device, instance_cap);
        Self {
            device,
            queue,
            pipeline,
            globals_buf,
            globals_bg,
            tex_layout,
            sampler,
            instances,
            instance_cap,
            format,
            scratch: None,
            atlas,
            images: HashMap::new(),
            next_id: 1,
            text,
        }
    }

    pub(crate) fn register_image(&mut self, image: &Image) -> u64 {
        let tex = make_texture(
            &self.device,
            image.width.max(1),
            image.height.max(1),
            "image",
        );
        upload(
            &self.queue,
            &tex,
            [0, 0],
            image.width,
            image.height,
            &image.premultiplied(),
        );
        let bind_group = make_bind_group(&self.device, &self.tex_layout, &tex, &self.sampler);
        let id = self.next_id;
        self.next_id += 1;
        self.images.insert(
            id,
            GpuImage {
                texture: tex,
                bind_group,
            },
        );
        id
    }

    pub(crate) fn unregister_image(&mut self, id: u64) {
        if let Some(img) = self.images.remove(&id) {
            img.texture.destroy();
        }
    }

    fn atlas_slot(&mut self, key: CacheKey) -> Option<[f32; 4]> {
        if let Some(uv) = self.atlas.entries.get(&key) {
            return Some(*uv);
        }
        let bm = self.text.bitmap(key)?;
        let (w, h) = (bm.width, bm.height);
        let a = &mut self.atlas;
        if a.cursor.0 + w + 1 > ATLAS_SIZE {
            a.cursor = (0, a.cursor.1 + a.row_h + 1);
            a.row_h = 0;
        }
        if w + 1 > ATLAS_SIZE || a.cursor.1 + h + 1 > ATLAS_SIZE {
            return None;
        }
        let (x, y) = a.cursor;
        a.cursor.0 += w + 1;
        a.row_h = a.row_h.max(h);
        upload(&self.queue, &a.texture, [x, y], w, h, &bm.rgba);
        let s = ATLAS_SIZE as f32;
        let uv = [
            x as f32 / s,
            y as f32 / s,
            (x + w) as f32 / s,
            (y + h) as f32 / s,
        ];
        a.entries.insert(key, uv);
        Some(uv)
    }

    fn reset_atlas(&mut self) {
        self.atlas.entries.clear();
        self.atlas.cursor = (0, 0);
        self.atlas.row_h = 0;
    }

    fn build(&mut self, prims: &[Prim], can_copy: bool) -> (Vec<Instance>, Vec<Batch>, bool) {
        let mut inst = Vec::with_capacity(prims.len());
        let mut batches: Vec<Batch> = Vec::new();
        let mut overflow = false;
        for p in prims {
            if p.kind == Kind::Backdrop && can_copy && p.blur > 0.05 {
                // Two passes through scratch textures: see `paint`.
                let r = (p.blur * 3.0).ceil().clamp(1.0, 120.0);
                let grown = [p.rect[0], p.rect[1] - r, p.rect[2], p.rect[3] + 2.0 * r];
                let hclip = [
                    p.clip[0].max(grown[0].floor() as i32),
                    p.clip[1].max(grown[1].floor() as i32),
                    p.clip[2].min((grown[0] + grown[2]).ceil() as i32),
                    p.clip[3].min((grown[1] + grown[3]).ceil() as i32),
                ];
                for (kind, rect, clip, binding, special) in [
                    (10.0, grown, hclip, Binding::Snap, Special::BlurH),
                    (11.0, p.rect, p.clip, Binding::Tmp, Special::BlurV),
                ] {
                    if clip[2] <= clip[0] || clip[3] <= clip[1] {
                        continue;
                    }
                    let index = inst.len() as u32;
                    inst.push(Instance {
                        rect,
                        color: p.color,
                        border_color: [0.0; 4],
                        uv: [0.0, 0.0, 1.0, 1.0],
                        params: [p.radius, 0.0, p.blur, 0.0],
                        meta: [if kind < 10.5 { 0.0 } else { 1.0 }, kind, 0.0, 0.0],
                    });
                    batches.push(Batch {
                        clip,
                        binding,
                        range: index..index + 1,
                        special: Some(special),
                    });
                }
                continue;
            }
            let (binding, uv) = match p.source {
                Source::None if matches!(p.kind, Kind::Gradient | Kind::Radial | Kind::Sheen) => {
                    (None, p.color2)
                }
                Source::None => (None, [0.0, 0.0, 1.0, 1.0]),
                Source::Image(id) => {
                    if !self.images.contains_key(&id) {
                        continue;
                    }
                    (Some(Binding::Image(id)), [0.0, 0.0, 1.0, 1.0])
                }
                Source::Glyph(k) => match self.atlas_slot(k) {
                    Some(uv) => (Some(Binding::Atlas), uv),
                    None => {
                        if self.text.bitmap(k).is_some() {
                            overflow = true;
                        }
                        continue;
                    }
                },
            };
            let index = inst.len() as u32;
            inst.push(Instance {
                rect: p.rect,
                color: p.color,
                border_color: p.border_color,
                uv,
                params: [p.radius, p.border_w, p.blur, p.aux],
                meta: [
                    p.pad,
                    match p.kind {
                        Kind::Shadow => 0.0,
                        Kind::Quad => 1.0,
                        Kind::Mask => 2.0,
                        Kind::Texture => 3.0,
                        Kind::Gradient => 4.0,
                        Kind::Radial => 5.0,
                        Kind::Sheen => 6.0,
                        Kind::Inner => 7.0,
                        Kind::Grain => 8.0,
                        Kind::Backdrop => 9.0,
                    },
                    p.dir[0],
                    p.dir[1],
                ],
            });
            match batches.last_mut() {
                Some(b) if b.special.is_none() && b.clip == p.clip && binding.map_or(true, |nb| nb == b.binding) => {
                    b.range.end = index + 1;
                }
                _ => batches.push(Batch {
                    clip: p.clip,
                    binding: binding.unwrap_or(Binding::Atlas),
                    range: index..index + 1,
                    special: None,
                }),
            }
        }
        (inst, batches, overflow)
    }

    /// Scratch textures for a `w`x`h` target, created on first use.
    fn scratch(&mut self, w: u32, h: u32) -> &Scratch {
        if !self.scratch.as_ref().is_some_and(|s| s.w == w && s.h == h) {
            let mk = |label: &str, usage: wgpu::TextureUsages| {
                self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage,
                    view_formats: &[],
                })
            };
            let snap = mk(
                "backdrop snapshot",
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            );
            let tmp = mk(
                "backdrop scratch",
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            self.scratch = Some(Scratch {
                w,
                h,
                snap_bg: make_bind_group(&self.device, &self.tex_layout, &snap, &self.sampler),
                tmp_bg: make_bind_group(&self.device, &self.tex_layout, &tmp, &self.sampler),
                snap,
                tmp,
            });
        }
        self.scratch.as_ref().expect("scratch just created")
    }

    /// Draw `list` into `view` (size `width`x`height`), clearing to `clear`
    /// (straight RGBA), and submit. Pass `target` (the texture behind `view`,
    /// created with `COPY_SRC`) to enable real backdrop blur; without it a
    /// blur falls back to its tint.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn paint(
        &mut self,
        view: &wgpu::TextureView,
        target: Option<&wgpu::Texture>,
        width: u32,
        height: u32,
        list: &DrawList,
        scale: f32,
        clear: Color,
    ) {
        let prims = flatten(list, scale, width, height, &mut self.text);
        let can_copy = target.is_some();
        let (mut inst, mut batches, overflow) = self.build(&prims, can_copy);
        if overflow {
            log::debug!("glyph atlas full; resetting");
            self.reset_atlas();
            let r = self.build(&prims, can_copy);
            inst = r.0;
            batches = r.1;
        }
        self.queue.write_buffer(
            &self.globals_buf,
            0,
            bytemuck::bytes_of(&Globals {
                size: [width as f32, height as f32],
                _pad: [0.0; 2],
            }),
        );
        if inst.len() as u64 > self.instance_cap {
            self.instance_cap = (inst.len() as u64).next_power_of_two();
            self.instances = make_instance_buffer(&self.device, self.instance_cap);
        }
        if !inst.is_empty() {
            self.queue
                .write_buffer(&self.instances, 0, bytemuck::cast_slice(&inst));
        }
        if let (true, Some(_)) = (batches.iter().any(|b| b.special.is_some()), target) {
            self.scratch(width, height);
        }
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("alloy-ui frame"),
            });
        let clear_op = wgpu::LoadOp::Clear(wgpu::Color {
            r: (clear.r * clear.a) as f64,
            g: (clear.g * clear.a) as f64,
            b: (clear.b * clear.a) as f64,
            a: clear.a as f64,
        });
        let mut at = 0;
        let mut load = clear_op;
        loop {
            // Everything up to the next backdrop blur goes in one pass.
            {
                let mut pass = self.begin(&mut enc, view, load);
                while at < batches.len() && batches[at].special.is_none() {
                    self.draw_batch(&mut pass, &batches[at]);
                    at += 1;
                }
            }
            load = wgpu::LoadOp::Load;
            if at >= batches.len() {
                break;
            }
            let (Some(target), Some(sc)) = (target, self.scratch.as_ref()) else {
                break;
            };
            // Snapshot what is drawn so far, blur it across, then back under the tint.
            enc.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: target,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &sc.snap,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            let tmp_view = sc.tmp.create_view(&wgpu::TextureViewDescriptor::default());
            {
                let mut pass = self.begin(
                    &mut enc,
                    &tmp_view,
                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                );
                self.draw_batch(&mut pass, &batches[at]);
            }
            at += 1;
            if at < batches.len() && batches[at].special == Some(Special::BlurV) {
                let mut pass = self.begin(&mut enc, view, wgpu::LoadOp::Load);
                self.draw_batch(&mut pass, &batches[at]);
                at += 1;
            }
        }
        self.queue.submit([enc.finish()]);
    }

    fn begin(
        &self,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        load: wgpu::LoadOp<wgpu::Color>,
    ) -> wgpu::RenderPass<'static> {
        let mut pass = enc
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("alloy-ui pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.globals_bg, &[]);
        pass.set_vertex_buffer(0, self.instances.slice(..));
        pass
    }

    fn draw_batch(&self, pass: &mut wgpu::RenderPass<'_>, b: &Batch) {
        let bg = match b.binding {
            Binding::Atlas => &self.atlas.bind_group,
            Binding::Image(id) => match self.images.get(&id) {
                Some(i) => &i.bind_group,
                None => return,
            },
            Binding::Snap => match &self.scratch {
                Some(s) => &s.snap_bg,
                None => return,
            },
            Binding::Tmp => match &self.scratch {
                Some(s) => &s.tmp_bg,
                None => return,
            },
        };
        pass.set_bind_group(1, bg, &[]);
        pass.set_scissor_rect(
            b.clip[0] as u32,
            b.clip[1] as u32,
            (b.clip[2] - b.clip[0]) as u32,
            (b.clip[3] - b.clip[1]) as u32,
        );
        pass.draw(0..4, b.range.clone());
    }
}

fn make_instance_buffer(device: &wgpu::Device, cap: u64) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instances"),
        size: cap * std::mem::size_of::<Instance>() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn make_texture(device: &wgpu::Device, w: u32, h: u32, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    tex: &wgpu::Texture,
    sampler: &wgpu::Sampler,
) -> wgpu::BindGroup {
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("texture bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn upload(queue: &wgpu::Queue, tex: &wgpu::Texture, at: [u32; 2], w: u32, h: u32, rgba: &[u8]) {
    if w == 0 || h == 0 {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: at[0],
                y: at[1],
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(w * 4),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
}
