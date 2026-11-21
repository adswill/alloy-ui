//! Share GPU textures with the window renderer.
//!
//! An app that renders its own wgpu scene (a game view, a map, a video frame, a
//! simulation) asks for the renderer's [`SharedGpu`], creates a texture on its
//! device, draws into it each frame, and shows it in the UI with
//! `DrawCommand::Image { id }` (the `GpuView` widget does that). Nothing is copied or read
//! back: the window renderer samples the very texture the app drew.
//!
//! Textures must be `Rgba8Unorm` (not sRGB) with `TEXTURE_BINDING` usage, because the
//! window renderer composes in display space with premultiplied alpha. Use
//! [`SharedGpu::create_target`] to get one with the right usage flags.
//!
//! This is a GPU-only feature: the CPU fallback renderer cannot sample a GPU texture,
//! so [`shared_gpu`] returns `None` there, and a published id draws nothing.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Ids of shared textures start here so they can never collide with the ids
/// `Renderer::register_image` hands out (which count up from 1).
pub const SHARED_ID_BASE: u64 = 1 << 62;

static NEXT: AtomicU64 = AtomicU64::new(SHARED_ID_BASE);
static GLOBAL: Mutex<Option<Arc<SharedGpu>>> = Mutex::new(None);

struct Entry {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// The renderer's device and queue plus a registry of app-owned textures it may draw.
pub struct SharedGpu {
    /// The device every shared texture must come from.
    pub device: wgpu::Device,
    /// The queue the renderer submits on.
    pub queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    entries: Mutex<HashMap<u64, Entry>>,
}

/// Why a texture could not be published.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ShareError {
    /// Only `Rgba8Unorm` is accepted.
    #[error("shared textures must be Rgba8Unorm, got {0:?}")]
    Format(wgpu::TextureFormat),
    /// The texture needs `TEXTURE_BINDING` usage.
    #[error("shared textures need TextureUsages::TEXTURE_BINDING")]
    Usage,
    /// Only plain 2D, single-sample, single-layer textures can be shown.
    #[error("shared textures must be 2D, single-sample, one layer")]
    Shape,
}

impl SharedGpu {
    pub(crate) fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        layout: wgpu::BindGroupLayout,
        sampler: wgpu::Sampler,
    ) -> Arc<Self> {
        Arc::new(Self {
            device,
            queue,
            layout,
            sampler,
            entries: Mutex::new(HashMap::new()),
        })
    }

    /// Make a texture the app can render into and the UI can show. Returns the texture
    /// (draw into it with `RENDER_ATTACHMENT`) and the id for `DrawCommand::Image`.
    pub fn create_target(&self, width: u32, height: u32) -> (wgpu::Texture, u64) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("alloy shared target"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let id = self
            .publish(&texture)
            .expect("create_target builds a valid texture");
        (texture, id)
    }

    /// Show an app-owned texture. The returned id is valid until [`SharedGpu::unpublish`].
    /// The texture handle is cloned (it is reference counted), so keep drawing into yours.
    pub fn publish(&self, texture: &wgpu::Texture) -> Result<u64, ShareError> {
        if texture.format() != wgpu::TextureFormat::Rgba8Unorm {
            return Err(ShareError::Format(texture.format()));
        }
        if !texture
            .usage()
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
        {
            return Err(ShareError::Usage);
        }
        if texture.dimension() != wgpu::TextureDimension::D2
            || texture.sample_count() != 1
            || texture.depth_or_array_layers() != 1
        {
            return Err(ShareError::Shape);
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shared texture bg"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                id,
                Entry {
                    texture: texture.clone(),
                    bind_group,
                },
            );
        Ok(id)
    }

    /// Stop showing `id`. Unknown ids are ignored. The app's own texture stays alive.
    pub fn unpublish(&self, id: u64) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }

    /// Size in pixels of a published texture.
    pub fn size_of(&self, id: u64) -> Option<(u32, u32)> {
        let e = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        e.get(&id).map(|e| (e.texture.width(), e.texture.height()))
    }

    pub(crate) fn contains(&self, id: u64) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&id)
    }

    pub(crate) fn bind_group(&self, id: u64) -> Option<wgpu::BindGroup> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .map(|e| e.bind_group.clone())
    }
}

/// Record the first window renderer's device as the process-wide shared one.
pub(crate) fn set_global(g: &Arc<SharedGpu>) {
    let mut slot = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = Some(g.clone());
    }
}

/// The GPU context of the first window renderer, once the app's window exists; `None`
/// before that, or when the CPU renderer is in use. Call it from `Application::update`
/// or a task, not at start-up before the window is made.
pub fn shared_gpu() -> Option<Arc<SharedGpu>> {
    GLOBAL.lock().unwrap_or_else(|e| e.into_inner()).clone()
}
