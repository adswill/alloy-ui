//! Empty space and divider rules.

use crate::{ui::Ui, util};
use alloy_ui_core::{style::Style, DrawCommand, DrawList, Element, Layout, Size, Widget};
use alloy_ui_layout::Length;

/// Invisible filler. `Space::fill()` pushes siblings apart in a row or column.
pub struct Space {
    width: Length,
    height: Length,
}

impl Space {
    /// Takes all leftover room along the container's main axis.
    pub fn fill() -> Self {
        Self {
            width: Length::Fill(1),
            height: Length::Fill(1),
        }
    }
    /// Takes leftover width only, so it can sit in a row without stretching the row's height.
    pub fn fill_w() -> Self {
        Self {
            width: Length::Fill(1),
            height: Length::Fixed(0.0),
        }
    }
    /// Takes leftover height only, for columns.
    pub fn fill_h() -> Self {
        Self {
            width: Length::Fixed(0.0),
            height: Length::Fill(1),
        }
    }
    /// A fixed gap.
    pub fn fixed(width: f32, height: f32) -> Self {
        Self {
            width: Length::Fixed(width),
            height: Length::Fixed(height),
        }
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for Space {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::default()
    }
    fn draw(&self, _layout: &Layout, _out: &mut DrawList) {}
}

/// A one-pixel divider in the theme's border colour. Prefer spacing to
/// separate groups; use a rule only where a hard edge helps.
pub struct Rule {
    ui: Ui,
    vertical: bool,
}

impl Rule {
    /// A horizontal rule that fills the width.
    pub fn horizontal(ui: &Ui) -> Self {
        Self {
            ui: ui.clone(),
            vertical: false,
        }
    }
    /// A vertical rule that fills the height.
    pub fn vertical(ui: &Ui) -> Self {
        Self {
            ui: ui.clone(),
            vertical: true,
        }
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for Rule {
    fn style(&self) -> Style {
        if self.vertical {
            util::sized(Length::Fixed(1.0), Length::Fill(1))
        } else {
            util::sized(Length::Fill(1), Length::Fixed(1.0))
        }
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(1.0, 1.0)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        out.push(DrawCommand::Quad {
            bounds: layout.bounds,
            fill: self.ui.theme().palette.border,
            radius: 0.0,
            border: None,
            shadow: None,
        });
    }
}
