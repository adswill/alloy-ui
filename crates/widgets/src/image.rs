//! A bitmap drawn at a fixed size.

use alloy_ui_core::{style::Style, DrawCommand, DrawList, Element, Layout, Size, Widget};
use alloy_ui_layout::Length;

/// Draws an image registered with the renderer (`Renderer::register_image`).
pub struct Image {
    id: u64,
    size: Size,
}

impl Image {
    /// The image `id`, shown at `size` logical pixels.
    pub fn new(id: u64, size: Size) -> Self {
        Self { id, size }
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for Image {
    fn style(&self) -> Style {
        Style {
            width: Length::Fixed(self.size.width),
            height: Length::Fixed(self.size.height),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        self.size
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        out.push(DrawCommand::Image {
            bounds: layout.bounds,
            id: self.id,
        });
    }
}
