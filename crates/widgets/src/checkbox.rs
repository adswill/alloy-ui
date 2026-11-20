//! Checkbox with a label.

use crate::a11y::Role;
use crate::{
    ui::Ui,
    util::{activation, control_label, focus_ring},
};
use alloy_ui_core::{
    Border, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};

const BOX: f32 = 16.0;

/// A labelled on/off control. Controlled: you pass `checked` and receive the
/// requested new value through `on_toggle`.
pub struct Checkbox<M> {
    ui: Ui,
    label: String,
    checked: bool,
    on_toggle: Option<Box<dyn Fn(bool) -> M>>,
    id: Option<u64>,
}

impl<M: 'static> Checkbox<M> {
    /// A checkbox showing `checked`.
    pub fn new(ui: &Ui, label: impl Into<String>, checked: bool) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            checked,
            on_toggle: None,
            id: None,
        }
    }
    /// Called with the new value when the user toggles; without it the checkbox is disabled.
    pub fn on_toggle(mut self, f: impl Fn(bool) -> M + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
    /// Make keyboard-focusable under `id`.
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn toggle(&self, out: &mut Vec<M>) {
        if let Some(f) = &self.on_toggle {
            out.push(f(!self.checked));
        }
    }
}

impl<M: 'static> Widget<M> for Checkbox<M> {
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(BOX + t.spacing.sm + w, s.line_height.max(BOX))
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let (consumed, activated) = activation(
            &self.ui,
            self.id,
            self.on_toggle.is_some(),
            event,
            &layout.bounds,
            cursor,
        );
        if activated {
            self.toggle(out);
        }
        consumed
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui.describe(Role::Checkbox, &self.label, &b, |n| {
            n.id = self.id;
            n.checked = Some(self.checked);
            n.enabled = self.on_toggle.is_some();
        });
        let enabled = self.on_toggle.is_some();
        let hover = enabled && self.ui.hovered(&b);
        let bx = Rectangle::new(
            Point::new(b.origin.x, b.origin.y + (b.size.height - BOX) / 2.0),
            Size::new(BOX, BOX),
        );
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(bx, t.radii.small, p.focus_ring));
        }
        let (fill, edge) = if !enabled {
            (p.surface, p.border)
        } else if self.checked {
            (if hover { p.accent_hover } else { p.accent }, p.accent)
        } else {
            (
                p.surface_raised,
                if hover {
                    p.text_secondary
                } else {
                    p.border_strong
                },
            )
        };
        out.push(DrawCommand::Quad {
            bounds: bx,
            fill,
            radius: t.radii.small,
            border: Some(Border {
                color: edge,
                width: 1.0,
            }),
            shadow: None,
        });
        if self.checked {
            let c = if enabled {
                p.on_accent
            } else {
                p.text_disabled
            };
            let (x, y) = (bx.origin.x, bx.origin.y);
            let stroke = |from: Point, to: Point| DrawCommand::Line {
                from,
                to,
                width: 1.8,
                color: c,
            };
            out.push(stroke(
                Point::new(x + 3.8, y + 8.4),
                Point::new(x + 6.5, y + 11.0),
            ));
            out.push(stroke(
                Point::new(x + 6.5, y + 11.0),
                Point::new(x + 12.2, y + 4.8),
            ));
        }
        out.push(control_label(&self.ui, &self.label, &b, BOX, enabled));
    }
}
