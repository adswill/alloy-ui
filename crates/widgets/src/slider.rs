//! Horizontal slider.

use crate::a11y::Role;
use crate::{
    ui::Ui,
    util::{self, focus_ring, key_of},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

const THUMB: f32 = 16.0;
const TRACK: f32 = 4.0;

/// Picks a value in a range by dragging or with the arrow keys. Controlled.
pub struct Slider<M> {
    ui: Ui,
    min: f32,
    max: f32,
    value: f32,
    step: Option<f32>,
    on_change: Option<Box<dyn Fn(f32) -> M>>,
    id: Option<u64>,
    width: Length,
}

impl<M: 'static> Slider<M> {
    /// A slider over `min..=max` currently at `value`.
    pub fn new(ui: &Ui, min: f32, max: f32, value: f32) -> Self {
        Self {
            ui: ui.clone(),
            min,
            max: max.max(min),
            value: value.clamp(min, max.max(min)),
            step: None,
            on_change: None,
            id: None,
            width: Length::Fill(1),
        }
    }
    /// Snap values to multiples of `step` from `min`.
    pub fn step(mut self, step: f32) -> Self {
        self.step = (step > 0.0).then_some(step);
        self
    }
    /// Called with the requested value; without it the slider is disabled.
    pub fn on_change(mut self, f: impl Fn(f32) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
    /// Make keyboard-focusable under `id`.
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    /// Width sizing (default fills).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn snap(&self, v: f32) -> f32 {
        let v = match self.step {
            Some(s) => self.min + ((v - self.min) / s).round() * s,
            None => v,
        };
        v.clamp(self.min, self.max)
    }
    fn value_at(&self, b: &Rectangle, x: f32) -> f32 {
        let span = (b.size.width - THUMB).max(1.0);
        let t = ((x - b.origin.x - THUMB / 2.0) / span).clamp(0.0, 1.0);
        self.snap(self.min + t * (self.max - self.min))
    }
    fn emit(&self, v: f32, out: &mut Vec<M>) {
        if let Some(f) = &self.on_change {
            if (v - self.value).abs() > f32::EPSILON {
                out.push(f(v));
            }
        }
    }
}

impl<M: 'static> Widget<M> for Slider<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(160.0, 24.0)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        if self.on_change.is_none() {
            return false;
        }
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_pressed(Some(key_of(&b)));
                if self.id.is_some() {
                    self.ui.set_focus(self.id);
                }
                self.emit(self.value_at(&b, cursor.x), out);
                true
            }
            Event::CursorMoved(p) if self.ui.pressed() == Some(key_of(&b)) => {
                self.emit(self.value_at(&b, p.x), out);
                true
            }
            Event::KeyPressed(Key::Named(k), _) if self.ui.is_focused(self.id) => {
                let step = self.step.unwrap_or((self.max - self.min) / 100.0);
                let v = match k.as_str() {
                    "ArrowLeft" | "ArrowDown" => self.snap(self.value - step),
                    "ArrowRight" | "ArrowUp" => self.snap(self.value + step),
                    "Home" => self.min,
                    "End" => self.max,
                    _ => return false,
                };
                self.emit(v, out);
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui.describe(Role::Slider, "", &b, |n| {
            n.id = self.id;
            n.value = Some(self.value.to_string());
            n.enabled = self.on_change.is_some();
        });
        let enabled = self.on_change.is_some();
        let span = (b.size.width - THUMB).max(0.0);
        let frac = if self.max > self.min {
            (self.value - self.min) / (self.max - self.min)
        } else {
            0.0
        };
        let cy = b.origin.y + b.size.height / 2.0;
        let x0 = b.origin.x + THUMB / 2.0;
        let tx = b.origin.x + frac * span;
        let rect = |x: f32, w: f32| {
            Rectangle::new(
                Point::new(x, cy - TRACK / 2.0),
                Size::new(w.max(0.0), TRACK),
            )
        };
        out.push(DrawCommand::Quad {
            bounds: rect(x0, span),
            fill: p.border,
            radius: TRACK / 2.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: rect(x0, tx + THUMB / 2.0 - x0),
            fill: if enabled { p.accent } else { p.text_disabled },
            radius: TRACK / 2.0,
            border: None,
            shadow: None,
        });
        let thumb = Rectangle::new(Point::new(tx, cy - THUMB / 2.0), Size::new(THUMB, THUMB));
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        if self.ui.is_focused(self.id) {
            out.push(focus_ring(thumb, THUMB / 2.0, p.focus_ring));
        }
        let hot = enabled && (self.ui.hovered(&b) || self.ui.is_pressed(&b));
        out.push(DrawCommand::Quad {
            bounds: thumb,
            fill: p.surface_raised,
            radius: THUMB / 2.0,
            border: Some(Border {
                color: if !enabled {
                    p.border
                } else if hot {
                    p.accent_hover
                } else {
                    p.accent
                },
                width: 2.0,
            }),
            shadow: None,
        });
    }
}
