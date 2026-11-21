//! The window furniture the facade adds around every page: keyboard shortcuts and the toast layer.

use crate::{
    keys::{self, Hotkey},
    palette,
};
use alloy_ui_core::{
    style::Style, Color, DrawCommand, DrawList, Element, Event, FontFamily, Key, Layout,
    MouseButton, Point, Rectangle, Size, Widget,
};
use alloy_ui_platform::shortcut::Shortcut;
use alloy_ui_widgets::Ui;

const H: f32 = 44.0;
const GAP: f32 = 8.0;
const BOTTOM: f32 = 24.0;
const PAD: f32 = 16.0;
const FADE_IN: f32 = 220.0;
const FADE_OUT: f32 = 260.0;
const ROWS: usize = 8;
const ROW_H: f32 = 36.0;
const INPUT_H: f32 = 52.0;

/// Wraps the page: offers key presses to the page first (so a focused text field keeps its own
/// `mod+z`), then to the app's shortcuts, and draws toasts above everything.
pub(crate) struct Chrome<M> {
    ui: Ui,
    child: Vec<Element<M>>,
    keys: Vec<(Shortcut, M)>,
    /// Everything the command palette lists: title, the keys that trigger it, what it sends.
    entries: Vec<(String, Option<String>, M)>,
    undo: Option<M>,
    /// Every command the desktop can call by name: menu items, tray items, global hotkeys.
    named: Vec<(String, M)>,
}

impl<M: Clone + 'static> Chrome<M> {
    /// Palette rows that match the query, best first: indices into `entries`.
    fn matches(&self) -> Vec<usize> {
        let titles: Vec<String> = self.entries.iter().map(|e| e.0.clone()).collect();
        let mut hits = palette::with(|p| palette::rank(&p.query, &titles));
        hits.truncate(ROWS);
        hits
    }

    pub(crate) fn new(ui: &Ui, page: Element<M>, keys: Vec<Hotkey<M>>, undo: Option<M>) -> Self {
        let named = keys
            .iter()
            .filter_map(|h| {
                let id =
                    h.id.clone()
                        .or_else(|| h.label.as_ref().map(|l| format!("cmd:{l}")))?;
                Some((id, h.msg.clone()))
            })
            .collect();
        Self {
            ui: ui.clone(),
            named,
            child: vec![page],
            entries: keys
                .iter()
                .map(|h| {
                    let pretty = (!h.keys.is_empty()).then(|| palette::pretty(&h.keys));
                    let title = h
                        .label
                        .clone()
                        .or_else(|| pretty.clone())
                        .unwrap_or_default();
                    (title, pretty, h.msg.clone())
                })
                .filter(|e| !e.0.is_empty())
                .collect(),
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

/// The palette panel for `n` result rows, centred near the top of `bounds`.
fn panel(bounds: Rectangle, n: usize) -> Rectangle {
    let w = 560.0_f32.min(bounds.size.width - 32.0);
    let body = if n == 0 {
        ROW_H + 12.0
    } else {
        n as f32 * ROW_H + 12.0
    };
    let top = (bounds.size.height * 0.14).max(32.0);
    Rectangle::new(
        Point::new(
            bounds.origin.x + (bounds.size.width - w) / 2.0,
            bounds.origin.y + top,
        ),
        Size::new(w, INPUT_H + 1.0 + body),
    )
}

fn row_rect(p: Rectangle, i: usize) -> Rectangle {
    Rectangle::new(
        Point::new(
            p.origin.x + 6.0,
            p.origin.y + INPUT_H + 1.0 + 6.0 + i as f32 * ROW_H,
        ),
        Size::new(p.size.width - 12.0, ROW_H),
    )
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

/// Where every toast sits in `bounds` right now, and how visible it is.
fn place(ui: &Ui, bounds: Rectangle, can_undo: bool) -> Vec<(Placed, String, bool, u8)> {
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
            let dot = if t.tone > 0 { 18.0 } else { 0.0 };
            let w = (tw
                + dot
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
                t.tone,
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
        if let Event::KeyPressed(key, mods) = event {
            if Shortcut::parse("mod+k").is_some_and(|k| k.matches(key, mods))
                && !self.entries.is_empty()
            {
                palette::toggle();
                return true;
            }
        }
        if palette::is_open() {
            let hits = self.matches();
            let p = panel(layout.bounds, hits.len());
            let last = hits.len().saturating_sub(1);
            match event {
                Event::KeyPressed(Key::Escape, _) => palette::close(),
                Event::KeyPressed(Key::Enter, _) => {
                    let sel = palette::with(|s| s.sel).min(last);
                    if let Some(&i) = hits.get(sel) {
                        out.push(self.entries[i].2.clone());
                    }
                    palette::close();
                }
                Event::KeyPressed(Key::Named(n), _) if n == "ArrowDown" => {
                    palette::with(|s| s.sel = (s.sel + 1).min(last))
                }
                Event::KeyPressed(Key::Named(n), _) if n == "ArrowUp" => {
                    palette::with(|s| s.sel = s.sel.saturating_sub(1))
                }
                Event::KeyPressed(Key::Backspace, _) => palette::with(|s| {
                    s.query.pop();
                    s.sel = 0;
                }),
                Event::TextInput(t) => palette::with(|s| {
                    s.query.extend(t.chars().filter(|c| !c.is_control()));
                    s.sel = 0;
                }),
                Event::CursorMoved(c) => {
                    if let Some(i) = (0..hits.len()).find(|&i| row_rect(p, i).contains(*c)) {
                        palette::with(|s| s.sel = i);
                    }
                    return true;
                }
                Event::MousePressed(MouseButton::Left) => {
                    match (0..hits.len()).find(|&i| row_rect(p, i).contains(cursor)) {
                        Some(i) => {
                            out.push(self.entries[hits[i]].2.clone());
                            palette::close();
                        }
                        None if !p.contains(cursor) => palette::close(),
                        None => {}
                    }
                }
                Event::KeyPressed(..) | Event::KeyReleased(..) | Event::MouseReleased(_) => {}
                Event::Scrolled { .. } | Event::ImePreedit(_) | Event::CursorLeft => {}
                _ => return self.on_idle(event, layout, cursor, out),
            }
            return true;
        }
        self.on_idle(event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        keys::toasts(|ts, _| !ts.is_empty()) || self.child[0].0.animating()
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.draw_page(layout, out);
        self.draw_toasts(layout, out);
        self.draw_palette(layout, out);
    }
}

impl<M: Clone + 'static> Chrome<M> {
    /// Everything that is not palette input: toast ticks and clicks, the page, then shortcuts.
    fn on_idle(&mut self, event: &Event, layout: &Layout, cursor: Point, out: &mut Vec<M>) -> bool {
        for id in keys::take_invoked() {
            if let Some((_, msg)) = self.named.iter().find(|(n, _)| *n == id) {
                out.push(msg.clone());
            }
        }
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
    fn draw_page(&self, layout: &Layout, out: &mut DrawList) {
        for (c, l) in self.child.iter().zip(&layout.children) {
            c.0.draw(l, out);
        }
    }
    fn draw_toasts(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let size = t.typography.body.size;
        let hover = self.ui.cursor();
        for (p, text, error, tone) in place(&self.ui, layout.bounds, self.undo.is_some()) {
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
            let lead = if tone > 0 { 18.0 } else { 0.0 };
            if tone > 0 {
                let c = match tone {
                    1 => t.palette.success,
                    2 => t.palette.warning,
                    _ => t.palette.accent,
                };
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(p.rect.origin.x + PAD, p.rect.origin.y + H / 2.0 - 4.0),
                        Size::new(8.0, 8.0),
                    ),
                    fill: fade(c),
                    radius: 4.0,
                    border: None,
                    shadow: None,
                });
            }
            out.push(DrawCommand::Text {
                origin: Point::new(p.rect.origin.x + PAD + lead, ty),
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

    fn draw_palette(&self, layout: &Layout, out: &mut DrawList) {
        if !palette::is_open() {
            return;
        }
        let t = self.ui.theme();
        let b = layout.bounds;
        let size = t.typography.body.size;
        let hits = self.matches();
        let p = panel(b, hits.len());
        let (query, sel) =
            palette::with(|s| (s.query.clone(), s.sel.min(hits.len().saturating_sub(1))));
        let text = |origin: Point, content: String, size: f32, color: Color, weight: u16| {
            DrawCommand::Text {
                origin,
                content,
                size,
                color,
                max_width: None,
                weight,
                family: FontFamily::Sans,
                line_height: None,
            }
        };
        // A light scrim: enough to separate the panel from the page, not a modal blackout.
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: Color::hex(0x000000).with_alpha(0.14),
            radius: 0.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: p,
            fill: t.palette.surface_raised,
            radius: t.radii.large,
            border: Some(alloy_ui_core::Border {
                color: t.palette.border,
                width: 1.0,
            }),
            shadow: Some(t.elevation.high),
        });
        let ty = p.origin.y + (INPUT_H - t.typography.body.line_height) / 2.0;
        let tx = p.origin.x + 18.0;
        if query.is_empty() {
            out.push(text(
                Point::new(tx + 4.0, ty),
                "Search commands".into(),
                size,
                t.palette.text_secondary,
                400,
            ));
        } else {
            out.push(text(
                Point::new(tx, ty),
                query.clone(),
                size,
                t.palette.text,
                400,
            ));
        }
        let cx = tx + self.ui.metrics().width(&query, size);
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(cx + 1.0, ty + 1.0),
                Size::new(1.5, t.typography.body.line_height - 2.0),
            ),
            fill: t.palette.accent,
            radius: 0.0,
            border: None,
            shadow: None,
        });
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(p.origin.x, p.origin.y + INPUT_H),
                Size::new(p.size.width, 1.0),
            ),
            fill: t.palette.border,
            radius: 0.0,
            border: None,
            shadow: None,
        });
        if hits.is_empty() {
            let r = row_rect(p, 0);
            out.push(text(
                Point::new(
                    r.origin.x + 12.0,
                    r.origin.y + (ROW_H - t.typography.body.line_height) / 2.0,
                ),
                "No matching commands".into(),
                size,
                t.palette.text_secondary,
                400,
            ));
        }
        for (i, &e) in hits.iter().enumerate() {
            let r = row_rect(p, i);
            if i == sel {
                out.push(DrawCommand::Quad {
                    bounds: r,
                    fill: t.palette.selection,
                    radius: t.radii.medium,
                    border: None,
                    shadow: None,
                });
            }
            let ry = r.origin.y + (ROW_H - t.typography.body.line_height) / 2.0;
            let (title, keys, _) = &self.entries[e];
            out.push(text(
                Point::new(r.origin.x + 12.0, ry),
                title.clone(),
                size,
                t.palette.text,
                if i == sel { 500 } else { 400 },
            ));
            if let Some(k) = keys.as_ref().filter(|k| *k != title) {
                let small = size - 1.0;
                let w = self.ui.metrics().width(k, small);
                out.push(text(
                    Point::new(r.origin.x + r.size.width - 12.0 - w, ry + 0.5),
                    k.clone(),
                    small,
                    t.palette.text_secondary,
                    400,
                ));
            }
        }
    }
}
