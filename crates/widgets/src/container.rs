//! A single-child box with padding, background, border and shadow.

use crate::util;
use alloy_ui_core::{
    style::{Align, Axis, Padding, Style},
    Border, Color, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Shadow, Size,
    Widget,
};
use alloy_ui_layout::Length;

/// Wraps one child. Draws a background only when asked, so plain grouping
/// costs nothing (flatten nesting instead of stacking cards).
pub struct Container<M> {
    style: Style,
    child: Vec<Element<M>>,
    background: Option<Color>,
    gradient: Option<(Color, Color, f32)>,
    border: Option<Border>,
    radius: f32,
    shadow: Option<Shadow>,
    hover: Option<(crate::Ui, Color)>,
}

impl<M: 'static> Container<M> {
    /// Wrap `child`.
    pub fn new(child: Element<M>) -> Self {
        Self {
            style: Style {
                axis: Axis::Vertical,
                ..Style::default()
            },
            child: vec![child],
            background: None,
            gradient: None,
            border: None,
            radius: 0.0,
            shadow: None,
            hover: None,
        }
    }
    /// Padding between the box edge and the child.
    pub fn padding(mut self, p: Padding) -> Self {
        self.style.padding = p;
        self
    }
    /// Fill colour.
    pub fn background(mut self, c: Color) -> Self {
        self.background = Some(c);
        self
    }
    /// Fill with a linear gradient instead of a flat colour. `angle_deg` 0 runs
    /// top to bottom, 90 left to right. Keep the two stops close: a gentle
    /// tonal shift reads as depth, a loud one reads as decoration.
    pub fn gradient(mut self, from: Color, to: Color, angle_deg: f32) -> Self {
        self.gradient = Some((from, to, angle_deg));
        self
    }
    /// Fill colour while the pointer is over the box, eased in and out. Without a
    /// [`background`](Self::background) the colour fades in over whatever is behind.
    pub fn hover_background(mut self, ui: &crate::Ui, c: Color) -> Self {
        self.hover = Some((ui.clone(), c));
        self
    }
    /// Border.
    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border = Some(Border { color, width });
        self
    }
    /// Corner radius.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// Drop shadow; use only for real elevation.
    pub fn shadow(mut self, s: Shadow) -> Self {
        self.shadow = Some(s);
        self
    }
    /// Horizontal alignment of the child.
    pub fn align_x(mut self, a: Align) -> Self {
        self.style.align = a;
        self
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.style.width = w;
        self
    }
    /// Height sizing.
    pub fn height(mut self, h: Length) -> Self {
        self.style.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Container<M> {
    fn style(&self) -> Style {
        self.style
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        util::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        if let Some((from, to, angle_deg)) = self.gradient {
            out.push(DrawCommand::GradientQuad {
                bounds: layout.bounds,
                from,
                to,
                angle_deg,
                radius: self.radius,
                border: self.border,
                shadow: self.shadow,
            });
        } else if self.background.is_some()
            || self.border.is_some()
            || self.shadow.is_some()
            || self.hover.is_some()
        {
            let base = self.background.unwrap_or(Color::TRANSPARENT);
            let fill = match &self.hover {
                Some((ui, hot)) => {
                    let h = ui.ease(
                        crate::Ui::anim_key(&layout.bounds, 2),
                        ui.hovered(&layout.bounds) as u8 as f32,
                    );
                    if base.a > 0.0 {
                        util::mix(base, *hot, h)
                    } else {
                        hot.with_alpha(hot.a * h)
                    }
                }
                None => base,
            };
            out.push(DrawCommand::Quad {
                bounds: layout.bounds,
                fill,
                radius: self.radius,
                border: self.border,
                shadow: self.shadow,
            });
        }
        util::draw_children(&self.child, layout, out);
    }
}
