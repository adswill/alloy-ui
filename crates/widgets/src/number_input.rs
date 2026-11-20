//! Numeric stepper.

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

const BTN: f32 = 28.0;

/// A value with `−` and `+` buttons, clamped to `min..=max` and moved by
/// `step`. Click, Up/Down (PageUp/PageDown move ten steps) and Home/End work.
/// Controlled.
pub struct NumberInput<M> {
    ui: Ui,
    id: u64,
    value: f64,
    min: f64,
    max: f64,
    step: f64,
    decimals: usize,
    unit: String,
    on_change: Option<Box<dyn Fn(f64) -> M>>,
    width: Length,
}

impl<M: 'static> NumberInput<M> {
    /// A stepper currently at `value`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, value: f64) -> Self {
        Self {
            ui: ui.clone(),
            id,
            value,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            step: 1.0,
            decimals: 0,
            unit: String::new(),
            on_change: None,
            width: Length::Shrink,
        }
    }
    /// Allowed range, inclusive.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max.max(min);
        self.value = self.value.clamp(self.min, self.max);
        self
    }
    /// Amount per click; also sets decimals to fit it (`0.25` shows two).
    pub fn step(mut self, step: f64) -> Self {
        self.step = step.abs().max(f64::EPSILON);
        let mut d = 0;
        while d < 6 && (self.step * 10f64.powi(d as i32)).fract().abs() > 1e-9 {
            d += 1;
        }
        self.decimals = d;
        self
    }
    /// Text after the value, e.g. `"px"`.
    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }
    /// Called with the requested value; without it the field is disabled.
    pub fn on_change(mut self, f: impl Fn(f64) -> M + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
    /// Width sizing (default shrinks to the widest value).
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn label(&self) -> String {
        format!("{:.*}{}", self.decimals, self.value, self.unit)
    }
    fn shifted(&self, steps: f64) -> f64 {
        // Round to `decimals` so repeated 0.1 steps don't drift.
        let k = 10f64.powi(self.decimals as i32);
        (((self.value + steps * self.step) * k).round() / k).clamp(self.min, self.max)
    }
    fn emit(&self, v: f64, out: &mut Vec<M>) {
        if let Some(f) = &self.on_change {
            if (v - self.value).abs() > 1e-12 {
                out.push(f(v));
            }
        }
    }
    fn minus(b: &Rectangle) -> Rectangle {
        Rectangle::new(b.origin, Size::new(BTN, b.size.height))
    }
    fn plus(b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x + b.size.width - BTN, b.origin.y),
            Size::new(BTN, b.size.height),
        )
    }
}

impl<M: 'static> Widget<M> for NumberInput<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let m = self.ui.metrics();
        let widest = [self.min, self.max, self.value]
            .iter()
            .filter(|v| v.is_finite())
            .map(|v| m.width(&format!("{:.*}{}", self.decimals, v, self.unit), s.size))
            .fold(m.width("0", s.size) * 3.0, f32::max);
        Size::new(
            widest + 2.0 * BTN + 2.0 * t.spacing.md,
            s.line_height + 2.0 * t.spacing.sm,
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
        if self.on_change.is_none() {
            return false;
        }
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
                if Self::minus(&b).contains(cursor) {
                    self.ui.set_pressed(Some(key_of(&Self::minus(&b))));
                    self.emit(self.shifted(-1.0), out);
                } else if Self::plus(&b).contains(cursor) {
                    self.ui.set_pressed(Some(key_of(&Self::plus(&b))));
                    self.emit(self.shifted(1.0), out);
                }
                true
            }
            Event::Scrolled { dy, .. }
                if b.contains(cursor) && self.ui.is_focused(Some(self.id)) =>
            {
                self.emit(self.shifted(dy.signum() as f64), out);
                true
            }
            Event::KeyPressed(Key::Named(k), _) if self.ui.is_focused(Some(self.id)) => {
                let v = match k.as_str() {
                    "ArrowUp" => self.shifted(1.0),
                    "ArrowDown" => self.shifted(-1.0),
                    "PageUp" => self.shifted(10.0),
                    "PageDown" => self.shifted(-10.0),
                    "Home" if self.min.is_finite() => self.min,
                    "End" if self.max.is_finite() => self.max,
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
        self.ui.describe(Role::SpinButton, &self.unit, &b, |n| {
            n.id = Some(self.id);
            n.value = Some(self.label());
            n.enabled = self.on_change.is_some();
        });
        let s = t.typography.body;
        let enabled = self.on_change.is_some();
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: if enabled { p.surface_raised } else { p.surface },
            radius: t.radii.medium,
            border: Some(Border {
                color: if enabled { p.border_strong } else { p.border },
                width: 1.0,
            }),
            shadow: None,
        });
        let at_min = self.value <= self.min;
        let at_max = self.value >= self.max;
        for (r, glyph, dead) in [
            (Self::minus(&b), "\u{2212}", at_min),
            (Self::plus(&b), "+", at_max),
        ] {
            let live = enabled && !dead;
            if live && (self.ui.is_pressed(&r) || self.ui.hovered(&r)) {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(r.origin.x + 2.0, r.origin.y + 2.0),
                        Size::new(r.size.width - 4.0, r.size.height - 4.0),
                    ),
                    fill: if self.ui.is_pressed(&r) {
                        p.border
                    } else {
                        p.surface
                    },
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            }
            let w = self.ui.metrics().width(glyph, s.size);
            out.push(DrawCommand::Text {
                family: alloy_ui_core::FontFamily::Sans,
                weight: 500,
                line_height: None,
                origin: Point::new(
                    r.origin.x + (r.size.width - w) / 2.0,
                    r.origin.y + (r.size.height - s.line_height) / 2.0,
                ),
                content: glyph.into(),
                size: s.size,
                color: if live { p.text } else { p.text_disabled },
                max_width: None,
            });
        }
        let label = self.label();
        let w = self.ui.metrics().width(&label, s.size);
        out.push(DrawCommand::Text {
            family: alloy_ui_core::FontFamily::Sans,
            weight: 400,
            line_height: None,
            origin: Point::new(
                b.origin.x + (b.size.width - w) / 2.0,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: label,
            size: s.size,
            color: if enabled { p.text } else { p.text_disabled },
            max_width: None,
        });
    }
}
