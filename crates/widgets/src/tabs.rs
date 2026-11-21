//! Tab strip with overflow menu.

use crate::{
    a11y::Role,
    menu::{menu_cmds, menu_geom, MenuItem},
    ui::Ui,
    util::{self, focus_ring, label_cmd},
};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

const TAB_H: f32 = 38.0;
const OVER_W: f32 = 52.0;
const CLOSE_W: f32 = 22.0;

/// A strip of tabs. The selected tab is semibold with an accent underline. When the labels do
/// not fit, the tabs that cannot be shown collapse into a `+N` menu at the right end (the
/// selected tab is always kept in view). Left and Right switch tabs while the strip has focus;
/// with `on_close`, each tab gets a close button and middle click closes it. It draws only the
/// strip: you show the selected tab's content yourself.
pub struct Tabs<M> {
    ui: Ui,
    id: u64,
    labels: Vec<String>,
    selected: usize,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
    on_close: Option<Rc<dyn Fn(usize) -> M>>,
}

struct Geom {
    tabs: Vec<(usize, Rectangle)>,
    overflow: Option<Rectangle>,
    hidden: Vec<usize>,
}

impl<M: 'static> Tabs<M> {
    /// Tabs labelled `labels`, with `selected` highlighted. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, labels: Vec<String>, selected: usize) -> Self {
        Self {
            ui: ui.clone(),
            id,
            selected: selected.min(labels.len().saturating_sub(1)),
            labels,
            on_select: None,
            on_close: None,
        }
    }
    /// Called with the index of the tab the user picked.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// Give every tab a close button; called with the index to close.
    pub fn on_close(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn widths(&self) -> Vec<f32> {
        let t = self.ui.theme();
        let m = self.ui.metrics();
        let s = t.typography.label;
        let extra = if self.on_close.is_some() {
            CLOSE_W
        } else {
            0.0
        };
        self.labels
            .iter()
            .map(|l| m.width(l, s.size) * 1.06 + 2.0 * t.spacing.md + extra)
            .collect()
    }
    fn geom(&self, b: &Rectangle) -> Geom {
        let w = self.widths();
        let total: f32 = w.iter().sum();
        let (first, avail, overflow) = if total <= b.size.width {
            (0, f32::INFINITY, false)
        } else {
            let avail = (b.size.width - OVER_W).max(0.0);
            let mut first = 0;
            while first < self.selected && w[first..=self.selected].iter().sum::<f32>() > avail {
                first += 1;
            }
            (first, avail, true)
        };
        let mut tabs = Vec::new();
        let mut x = 0.0;
        for (i, wi) in w.iter().enumerate().skip(first) {
            if x + wi > avail && i != self.selected {
                break;
            }
            tabs.push((
                i,
                Rectangle::new(
                    Point::new(b.origin.x + x, b.origin.y),
                    Size::new(*wi, b.size.height),
                ),
            ));
            x += wi;
        }
        let shown: Vec<usize> = tabs.iter().map(|(i, _)| *i).collect();
        let hidden: Vec<usize> = (0..w.len()).filter(|i| !shown.contains(i)).collect();
        Geom {
            tabs,
            overflow: (overflow && !hidden.is_empty()).then(|| {
                Rectangle::new(
                    Point::new(b.origin.x + b.size.width - OVER_W, b.origin.y),
                    Size::new(OVER_W, b.size.height),
                )
            }),
            hidden,
        }
    }
    fn menu_items(&self, g: &Geom) -> Vec<MenuItem> {
        g.hidden
            .iter()
            .map(|&i| MenuItem::new(self.labels[i].clone()))
            .collect()
    }
    fn is_open(&self) -> bool {
        self.ui.popup() == Some(self.id)
    }
    fn close_rect(&self, r: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(r.origin.x + r.size.width - CLOSE_W - 2.0, r.origin.y + 8.0),
            Size::new(CLOSE_W, r.size.height - 16.0),
        )
    }
}

impl<M: 'static> Widget<M> for Tabs<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Fixed(TAB_H))
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(self.widths().iter().sum(), TAB_H)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let g = self.geom(&b);
        let select = |i: usize, out: &mut Vec<M>| {
            if let Some(f) = &self.on_select {
                out.push(f(i));
            }
        };
        match event {
            Event::MousePressed(btn) => {
                if self.is_open() {
                    let items = self.menu_items(&g);
                    if let Some(p) = self.ui.take_hit(self.id) {
                        self.ui.set_popup(None);
                        let mg = menu_geom(
                            &self.ui,
                            &items,
                            Point::new(b.origin.x + b.size.width, b.origin.y + b.size.height),
                            true,
                        );
                        if let Some(k) = mg.item_at(&items, p) {
                            select(g.hidden[k], out);
                        }
                        return true;
                    }
                    self.ui.set_popup(None);
                    if g.overflow.is_some_and(|o| o.contains(cursor)) {
                        return true;
                    }
                }
                if !b.contains(cursor) {
                    return false;
                }
                if *btn == MouseButton::Left {
                    self.ui.set_focus(Some(self.id));
                    if g.overflow.is_some_and(|o| o.contains(cursor)) {
                        self.ui.set_popup(Some(self.id));
                        return true;
                    }
                }
                for (i, r) in &g.tabs {
                    if !r.contains(cursor) {
                        continue;
                    }
                    let on_x = self.on_close.is_some() && self.close_rect(r).contains(cursor);
                    match (btn, &self.on_close) {
                        (MouseButton::Left, Some(c)) if on_x => out.push(c(*i)),
                        (MouseButton::Left, _) => select(*i, out),
                        (MouseButton::Middle, Some(c)) => out.push(c(*i)),
                        _ => return false,
                    }
                    return true;
                }
                false
            }
            Event::KeyPressed(Key::Escape, _) if self.is_open() => {
                self.ui.set_popup(None);
                true
            }
            Event::KeyPressed(Key::Named(n), _) if self.ui.is_focused(Some(self.id)) => {
                let last = self.labels.len().saturating_sub(1);
                let to = match n.as_str() {
                    "ArrowLeft" => self.selected.saturating_sub(1),
                    "ArrowRight" => (self.selected + 1).min(last),
                    "Home" => 0,
                    "End" => last,
                    _ => return false,
                };
                if !self.labels.is_empty() {
                    select(to, out);
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let p = &t.palette;
        let s = t.typography.label;
        self.ui.register(self.id);
        let g = self.geom(&b);
        // The strip's baseline.
        out.push(DrawCommand::Quad {
            bounds: Rectangle::new(
                Point::new(b.origin.x, b.origin.y + b.size.height - 1.0),
                Size::new(b.size.width, 1.0),
            ),
            fill: p.border,
            radius: 0.0,
            border: None,
            shadow: None,
        });
        let focused = self.ui.is_focused(Some(self.id));
        for (i, r) in &g.tabs {
            let sel = *i == self.selected;
            self.ui.describe(Role::Tab, &self.labels[*i], r, |n| {
                n.id = Some(self.id);
                n.checked = Some(sel);
            });
            let hot = self.ui.hovered(r);
            let chip = Rectangle::new(
                Point::new(r.origin.x + 2.0, r.origin.y + 5.0),
                Size::new(r.size.width - 4.0, r.size.height - 11.0),
            );
            if hot && !sel {
                out.push(DrawCommand::Quad {
                    bounds: chip,
                    fill: p.hover,
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            }
            out.push(label_cmd(
                Point::new(
                    r.origin.x + t.spacing.md,
                    r.origin.y + (r.size.height - 1.0 - s.line_height) / 2.0,
                ),
                self.labels[*i].clone(),
                &s,
                if sel { 600 } else { 400 },
                if sel { p.text } else { p.text_secondary },
            ));
            if self.on_close.is_some() && (sel || hot) {
                let c = self.close_rect(r);
                if self.ui.hovered(&c) {
                    out.push(DrawCommand::Quad {
                        bounds: Rectangle::new(
                            Point::new(c.origin.x + 2.0, c.origin.y + (c.size.height - 18.0) / 2.0),
                            Size::new(c.size.width - 4.0, 18.0),
                        ),
                        fill: p.pressed,
                        radius: t.radii.small,
                        border: None,
                        shadow: None,
                    });
                }
                let (cx, cy) = (
                    c.origin.x + c.size.width / 2.0,
                    c.origin.y + c.size.height / 2.0,
                );
                for (a, bb) in [((-3.0, -3.0), (3.0, 3.0)), ((-3.0, 3.0), (3.0, -3.0))] {
                    out.push(DrawCommand::Line {
                        from: Point::new(cx + a.0, cy + a.1),
                        to: Point::new(cx + bb.0, cy + bb.1),
                        width: 1.25,
                        color: p.text_secondary,
                    });
                }
            }
            if sel {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(r.origin.x + t.spacing.sm, r.origin.y + r.size.height - 2.0),
                        Size::new(r.size.width - 2.0 * t.spacing.sm, 2.0),
                    ),
                    fill: p.accent,
                    radius: 1.0,
                    border: None,
                    shadow: None,
                });
                if focused {
                    out.push(focus_ring(chip, t.radii.small, p.focus_ring));
                }
            }
        }
        if let Some(o) = g.overflow {
            let cap = t.typography.caption;
            let hot = self.ui.hovered(&o) || self.is_open();
            if hot {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(o.origin.x + 4.0, o.origin.y + 5.0),
                        Size::new(o.size.width - 8.0, o.size.height - 11.0),
                    ),
                    fill: p.hover,
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            }
            out.push(label_cmd(
                Point::new(
                    o.origin.x + 10.0,
                    o.origin.y + (o.size.height - 1.0 - cap.line_height) / 2.0,
                ),
                format!("+{} \u{25BE}", g.hidden.len()),
                &cap,
                500,
                p.text_secondary,
            ));
            if self.is_open() {
                let items = self.menu_items(&g);
                let mg = menu_geom(
                    &self.ui,
                    &items,
                    Point::new(b.origin.x + b.size.width, b.origin.y + b.size.height),
                    true,
                );
                let hot = mg.item_at(&items, self.ui.cursor());
                let cmds = menu_cmds(&self.ui, &items, &mg, hot);
                self.ui.overlay(Some(self.id), Some(mg.panel), cmds);
            }
        }
    }
}
