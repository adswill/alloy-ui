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

mod cpu;
mod error;
mod gpu;
mod image;
mod offscreen;
mod scene;
pub mod testing;
mod text;
mod window;

use alloy_ui_core::DrawList;

pub use cpu::CpuRenderer;
pub use error::RenderError;
pub use gpu::GpuContext;
pub use image::Image;
pub use offscreen::GpuHeadlessRenderer;
pub use text::{GlyphBitmap, TextMeasurer, TextSystem};
pub use window::WindowRenderer;

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
