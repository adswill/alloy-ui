//! Determinate progress bar.

use crate::a11y::Role;
use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// A thin bar showing `value` within `min..=max`. Static: it never animates,
/// so it costs nothing while idle.
pub struct ProgressBar {
    ui: Ui,
    fraction: f32,
    width: Length,
}

impl ProgressBar {
    /// A bar at `value` of `min..=max`.
    pub fn new(ui: &Ui, min: f32, max: f32, value: f32) -> Self {
        let span = max - min;
        let fraction = if span > 0.0 && value.is_finite() {
            ((value - min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Self {
            ui: ui.clone(),
            fraction,
            width: Length::Fill(1),
        }
    }
    /// Width sizing (default fills).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for ProgressBar {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(120.0, 6.0)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        self.ui.describe(Role::ProgressBar, "", &b, |n| {
            n.value = Some(format!("{:.0}%", self.fraction * 100.0));
        });
        let r = b.size.height / 2.0;
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: t.palette.border,
            radius: r,
            border: None,
            shadow: None,
        });
        let w = b.size.width * self.fraction;
        if w > 0.0 {
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(b.origin.x, b.origin.y),
                    Size::new(w, b.size.height),
                ),
                fill: t.palette.accent,
                radius: r.min(w / 2.0),
                border: None,
                shadow: None,
            });
        }
    }
}
