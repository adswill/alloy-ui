//! Streamed pixel surfaces: show RGBA frames that your code produces, on every renderer.
//!
//! A video decoder, a software canvas or a camera feed produces a new bitmap many times a
//! second. [`PixelSurface`] publishes one such bitmap under an id you can put in
//! `DrawCommand::Image` (the `Frame::image` call of a canvas does that); each
//! [`PixelSurface::update`] replaces the picture. The GPU renderer uploads the new frame once, when
//! it is next drawn, and the CPU renderer samples it directly, so it works with every backend
//! including the software fallback and headless tests.
//!
//! The registry is process-wide, so a decoder thread can update a surface while the UI thread
//! draws it. Dropping the surface frees it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Ids of pixel surfaces start here; they never collide with registered images (counting up
/// from 1) or shared GPU textures (from `1 << 62`).
pub const PIXEL_ID_BASE: u64 = 1 << 61;

static NEXT: AtomicU64 = AtomicU64::new(PIXEL_ID_BASE);
static SURFACES: Mutex<Option<HashMap<u64, Arc<PixelData>>>> = Mutex::new(None);

/// One published frame: premultiplied RGBA8 plus a version that changes with every update.
pub(crate) struct PixelData {
    pub width: u32,
    pub height: u32,
    pub version: u64,
    pub rgba: Vec<u8>,
}

fn with<R>(f: impl FnOnce(&mut HashMap<u64, Arc<PixelData>>) -> R) -> R {
    let mut g = SURFACES.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

/// A bitmap that can be replaced at any time and drawn by id. Cheap to keep around.
#[derive(Debug)]
pub struct PixelSurface {
    id: u64,
}

impl PixelSurface {
    /// A transparent surface of `width` by `height` pixels.
    pub fn new(width: u32, height: u32) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let (w, h) = (width.max(1), height.max(1));
        with(|m| {
            m.insert(
                id,
                Arc::new(PixelData {
                    width: w,
                    height: h,
                    version: 0,
                    rgba: vec![0; (w * h * 4) as usize],
                }),
            )
        });
        Self { id }
    }

    /// The id to draw with `DrawCommand::Image { id, .. }`.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Replace the picture with straight (non-premultiplied) RGBA8 of `width` by `height`.
    /// Ignored if `rgba` is shorter than `width * height * 4` bytes.
    pub fn update(&self, width: u32, height: u32, rgba: &[u8]) {
        let n = (width as usize) * (height as usize) * 4;
        if width == 0 || height == 0 || rgba.len() < n {
            return;
        }
        let premul: Vec<u8> = rgba[..n]
            .chunks_exact(4)
            .flat_map(|p| {
                let a = p[3] as u32;
                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                [m(p[0]), m(p[1]), m(p[2]), p[3]]
            })
            .collect();
        self.store(width, height, premul);
    }

    /// As [`PixelSurface::update`] for a fully opaque picture (video, camera): the alpha
    /// channel is ignored and treated as 255, which skips the premultiply pass.
    pub fn update_opaque(&self, width: u32, height: u32, rgba: &[u8]) {
        let n = (width as usize) * (height as usize) * 4;
        if width == 0 || height == 0 || rgba.len() < n {
            return;
        }
        let mut v = rgba[..n].to_vec();
        for p in v.chunks_exact_mut(4) {
            p[3] = 255;
        }
        self.store(width, height, v);
    }

    fn store(&self, width: u32, height: u32, rgba: Vec<u8>) {
        with(|m| {
            if let Some(old) = m.get(&self.id) {
                let version = old.version + 1;
                m.insert(
                    self.id,
                    Arc::new(PixelData {
                        width,
                        height,
                        version,
                        rgba,
                    }),
                );
            }
        });
    }

    /// Current size in pixels.
    pub fn size(&self) -> (u32, u32) {
        with(|m| m.get(&self.id).map_or((0, 0), |d| (d.width, d.height)))
    }

    /// How many times the picture has been replaced.
    pub fn version(&self) -> u64 {
        with(|m| m.get(&self.id).map_or(0, |d| d.version))
    }
}

impl Drop for PixelSurface {
    fn drop(&mut self) {
        with(|m| m.remove(&self.id));
    }
}

pub(crate) fn get(id: u64) -> Option<Arc<PixelData>> {
    if !(PIXEL_ID_BASE..(1 << 62)).contains(&id) {
        return None;
    }
    with(|m| m.get(&id).cloned())
}

#[cfg(feature = "gpu")]
pub(crate) fn exists(id: u64) -> bool {
    with(|m| m.contains_key(&id))
}
