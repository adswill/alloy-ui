//! Surface effects wrapped around one child: hover lift and tilt, press
//! ripple and glow, inset wells, film grain and frosted glass.
//!
//! One [`Fx`] carries any combination of [`Layer`]s. Everything is drawn from
//! existing primitives plus the renderer's radial gradient, inner shadow,
//! grain and backdrop blur, so nothing here needs a render layer, and nothing
//! keeps the app ticking once the pointer stops moving.

use crate::{
    ui::Ui,
    util,
    xform::{transform, Xf},
};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, Event, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_theme::Appearance;

/// One visual effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Layer {
    /// Rises 2 px and deepens its shadow while hovered.
    Lift,
    /// Hover depth: grows 1.2%, the shadow leans away from the pointer and a
    /// faint highlight follows it. Think of a card catching the light.
    Tilt,
    /// A soft circle spreads from the press point and fades. `None` uses the
    /// text colour at low strength.
    Ripple(Option<Color>),
    /// While pressed, the accent colour rims the inside edge.
    Glow,
    /// A recessed well: a soft shadow inside the top edge.
    Inset,
    /// Fine film grain of this strength (0.02 to 0.05 is the usable range).
    Grain(f32),
    /// Frosted glass: what is behind is blurred by this many logical pixels
    /// and washed with the surface colour. `None` tint uses the theme.
    Frosted(f32, Option<Color>),
    /// Keeps the app ticking so the view can read the clock (used by
    /// continuously flowing gradients).
    Keepalive,
}

/// A child with [`Layer`]s.
pub struct Fx<M> {
    ui: Ui,
    key: u64,
    radius: f32,
    layers: Vec<Layer>,
    child: Vec<Element<M>>,
}

impl<M: 'static> Fx<M> {
    /// Wrap `child`. `key` identifies the effect across view rebuilds;
    /// `radius` is the child's corner radius, so overlays follow its shape.
    pub fn new(ui: &Ui, key: u64, radius: f32, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            key,
            radius,
            layers: Vec::new(),
            child: vec![child],
        }
    }
    /// Add an effect.
    pub fn layer(mut self, l: Layer) -> Self {
        self.layers.push(l);
        self
    }
    /// Add several effects.
    pub fn layers(mut self, l: impl IntoIterator<Item = Layer>) -> Self {
        self.layers.extend(l);
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn has(&self, f: impl Fn(&Layer) -> bool) -> bool {
        self.layers.iter().any(f)
    }
    fn k(&self, salt: u64) -> u64 {
        self.key ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }
    fn dark(&self) -> bool {
        self.ui.theme().appearance == Appearance::Dark
    }
    fn is_pressed_flag(&self) -> bool {
        self.ui.memo(self.k(3))[3] > 0.5
    }
}

/// How long a ripple takes.
const RIPPLE_MS: u64 = 520;

impl<M: 'static> Widget<M> for Fx<M> {
    fn style(&self) -> Style {
        let inner = self.child[0].0.style();
        Style {
            width: inner.width,
            height: inner.height,
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        Layout {
            bounds,
            children: vec![self.child[0].0.arrange(bounds)],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        // Observe before the child gets the event: buttons consume presses.
        match event {
            Event::MousePressed(MouseButton::Left) if layout.bounds.contains(cursor) => {
                let b = layout.bounds;
                self.ui.set_memo(
                    self.k(3),
                    [cursor.x - b.origin.x, cursor.y - b.origin.y, 1.0, 1.0],
                );
                self.ui.restart_clock(self.k(4));
            }
            Event::MouseReleased(MouseButton::Left) => {
                let mut m = self.ui.memo(self.k(3));
                m[3] = 0.0;
                self.ui.set_memo(self.k(3), m);
            }
            _ => {}
        }
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        self.has(|l| matches!(l, Layer::Keepalive)) || util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let cursor = self.ui.cursor();
        let hover = b.contains(cursor) && self.ui.pressed().is_none();
        let r = self.radius;
        let dark = self.dark();

        let lift_on = self.has(|l| matches!(l, Layer::Lift));
        let tilt_on = self.has(|l| matches!(l, Layer::Tilt));
        let lift = if lift_on {
            self.ui
                .spring(self.k(1), hover as u8 as f32, crate::Spring::SNAPPY)
        } else {
            0.0
        };
        let tilt = if tilt_on {
            self.ui
                .spring(self.k(2), hover as u8 as f32, crate::Spring::GENTLE)
        } else {
            0.0
        };
        let c = b.center();
        let xf = Xf {
            alpha: 1.0,
            scale: 1.0 + 0.012 * tilt,
            origin: c,
            dx: 0.0,
            dy: -2.0 * lift,
        };

        let mut under: Vec<DrawCommand> = Vec::new();
        let mut over: Vec<DrawCommand> = Vec::new();

        // Depth: a soft shadow that grows with lift and leans away from the pointer on tilt.
        let depth = lift.max(tilt).clamp(0.0, 1.0);
        if depth > 0.003 {
            let mut sh = t.elevation.medium;
            sh.color.a *= depth;
            if tilt_on {
                let nx = ((cursor.x - c.x) / b.size.width.max(1.0)).clamp(-0.5, 0.5);
                let ny = ((cursor.y - c.y) / b.size.height.max(1.0)).clamp(-0.5, 0.5);
                sh.offset = Point::new(-nx * 14.0 * tilt, sh.offset.y - ny * 8.0 * tilt);
            }
            under.push(DrawCommand::Quad {
                bounds: b,
                fill: Color::TRANSPARENT,
                radius: r,
                border: None,
                shadow: Some(sh),
            });
        }

        for layer in &self.layers {
            match *layer {
                Layer::Frosted(blur, tint) => {
                    let tint = tint.unwrap_or_else(|| {
                        let mut s = t.palette.surface;
                        s.a = if dark { 0.58 } else { 0.66 };
                        s
                    });
                    under.push(DrawCommand::BackdropBlur {
                        bounds: b,
                        radius: r,
                        blur,
                        tint,
                    });
                    // A hairline edge, then a faint light along the top.
                    over.push(DrawCommand::Quad {
                        bounds: b,
                        fill: Color::TRANSPARENT,
                        radius: r,
                        border: Some(alloy_ui_core::Border {
                            color: if dark {
                                Color::hex(0xFFFFFF).with_alpha(0.10)
                            } else {
                                Color::hex(0xFFFFFF).with_alpha(0.55)
                            },
                            width: 1.0,
                        }),
                        shadow: None,
                    });
                }
                Layer::Inset => over.push(DrawCommand::InnerShadow {
                    bounds: b,
                    radius: r,
                    color: Color::hex(0x000000).with_alpha(if dark { 0.40 } else { 0.11 }),
                    offset: Point::new(0.0, 2.0),
                    blur: 6.0,
                }),
                Layer::Grain(amount) => over.push(DrawCommand::Grain {
                    bounds: b,
                    radius: r,
                    amount,
                    seed: (self.key ^ (self.key >> 29)) as u32,
                }),
                Layer::Tilt if tilt > 0.01 => {
                    let w = b.size.width.max(1.0);
                    let h = b.size.height.max(1.0);
                    let p = Point::new(
                        cursor.x.clamp(b.origin.x, b.origin.x + w),
                        cursor.y.clamp(b.origin.y, b.origin.y + h),
                    );
                    let hi = Color::hex(0xFFFFFF).with_alpha(if dark { 0.07 } else { 0.30 } * tilt);
                    over.push(DrawCommand::RadialGradient {
                        bounds: b,
                        center: p,
                        gradient_radius: w.max(h) * 0.85,
                        from: hi,
                        to: hi.with_alpha(0.0),
                        radius: r,
                    });
                }
                Layer::Glow => {
                    let g = self.ui.animate(
                        self.k(5),
                        self.is_pressed_flag() as u8 as f32,
                        crate::Transition::ms(t.motion.fast_ms),
                    );
                    if g > 0.004 {
                        over.push(DrawCommand::InnerShadow {
                            bounds: b,
                            radius: r,
                            color: t.palette.accent.with_alpha(0.55 * g),
                            offset: Point::new(0.0, 0.0),
                            blur: 10.0,
                        });
                    }
                }
                _ => {}
            }
        }

        // Ripple: only after a press has started the clock.
        if let Some(Layer::Ripple(col)) = self.layers.iter().find(|l| matches!(l, Layer::Ripple(_)))
        {
            let m = self.ui.memo(self.k(3));
            if m[2] > 0.5 {
                let (el, running) = self.ui.clock(self.k(4), RIPPLE_MS);
                if running {
                    let p = el as f32 / RIPPLE_MS as f32;
                    let grow = alloy_ui_theme::Easing::QUART_OUT.at(p);
                    let fade = 1.0
                        - alloy_ui_theme::Easing::CUBIC_IN.at(((p - 0.25) / 0.75).clamp(0.0, 1.0));
                    let origin = Point::new(b.origin.x + m[0], b.origin.y + m[1]);
                    // Far enough to cover the whole control by the end.
                    let far = [
                        (0.0, 0.0),
                        (b.size.width, 0.0),
                        (0.0, b.size.height),
                        (b.size.width, b.size.height),
                    ]
                    .iter()
                    .map(|(x, y)| ((m[0] - x).powi(2) + (m[1] - y).powi(2)).sqrt())
                    .fold(0.0f32, f32::max);
                    let base = col.unwrap_or(if dark {
                        Color::hex(0xFFFFFF)
                    } else {
                        t.palette.text
                    });
                    let a = base.a * if dark { 0.16 } else { 0.13 } * fade;
                    over.push(DrawCommand::RadialGradient {
                        bounds: b,
                        center: origin,
                        gradient_radius: (far * grow).max(1.0),
                        from: base.with_alpha(a * 0.55),
                        to: base.with_alpha(a),
                        radius: r,
                    });
                }
            }
        }

        let mut tmp = DrawList::default();
        util::draw_children(&self.child, layout, &mut tmp);
        let moving = !xf.is_identity();
        let push = |c: DrawCommand, out: &mut DrawList| {
            out.push(if moving { transform(c, &xf) } else { c });
        };
        for c in under {
            push(c, out);
        }
        for c in tmp.0 {
            push(c, out);
        }
        for c in over {
            push(c, out);
        }
    }
}

trait Centre {
    fn center(&self) -> Point;
}
impl Centre for Rectangle {
    fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width * 0.5,
            self.origin.y + self.size.height * 0.5,
        )
    }
}
