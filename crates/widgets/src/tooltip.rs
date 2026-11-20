//! Hover hint around another widget.

use crate::{ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_theme::Easing;

const DELAY_MS: u64 = 500;

/// Shows `text` under its child once the cursor has rested on it for half a
/// second, fading in. It is drawn on the overlay layer, above later siblings.
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
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.child, layout, out);
        let b = layout.bounds;
        let hovered = !self.text.is_empty() && self.ui.hovered(&b);
        // Wait for the pointer to rest before showing; hide at once.
        let dwell = self.ui.ease_with(
            Ui::anim_key(&b, 5),
            hovered as u8 as f32,
            if hovered { DELAY_MS } else { 0 },
            Easing::LINEAR,
        );
        let show = hovered && dwell >= 1.0;
        let fade = self.ui.ease(Ui::anim_key(&b, 6), show as u8 as f32);
        if !show {
            return;
        }
        let t = self.ui.theme();
        let s = t.typography.caption;
        let w = self.ui.metrics().width(&self.text, s.size);
        let (px, py) = (t.spacing.sm, t.spacing.xs);
        let y = b.origin.y + b.size.height + 6.0;
        let mut shadow = t.elevation.medium;
        shadow.color = shadow.color.with_alpha(shadow.color.a * fade);
        // Drawn through the overlay layer so later siblings cannot cover it.
        self.ui.overlay(
            None,
            None,
            vec![
                DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(b.origin.x, y),
                        Size::new(w + 2.0 * px, s.line_height + 2.0 * py),
                    ),
                    fill: t.palette.text.with_alpha(fade),
                    radius: t.radii.small,
                    border: None,
                    shadow: Some(shadow),
                },
                DrawCommand::Text {
                    weight: 400,
                    line_height: None,
                    origin: Point::new(b.origin.x + px, y + py),
                    content: self.text.clone(),
                    size: s.size,
                    color: t.palette.background.with_alpha(fade),
                    max_width: None,
                },
            ],
        );
    }
}
