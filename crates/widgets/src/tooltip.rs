//! Hover hint around another widget.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};

/// Shows `text` under its child while the cursor is over it. There is no
/// delay because widgets have no timer yet. The hint is drawn after the child
/// but can still be covered by later siblings.
pub struct Tooltip<M> {
    ui: Ui,
    text: String,
    child: Vec<Element<M>>,
}

impl<M: 'static> Tooltip<M> {
    /// Attach `text` to `child`.
    pub fn new(ui: &Ui, child: Element<M>, text: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            text: text.into(),
            child: vec![child],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Tooltip<M> {
    fn style(&self) -> Style {
        self.child[0].0.style()
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
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.child, layout, out);
        let b = layout.bounds;
        if self.text.is_empty() || !self.ui.hovered(&b) {
            return;
        }
        let t = self.ui.theme();
        let s = t.typography.caption;
        let w = self.ui.metrics().width(&self.text, s.size);
        let (px, py) = (t.spacing.sm, t.spacing.xs);
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(b.origin.x, b.origin.y + b.size.height + 6.0),
                Size::new(w + 2.0 * px, s.line_height + 2.0 * py),
            ),
            fill: t.palette.text,
            radius: t.radii.small,
            border: None,
            shadow: Some(t.elevation.medium),
        });
        out.push(DrawCommand::Text {
            origin: Point::new(b.origin.x + px, b.origin.y + b.size.height + 6.0 + py),
            content: self.text.clone(),
            size: s.size,
            color: t.palette.background,
            max_width: None,
        });
    }
}
