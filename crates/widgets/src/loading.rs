//! Loading and progress indicators: [`Skeleton`] placeholders with a shimmer,
//! [`Spinner`]s, and a determinate [`ProgressRing`]. They loop only while on
//! screen, and under reduced motion they stand still (and stop ticking).

use crate::{a11y::Role, ui::Ui, util};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use alloy_ui_theme::{Appearance, Easing};
use std::f32::consts::{PI, TAU};

// ------------------------------------------------------------ Skeleton

/// A grey placeholder block with a slow highlight sweeping across it, for
/// content that is still loading. Size it like the content it stands in for.
pub struct Skeleton {
    ui: Ui,
    width: Length,
    height: f32,
    radius: f32,
}

impl Skeleton {
    /// A block `height` tall; fills the width until [`Skeleton::width`] says otherwise.
    pub fn new(ui: &Ui, height: f32) -> Self {
        Self {
            ui: ui.clone(),
            width: Length::Fill(1),
            height,
            radius: ui.theme().radii.medium,
        }
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Corner radius; a circle for avatars is `height / 2`.
    pub fn radius(mut self, r: f32) -> Self {
        self.radius = r;
        self
    }
    /// As an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

/// How long one sweep takes, including the rest between sweeps.
const SWEEP_MS: u64 = 1700;

impl<M> Widget<M> for Skeleton {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Fixed(self.height))
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(80.0, self.height)
    }
    fn animating(&self) -> bool {
        !self.ui.reduced_motion()
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let dark = t.appearance == Appearance::Dark;
        let ink = t.palette.text;
        let base = ink.with_alpha(if dark { 0.09 } else { 0.065 });
        let hi = ink.with_alpha(if dark { 0.17 } else { 0.025 });
        let hw = 0.28;
        let phase = (self.ui.now() % SWEEP_MS) as f32 / SWEEP_MS as f32;
        // Sweeps in the first 70% and rests, so it reads as a glint, not a loop.
        let k = Easing::SINE_IN_OUT.at((phase / 0.7).min(1.0));
        let position = -hw + (1.0 + 2.0 * hw) * k;
        if self.ui.reduced_motion() {
            out.push(DrawCommand::Quad {
                bounds: layout.bounds,
                fill: base,
                radius: self.radius,
                border: None,
                shadow: None,
            });
        } else {
            out.push(DrawCommand::Sheen {
                bounds: layout.bounds,
                base,
                highlight: hi,
                angle_deg: 100.0,
                position,
                half_width: hw,
                radius: self.radius,
            });
        }
        self.ui
            .describe(Role::ProgressBar, "Loading", &layout.bounds, |_| {});
    }
}

// ------------------------------------------------------------ Spinner

/// The shape of a [`Spinner`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerKind {
    /// A ring with an arc that turns and breathes between short and long.
    #[default]
    Ring,
    /// Three dots handing a light along.
    Dots,
    /// An indeterminate bar: a segment sliding across a track.
    Bar,
}

/// An indeterminate "working" indicator.
pub struct Spinner {
    ui: Ui,
    kind: SpinnerKind,
    size: f32,
    color: Option<Color>,
}

impl Spinner {
    /// A spinner of `kind`; `size` is the ring's diameter or the bar's width.
    pub fn new(ui: &Ui, kind: SpinnerKind) -> Self {
        let size = match kind {
            SpinnerKind::Ring => 20.0,
            SpinnerKind::Dots => 28.0,
            SpinnerKind::Bar => 160.0,
        };
        Self {
            ui: ui.clone(),
            kind,
            size,
            color: None,
        }
    }
    /// Diameter (ring), overall width (dots) or length (bar), in logical pixels.
    pub fn size(mut self, s: f32) -> Self {
        self.size = s.max(8.0);
        self
    }
    /// Colour; default is the accent.
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
    /// As an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
    fn height(&self) -> f32 {
        match self.kind {
            SpinnerKind::Ring => self.size,
            SpinnerKind::Dots => (self.size / 4.5).max(5.0),
            SpinnerKind::Bar => 4.0,
        }
    }
}

/// Polyline approximating the arc `from..from+sweep` (radians, 0 = 3 o'clock, clockwise).
pub(crate) fn arc(out: &mut DrawList, c: Point, r: f32, from: f32, sweep: f32, w: f32, col: Color) {
    let n = ((sweep.abs() / TAU) * 56.0).ceil().max(2.0) as usize;
    let at = |i: usize| {
        let a = from + sweep * i as f32 / n as f32;
        Point::new(c.x + r * a.cos(), c.y + r * a.sin())
    };
    for i in 0..n {
        out.push(DrawCommand::Line {
            from: at(i),
            to: at(i + 1),
            width: w,
            color: col,
        });
    }
}

impl<M> Widget<M> for Spinner {
    fn style(&self) -> Style {
        let w = if self.kind == SpinnerKind::Bar {
            Length::Fixed(self.size)
        } else {
            Length::Shrink
        };
        util::sized(w, Length::Shrink)
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(self.size, self.height())
    }
    fn animating(&self) -> bool {
        !self.ui.reduced_motion()
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let col = self.color.unwrap_or(t.palette.accent);
        let still = self.ui.reduced_motion();
        let ms = self.ui.now() as f32;
        self.ui.describe(Role::ProgressBar, "Loading", &b, |_| {});
        match self.kind {
            SpinnerKind::Ring => {
                let w = (self.size / 8.0).clamp(2.0, 4.0);
                let r = self.size / 2.0 - w / 2.0;
                let c = Point::new(b.origin.x + self.size / 2.0, b.origin.y + self.size / 2.0);
                arc(out, c, r, 0.0, TAU - 0.001, w, t.palette.border);
                let (from, sweep) = if still {
                    (-PI / 2.0, PI * 0.6)
                } else {
                    let turn = (ms % 1000.0) / 1000.0;
                    let breathe = 0.5 - 0.5 * ((ms % 1600.0) / 1600.0 * TAU).cos();
                    (turn * TAU - PI / 2.0, 0.7 + 3.4 * breathe)
                };
                arc(out, c, r, from, sweep, w, col);
            }
            SpinnerKind::Dots => {
                let d = self.height();
                let gap = (self.size - 3.0 * d) / 2.0;
                for i in 0..3 {
                    let a = if still {
                        0.55
                    } else {
                        let ph = (ms / 1200.0 - i as f32 * 0.16).rem_euclid(1.0);
                        0.22 + 0.78 * (ph * PI).sin().max(0.0).powi(2)
                    };
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(b.origin.x + i as f32 * (d + gap), b.origin.y),
                            Size::new(d, d),
                        ),
                        fill: col.with_alpha(a),
                        radius: d / 2.0,
                        border: None,
                        shadow: None,
                    });
                }
            }
            SpinnerKind::Bar => {
                let h = b.size.height;
                out.push(DrawCommand::Quad {
                    bounds: b,
                    fill: t.palette.border,
                    radius: h / 2.0,
                    border: None,
                    shadow: None,
                });
                let seg = b.size.width * 0.34;
                let x = if still {
                    b.size.width * 0.33
                } else {
                    let p = Easing::SINE_IN_OUT.at((ms % 1500.0) / 1500.0);
                    -seg + (b.size.width + seg) * p
                };
                out.push(DrawCommand::PushClip(b));
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(b.origin.x + x, b.origin.y),
                        Size::new(seg, h),
                    ),
                    fill: col,
                    radius: h / 2.0,
                    border: None,
                    shadow: None,
                });
                out.push(DrawCommand::PopClip);
            }
        }
    }
}

// ------------------------------------------------------------ ProgressRing

/// A determinate ring whose arc eases to each new value.
pub struct ProgressRing {
    ui: Ui,
    key: u64,
    fraction: f32,
    size: f32,
    label: bool,
}

impl ProgressRing {
    /// A ring showing `fraction` (0 to 1). `key` keeps its animation across rebuilds.
    pub fn new(ui: &Ui, key: u64, fraction: f32) -> Self {
        Self {
            ui: ui.clone(),
            key,
            fraction: if fraction.is_finite() {
                fraction.clamp(0.0, 1.0)
            } else {
                0.0
            },
            size: 56.0,
            label: true,
        }
    }
    /// Diameter in logical pixels.
    pub fn size(mut self, s: f32) -> Self {
        self.size = s.max(16.0);
        self
    }
    /// Show or hide the percentage in the middle (shown by default).
    pub fn label(mut self, on: bool) -> Self {
        self.label = on;
        self
    }
    /// As an element.
    pub fn into_element<M: 'static>(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M> Widget<M> for ProgressRing {
    fn style(&self) -> Style {
        Style::default()
    }
    fn measure(&self, _a: Size) -> Size {
        Size::new(self.size, self.size)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let f = self.ui.animate(
            self.key,
            self.fraction,
            crate::Transition::eased(t.motion.slow_ms + 200, Easing::QUART_OUT),
        );
        let w = (self.size / 9.0).clamp(3.0, 6.0);
        let r = self.size / 2.0 - w / 2.0;
        let c = Point::new(b.origin.x + self.size / 2.0, b.origin.y + self.size / 2.0);
        arc(out, c, r, 0.0, TAU - 0.001, w, t.palette.border);
        if f > 0.002 {
            arc(
                out,
                c,
                r,
                -PI / 2.0,
                TAU * f.min(0.999),
                w,
                t.palette.accent,
            );
        }
        let pct = format!("{:.0}%", f * 100.0);
        self.ui
            .describe(Role::ProgressBar, "", &b, |n| n.value = Some(pct.clone()));
        if self.label {
            let s = t.typography.caption;
            let tw =
                self.ui
                    .metrics()
                    .width_styled(&pct, s.size, 600, alloy_ui_core::FontFamily::Sans);
            out.push(DrawCommand::Text {
                origin: Point::new(c.x - tw / 2.0, c.y - s.line_height / 2.0),
                content: pct,
                size: s.size,
                color: t.palette.text,
                max_width: None,
                weight: 600,
                family: alloy_ui_core::FontFamily::Sans,
                line_height: None,
            });
        }
    }
}
