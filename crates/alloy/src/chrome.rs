//! The window furniture the facade adds around every page: keyboard shortcuts and the toast layer.

use crate::keys::{self, Hotkey};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, Event, FontFamily, Layout, MouseButton,
    Point, Rectangle, Size, Widget,
};
use alloy_ui_platform::shortcut::Shortcut;
use alloy_ui_widgets::Ui;

const H: f32 = 44.0;
const GAP: f32 = 8.0;
const BOTTOM: f32 = 24.0;
const PAD: f32 = 16.0;
const FADE_IN: f32 = 220.0;
const FADE_OUT: f32 = 260.0;

/// Wraps the page: offers key presses to the page first (so a focused text field keeps its own
/// `mod+z`), then to the app's shortcuts, and draws toasts above everything.
pub(crate) struct Chrome<M> {
    ui: Ui,
    child: Vec<Element<M>>,
    keys: Vec<(Shortcut, M)>,
    undo: Option<M>,
}

impl<M: Clone + 'static> Chrome<M> {
    pub(crate) fn new(ui: &Ui, page: Element<M>, keys: Vec<Hotkey<M>>, undo: Option<M>) -> Self {
        Self {
            ui: ui.clone(),
            child: vec![page],
            keys: keys
                .into_iter()
                .filter_map(|h| Shortcut::parse(&h.keys).map(|s| (s, h.msg)))
                .collect(),
            undo,
        }
    }
}

struct Placed {
    id: u64,
    rect: Rectangle,
    undo: Option<Rectangle>,
    alpha: f32,
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Where every toast sits in `bounds` right now, and how visible it is.
fn place(ui: &Ui, bounds: Rectangle, can_undo: bool) -> Vec<(Placed, String, bool)> {
    let size = ui.theme().typography.body.size;
    keys::toasts(|ts, now| {
        let mut y = bounds.origin.y + bounds.size.height - BOTTOM;
        let mut out = Vec::new();
        for t in ts.iter().rev() {
            let age = t.born.map_or(0.0, |b| now.saturating_sub(b) as f32);
            let life = t.secs * 1000.0;
            let a_in = ease_out(age / FADE_IN);
            let a_out = ((life + FADE_OUT - age) / FADE_OUT).clamp(0.0, 1.0);
            let alpha = a_in * a_out;
            let tw = ui.metrics().width(&t.text, size);
            let uw = if t.undo && can_undo {
                ui.metrics().width("Undo", size) + 2.0 * 10.0
            } else {
                0.0
            };
            let w = (tw
                + 2.0 * PAD
                + if t.undo && can_undo {
                    12.0 + uw - PAD + 6.0
                } else {
                    0.0
                })
            .min(bounds.size.width - 32.0);
            let x = bounds.origin.x + (bounds.size.width - w) / 2.0;
            y -= H;
            let slide = (1.0 - a_in) * 14.0;
            let rect = Rectangle::new(Point::new(x, y + slide), Size::new(w, H));
            let undo = (t.undo && can_undo).then(|| {
                Rectangle::new(
                    Point::new(x + w - uw - 8.0, y + slide + 7.0),
                    Size::new(uw, H - 14.0),
                )
            });
            y -= GAP;
            out.push((
                Placed {
                    id: t.id,
                    rect,
                    undo,
                    alpha,
                },
                t.text.clone(),
                t.error,
            ));
        }
        out
    })
}

impl<M: Clone + 'static> Widget<M> for Chrome<M> {
    fn style(&self) -> Style {
        self.child[0].0.style()
    }
    fn measure(&self, available: Size) -> Size {
        self.child[0].0.measure(available)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        alloy_ui_widgets::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        match event {
            Event::Tick(ms) => {
                keys::set_now(*ms);
                keys::toasts(|ts, now| {
                    for t in ts.iter_mut() {
                        t.born.get_or_insert(now);
                    }
                    ts.retain(|t| {
                        t.born
                            .map_or(true, |b| (now - b) as f32 <= t.secs * 1000.0 + FADE_OUT)
                    });
                });
            }
            Event::MousePressed(MouseButton::Left) => {
                for (p, ..) in place(&self.ui, layout.bounds, self.undo.is_some()) {
                    if !p.rect.contains(cursor) {
                        continue;
                    }
                    if p.undo.is_some_and(|u| u.contains(cursor)) {
                        out.extend(self.undo.clone());
                    }
                    keys::toasts(|ts, _| ts.retain(|t| t.id != p.id));
                    return true;
                }
            }
            _ => {}
        }
        for (child, l) in self.child.iter_mut().zip(&layout.children).rev() {
            if child.0.on_event(event, l, cursor, out) {
                return true;
            }
        }
        if let Event::KeyPressed(key, mods) = event {
            if let Some((_, msg)) = self.keys.iter().find(|(s, _)| s.matches(key, mods)) {
                out.push(msg.clone());
                return true;
            }
        }
        false
    }
    fn animating(&self) -> bool {
        keys::toasts(|ts, _| !ts.is_empty()) || self.child[0].0.animating()
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        for (c, l) in self.child.iter().zip(&layout.children) {
            c.0.draw(l, out);
        }
        let t = self.ui.theme();
        let size = t.typography.body.size;
        let hover = self.ui.cursor();
        for (p, text, error) in place(&self.ui, layout.bounds, self.undo.is_some()) {
            if p.alpha <= 0.0 {
                continue;
            }
            let fade = |c: Color| c.with_alpha(c.a * p.alpha);
            let mut shadow = t.elevation.high;
            shadow.color = fade(shadow.color);
            out.push(DrawCommand::Quad {
                bounds: p.rect,
                fill: fade(t.palette.surface_raised),
                radius: t.radii.large,
                border: Some(alloy_ui_core::Border {
                    color: fade(if error {
                        t.palette.danger
                    } else {
                        t.palette.border
                    }),
                    width: 1.0,
                }),
                shadow: Some(shadow),
            });
            let ty = p.rect.origin.y + (H - t.typography.body.line_height) / 2.0;
            out.push(DrawCommand::Text {
                origin: Point::new(p.rect.origin.x + PAD, ty),
                content: text,
                size,
                color: fade(if error {
                    t.palette.danger
                } else {
                    t.palette.text
                }),
                max_width: None,
                weight: 400,
                family: FontFamily::Sans,
                line_height: None,
            });
            if let Some(u) = p.undo {
                if u.contains(hover) {
                    out.push(DrawCommand::Quad {
                        bounds: u,
                        fill: fade(t.palette.selection),
                        radius: t.radii.medium,
                        border: None,
                        shadow: None,
                    });
                }
                out.push(DrawCommand::Text {
                    origin: Point::new(u.origin.x + 10.0, ty),
                    content: "Undo".into(),
                    size,
                    color: fade(t.palette.accent),
                    max_width: None,
                    weight: 600,
                    family: FontFamily::Sans,
                    line_height: None,
                });
            }
        }
    }
}
