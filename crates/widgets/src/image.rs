//! A bitmap drawn at a fixed size.

use alloy_ui_core::{style::Style, DrawCommand, DrawList, Element, Layout, Size, Widget};
use alloy_ui_layout::Length;

/// Draws an image registered with the renderer (`Renderer::register_image`).
pub struct Image {
    id: u64,
    size: Size,
    alpha: f32,
}

impl Image {
    /// The image `id`, shown at `size` logical pixels.
    pub fn new(id: u64, size: Size) -> Self {
        Self {
            id,
            size,
            alpha: 1.0,
        }
    }
    /// Draw at `alpha` opacity, 0 to 1 (default 1). Fades and enters also apply.
    pub fn opacity(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
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
        out.push(if self.alpha >= 0.9999 {
            DrawCommand::Image {
                bounds: layout.bounds,
                id: self.id,
            }
        } else {
            DrawCommand::ImageAlpha {
                bounds: layout.bounds,
                id: self.id,
                alpha: self.alpha,
            }
        });
    }
}
