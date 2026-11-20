//! Alloy 2D renderer. The wgpu backend and headless mode land in the render workstream.
#![warn(missing_docs)]

use alloy_ui_core::DrawList;

/// RGBA8 pixels, row-major, premultiplied-free sRGB.
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

/// Consumes a frame's draw list. Implemented by the wgpu window renderer and
/// the offscreen renderer used by golden-image tests.
pub trait Renderer {
    /// Render one frame at the given scale factor and return nothing; presentation is backend-specific.
    fn render(&mut self, list: &DrawList, scale_factor: f32);
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
