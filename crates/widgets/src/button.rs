//! Push button.

use crate::a11y::Role;
use crate::{
    ui::Ui,
    util::{self, focus_ring, key_of},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Size, Widget,
};
use alloy_ui_layout::Length;

/// Visual weight of a [`Button`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    /// The one main action of a view: filled with the accent.
    Primary,
    /// Everyday action: outlined.
    #[default]
    Secondary,
    /// Low emphasis: no fill until hovered.
    Ghost,
    /// A row in a sidebar or settings list: left-aligned, no outline. Mark the
    /// current page with [`Button::selected`].
    Nav,
}

/// A button that emits a message when clicked or activated with Enter/Space.
/// Without `on_press` it is disabled.
pub struct Button<M> {
    ui: Ui,
    label: String,
    on_press: Option<M>,
    variant: Variant,
    id: Option<u64>,
    width: Length,
    selected: bool,
    tint: Option<alloy_ui_core::Color>,
    hover_tint: Option<alloy_ui_core::Color>,
}

impl<M: Clone + 'static> Button<M> {
    /// A button with `label`.
    pub fn new(ui: &Ui, label: impl Into<String>) -> Self {
        Self {
            ui: ui.clone(),
            label: label.into(),
            on_press: None,
            variant: Variant::default(),
            id: None,
            width: Length::Shrink,
            selected: false,
            tint: None,
            hover_tint: None,
        }
    }
    /// Message to emit on activation; enables the button.
    pub fn on_press(mut self, m: M) -> Self {
        self.on_press = Some(m);
        self
    }
    /// Visual weight.
    pub fn variant(mut self, v: Variant) -> Self {
        self.variant = v;
        self
    }
    /// Fill with `c` instead of the theme colour. It turns lighter on hover and darker while
    /// pressed on its own, and the label switches to whichever of white or near-black reads
    /// better. Disabled buttons ignore it. Override the hover colour with
    /// [`hover_tint`](Self::hover_tint).
    pub fn tint(mut self, c: alloy_ui_core::Color) -> Self {
        self.tint = Some(c);
        self
    }
    /// The fill while hovered, for a [`tint`](Self::tint)ed button.
    pub fn hover_tint(mut self, c: alloy_ui_core::Color) -> Self {
        self.hover_tint = Some(c);
        self
    }
    /// Mark this button as the current choice (only [`Variant::Nav`] shows it).
    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }
    /// Make the button keyboard-focusable under `id` (unique per window).
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
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
}

impl<M: Clone + 'static> Widget<M> for Button<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.label;
        let w = self.ui.metrics().width(&self.label, s.size);
        Size::new(w + 2.0 * t.spacing.md, s.line_height + 2.0 * t.spacing.sm)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let enabled = self.on_press.is_some();
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                if enabled {
                    self.ui.set_pressed(Some(key_of(&b)));
                    if self.id.is_some() {
                        self.ui.set_focus(self.id);
                    }
                }
                true
            }
            Event::MouseReleased(MouseButton::Left) => {
                let was = self.ui.pressed() == Some(key_of(&b));
                if was && b.contains(cursor) {
                    if let Some(m) = &self.on_press {
                        out.push(m.clone());
                    }
                }
                was
            }
            Event::KeyPressed(Key::Enter, _) if enabled && self.ui.is_focused(self.id) => {
                out.extend(self.on_press.clone());
                true
            }
            Event::KeyPressed(Key::Character(c), _)
                if enabled && c == " " && self.ui.is_focused(self.id) =>
            {
                out.extend(self.on_press.clone());
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        self.ui.describe(Role::Button, &self.label, &b, |n| {
            n.id = self.id;
            n.enabled = self.on_press.is_some();
        });
        let enabled = self.on_press.is_some();
        let hover = enabled && self.ui.hovered(&b);
        let down = enabled && self.ui.is_pressed(&b) && hover;
        let h = self.ui.ease(Ui::anim_key(&b, 2), hover as u8 as f32);
        // Press is eased too: quick in, a touch slower out, so a tap reads as
        // a change of state rather than a flicker.
        let pr = self.ui.ease_with(
            Ui::anim_key(&b, 8),
            down as u8 as f32,
            if down { 45 } else { 120 },
            t.motion.easing,
        );
        let overlay = |base: alloy_ui_core::Color| {
            util::over(
                util::over(base, p.hover.with_alpha(p.hover.a * h * (1.0 - pr))),
                p.pressed.with_alpha(p.pressed.a * pr),
            )
        };
        let (fill, border, text) = match (self.variant, enabled) {
            _ if self.tint.is_some() && enabled => {
                let base = self.tint.unwrap_or(p.accent);
                let (white, black) = (
                    alloy_ui_core::Color::hex(0xFFFFFF),
                    alloy_ui_core::Color::hex(0x101114),
                );
                let hot = self
                    .hover_tint
                    .unwrap_or_else(|| util::mix(base, white, 0.18));
                let fill = util::mix(util::mix(base, hot, h), util::mix(base, black, 0.2), pr);
                let label = if alloy_ui_theme::contrast_ratio(base, white)
                    >= alloy_ui_theme::contrast_ratio(base, black)
                {
                    white
                } else {
                    black
                };
                (fill, None, label)
            }
            (Variant::Primary, true) => (
                util::mix(util::mix(p.accent, p.accent_hover, h), p.accent_pressed, pr),
                None,
                p.on_accent,
            ),
            (Variant::Primary, false) => (p.border, None, p.text_disabled),
            (Variant::Secondary, e) => (
                if e {
                    overlay(p.surface_raised)
                } else {
                    p.surface
                },
                Some(Border {
                    color: if e { p.border_control } else { p.border },
                    width: 1.0,
                }),
                if e { p.text } else { p.text_disabled },
            ),
            (Variant::Nav, e) => (
                if self.selected {
                    util::over(p.selection, p.hover.with_alpha(p.hover.a * h * 0.5))
                } else if e {
                    overlay(p.surface.with_alpha(0.0))
                } else {
                    p.surface.with_alpha(0.0)
                },
                None,
                if e { p.text } else { p.text_disabled },
            ),
            (Variant::Ghost, e) => (
                if e {
                    overlay(p.surface.with_alpha(0.0))
                } else {
                    p.surface.with_alpha(0.0)
                },
                None,
                if e { p.text } else { p.text_disabled },
            ),
        };
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        let ring = self.ui.ease(
            Ui::anim_key(&b, 9),
            self.ui.is_focused(self.id) as u8 as f32,
        );
        if ring > 0.01 {
            out.push(focus_ring(
                b,
                t.radii.medium,
                p.focus_ring.with_alpha(p.focus_ring.a * ring),
            ));
        }
        out.push(DrawCommand::Quad {
            bounds: b,
            fill,
            radius: t.radii.medium,
            border,
            shadow: None,
        });
        let s = t.typography.label;
        let nav = self.variant == Variant::Nav;
        let weight = if nav && self.selected {
            600
        } else {
            s.weight.0
        };
        let w = self.ui.metrics().width_styled(
            &self.label,
            s.size,
            weight,
            alloy_ui_core::FontFamily::Sans,
        );
        let x = if nav {
            b.origin.x + t.spacing.md
        } else {
            b.origin.x + (b.size.width - w) / 2.0
        };
        out.push(DrawCommand::Text {
            family: alloy_ui_core::FontFamily::Sans,
            weight,
            line_height: None,
            origin: Point::new(x, b.origin.y + (b.size.height - s.line_height) / 2.0 + pr),
            content: self.label.clone(),
            size: s.size,
            color: text,
            max_width: None,
        });
    }
}
