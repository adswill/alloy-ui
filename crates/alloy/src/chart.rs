//! A small bar chart, the one drawing widget the facade ships itself.

use crate::{cx, view::Node};
use alloy_ui_core::{
    style::{Length, Style},
    Color, DrawCommand, DrawList, Element, Layout, Point, Rectangle, Size, Widget,
};

struct Bars {
    values: Vec<f32>,
    color: Color,
    track: Color,
    height: f32,
}

impl<M> Widget<M> for Bars {
    fn style(&self) -> Style {
        Style {
            width: Length::Fill(1),
            height: Length::Fixed(self.height),
            ..Style::default()
        }
    }
    fn measure(&self, _: Size) -> Size {
        Size::new(0.0, self.height)
    }
    fn draw(&self, l: &Layout, out: &mut DrawList) {
        let n = self.values.len().max(1) as f32;
        let gap = 6.0;
        let w = ((l.bounds.size.width - gap * (n - 1.0)) / n).max(1.0);
        let base = l.bounds.origin.y + l.bounds.size.height;
        let quad = |bounds, fill| DrawCommand::Quad {
            bounds,
            fill,
            radius: 2.0,
            border: None,
            shadow: None,
        };
        for (i, v) in self.values.iter().enumerate() {
            let x = l.bounds.origin.x + i as f32 * (w + gap);
            let h = l.bounds.size.height * v.clamp(0.0, 1.0);
            let full = Rectangle::new(
                Point::new(x, l.bounds.origin.y),
                Size::new(w, l.bounds.size.height),
            );
            out.push(quad(full, self.track));
            out.push(quad(
                Rectangle::new(Point::new(x, base - h), Size::new(w, h)),
                self.color,
            ));
        }
    }
}

/// A bar chart over values in `0.0..=1.0`, 120 px tall, filling the width.
pub fn bars<M: 'static>(values: impl IntoIterator<Item = f32>) -> Node<M> {
    let p = cx::palette();
    Node(Element::new(Bars {
        values: values.into_iter().collect(),
        color: p.accent,
        track: p.surface,
        height: 120.0,
    }))
}
