//! Alloy 2D renderer.
//!
//! * [`WindowRenderer`]: wgpu renderer for OS windows.
//! * [`GpuHeadlessRenderer`]: wgpu offscreen renderer; prefers a software
//!   adapter (lavapipe, WARP, ...) so golden tests are reproducible.
//! * [`CpuRenderer`]: pure-CPU reference rasteriser implementing the same
//!   formulas as the shader. Always available; the source of the goldens.
//!
//! All consume an [`alloy_ui_core::DrawList`] in logical pixels and a scale
//! factor. See [`testing`] for golden-image helpers.
#![warn(missing_docs)]
#![cfg_attr(not(any(feature = "gpu", feature = "soft-window")), allow(dead_code))]

mod cpu;
mod error;
#[cfg(feature = "gpu")]
mod gpu;
mod image;
#[cfg(feature = "gpu")]
mod offscreen;
mod pixels;
mod scene;
#[cfg(feature = "gpu")]
mod shared;
#[cfg(feature = "soft-window")]
mod soft;
pub mod testing;
mod text;
mod window;

use alloy_ui_core::DrawList;

pub use cpu::CpuRenderer;
pub use error::RenderError;
#[cfg(feature = "gpu")]
pub use gpu::GpuContext;
pub use image::Image;
#[cfg(feature = "gpu")]
pub use offscreen::GpuHeadlessRenderer;
pub use pixels::{PixelSurface, PIXEL_ID_BASE};
#[cfg(feature = "gpu")]
pub use shared::{shared_gpu, ShareError, SharedGpu, SHARED_ID_BASE};
pub use text::{GlyphBitmap, TextMeasurer, TextSystem};
#[cfg(feature = "gpu")]
pub use wgpu;
pub use window::{Backend, WindowRenderer};

/// Consumes a frame's draw list. Implemented by the wgpu window renderer and
/// the offscreen renderers used by golden-image tests.
pub trait Renderer {
    /// Render one frame at the given scale factor; presentation is backend-specific.
    fn render(&mut self, list: &DrawList, scale_factor: f32);

    /// Register an image and return the id to use in `DrawCommand::Image`.
    fn register_image(&mut self, image: &Image) -> u64;

    /// Release an image previously registered. Unknown ids are ignored.
    fn unregister_image(&mut self, id: u64);
}

/// Offscreen rendering for tests.
pub trait HeadlessRenderer: Renderer {
    /// Render a list to an image of the given physical size.
    fn render_to_image(
        &mut self,
        list: &DrawList,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Image;
}

/// With `ALLOY_PERF` set, print how long a start-up phase took (since the last mark) to stderr.
/// Used to find what a cold start spends its time on; free when the variable is unset.
#[doc(hidden)]
pub fn perf_mark(label: &str, last: &mut std::time::Instant) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var_os("ALLOY_PERF").is_some()) {
        eprintln!(
            "alloy-perf phase {label}={:.1}ms",
            last.elapsed().as_secs_f64() * 1e3
        );
    }
    *last = std::time::Instant::now();
}

// ------------------------------------------------------------------ start-up prewarm

use std::sync::Mutex;
use std::thread::JoinHandle;

static TEXT: Mutex<Option<JoinHandle<TextSystem>>> = Mutex::new(None);
#[cfg(feature = "gpu")]
static INSTANCE: Mutex<Option<JoinHandle<wgpu::Instance>>> = Mutex::new(None);

/// Start the slow parts of renderer set-up that need no window, on background threads, so they
/// overlap with the platform creating the window: loading the Vulkan/Metal/D3D runtime
/// (about 20 ms) and scanning system fonts (about 6 ms). The next [`WindowRenderer::new`] picks
/// the results up. Harmless to skip or to call twice.
pub fn prewarm() {
    spawn_once(&TEXT, TextSystem::new);
    #[cfg(feature = "gpu")]
    if std::env::var("ALLOY_RENDERER").as_deref() != Ok("cpu") {
        spawn_once(&INSTANCE, || {
            wgpu::Instance::new(&wgpu::InstanceDescriptor::default())
        });
    }
}

fn spawn_once<T: Send + 'static>(slot: &Mutex<Option<JoinHandle<T>>>, f: fn() -> T) {
    let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
    if slot.is_none() {
        *slot = std::thread::Builder::new()
            .name("alloy-prewarm".into())
            .spawn(f)
            .ok();
    }
}

fn take<T>(slot: &Mutex<Option<JoinHandle<T>>>) -> Option<T> {
    let h = slot.lock().unwrap_or_else(|e| e.into_inner()).take()?;
    h.join().ok()
}

/// The prewarmed text system if there is one, else a fresh one.
pub(crate) fn take_text_system() -> TextSystem {
    take(&TEXT).unwrap_or_default()
}

/// The prewarmed wgpu instance if there is one, else a fresh one.
#[cfg(feature = "gpu")]
pub(crate) fn take_instance() -> wgpu::Instance {
    take(&INSTANCE).unwrap_or_else(|| wgpu::Instance::new(&wgpu::InstanceDescriptor::default()))
}
