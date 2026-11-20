//! Segmented control: a compact either-or switch between a few views.

use crate::a11y::Role;
use crate::{
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// Two to five labelled segments, one selected. Use it for switching what a
/// view shows (a tab bar without the pages); use [`crate::Radio`] for form
/// choices. Left/Right change the selection while focused.
pub struct Segmented<M> {
    ui: Ui,
    id: u64,
    labels: Vec<String>,
    selected: usize,
    on_select: Option<Box<dyn Fn(usize) -> M>>,
    width: Length,
}

impl<M: 'static> Segmented<M> {
    /// Segments `labels` with `selected` active. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, labels: Vec<String>, selected: usize) -> Self {
        Self {
            ui: ui.clone(),
            id,
            labels,
            selected,
            on_select: None,
            width: Length::Shrink,
        }
    }
    /// Called with the chosen index; without it the control is disabled.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn natural_widths(&self) -> Vec<f32> {
        let t = self.ui.theme();
        let m = self.ui.metrics();
        self.labels
            .iter()
            .map(|l| m.width(l, t.typography.label.size) + 2.0 * t.spacing.md)
            .collect()
    }
    /// Segment rectangles inside `b`; extra width is shared equally.
    fn segments(&self, b: &Rectangle) -> Vec<Rectangle> {
        let nat = self.natural_widths();
        let total: f32 = nat.iter().sum();
        let extra = ((b.size.width - 4.0 - total) / nat.len().max(1) as f32).max(0.0);
        let mut x = b.origin.x + 2.0;
        nat.iter()
            .map(|w| {
                let r = Rectangle::new(
                    Point::new(x, b.origin.y + 2.0),
                    Size::new(w + extra, b.size.height - 4.0),
                );
                x += w + extra;
                r
            })
            .collect()
    }
}

impl<M: 'static> Widget<M> for Segmented<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        Size::new(
            self.natural_widths().iter().sum::<f32>() + 4.0,
            t.typography.label.line_height + 2.0 * t.spacing.sm + 4.0,
        )
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let Some(f) = &self.on_select else {
            return false;
        };
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                if let Some(i) = self.segments(&b).iter().position(|r| r.contains(cursor)) {
                    if i != self.selected {
                        out.push(f(i));
                    }
                }
                true
            }
            Event::MouseReleased(MouseButton::Left) => false,
            Event::KeyPressed(Key::Named(n), _) if self.ui.is_focused(Some(self.id)) => {
                let last = self.labels.len().saturating_sub(1);
                let to = match n.as_str() {
                    "ArrowLeft" => self.selected.saturating_sub(1),
                    "ArrowRight" => (self.selected + 1).min(last),
                    _ => return false,
                };
                if to != self.selected {
                    out.push(f(to));
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let seg = b.size.width / self.labels.len().max(1) as f32;
        for (i, l) in self.labels.iter().enumerate() {
            let r = Rectangle::new(
                Point::new(b.origin.x + i as f32 * seg, b.origin.y),
                Size::new(seg, b.size.height),
            );
            self.ui.describe(Role::Tab, l, &r, |n| {
                n.id = Some(self.id);
                n.checked = Some(i == self.selected);
                n.enabled = self.on_select.is_some();
            });
        }
        let enabled = self.on_select.is_some();
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: crate::util::over(p.surface, p.hover),
            radius: t.radii.medium,
            border: Some(Border {
                color: p.border,
                width: 1.0,
            }),
            shadow: None,
        });
        let s = t.typography.label;
        let m = self.ui.metrics();
        for (i, (seg, label)) in self.segments(&b).iter().zip(&self.labels).enumerate() {
            let on = i == self.selected;
            let hover = enabled && !on && self.ui.hovered(seg);
            if on {
                out.push(DrawCommand::Quad {
                    bounds: *seg,
                    fill: p.surface_raised,
                    radius: (t.radii.medium - 1.0).max(0.0),
                    border: Some(Border {
                        color: p.border_control,
                        width: 1.0,
                    }),
                    shadow: Some(t.elevation.low),
                });
            } else if hover {
                out.push(DrawCommand::Quad {
                    bounds: *seg,
                    fill: p.hover,
                    radius: (t.radii.medium - 1.0).max(0.0),
                    border: None,
                    shadow: None,
                });
            }
            let w = m.width(label, s.size);
            out.push(DrawCommand::Text {
                family: alloy_ui_core::FontFamily::Sans,
                weight: if on { 600 } else { 400 },
                line_height: None,
                origin: Point::new(
                    seg.origin.x + (seg.size.width - w) / 2.0,
                    seg.origin.y + (seg.size.height - s.line_height) / 2.0,
                ),
                content: label.clone(),
                size: s.size,
                color: if !enabled {
                    p.text_disabled
                } else if on || hover {
                    p.text
                } else {
                    p.text_secondary
                },
                max_width: None,
            });
        }
    }
}
