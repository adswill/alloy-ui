//! Context menu: a floating list of commands opened by right click or the menu key.

use crate::{
    ui::Ui,
    util::{self, label_cmd},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use std::rc::Rc;

/// One entry of a [`ContextMenu`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MenuItem {
    /// Command name.
    pub label: String,
    /// Shortcut hint shown dim on the right (`"Ctrl+C"`); display only.
    pub shortcut: String,
    /// Greyed out and not selectable.
    pub disabled: bool,
    /// Destructive: drawn in the danger colour.
    pub danger: bool,
    /// A hairline divider instead of a command.
    pub separator: bool,
}

impl MenuItem {
    /// A command.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }
    /// A divider.
    pub fn separator() -> Self {
        Self {
            separator: true,
            ..Self::default()
        }
    }
    /// Show a shortcut hint.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = s.into();
        self
    }
    /// Grey it out.
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
    /// Mark as destructive.
    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }
    fn selectable(&self) -> bool {
        !self.separator && !self.disabled
    }
}

const PAD: f32 = 4.0;
const SEP_H: f32 = 9.0;

/// Where a menu panel and its rows sit.
pub(crate) struct MenuGeom {
    pub panel: Rectangle,
    pub rows: Vec<Rectangle>,
}

impl MenuGeom {
    pub(crate) fn item_at(&self, items: &[MenuItem], p: Point) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.contains(p))
            .filter(|&i| items[i].selectable())
    }
}

/// Size and place a menu with its top-left (or top-right when `right`) at `at`,
/// flipped and nudged to stay inside the window.
pub(crate) fn menu_geom(ui: &Ui, items: &[MenuItem], at: Point, right: bool) -> MenuGeom {
    let t = ui.theme();
    let s = t.typography.body;
    let row_h = s.line_height + 2.0 * t.spacing.xs;
    let m = ui.metrics();
    let inner = items
        .iter()
        .map(|i| {
            let sc = if i.shortcut.is_empty() {
                0.0
            } else {
                t.spacing.xl + m.width(&i.shortcut, t.typography.caption.size)
            };
            m.width(&i.label, s.size) + sc
        })
        .fold(0.0, f32::max);
    let w = (inner + 2.0 * (t.spacing.md + PAD)).max(188.0);
    let h: f32 = items
        .iter()
        .map(|i| if i.separator { SEP_H } else { row_h })
        .sum::<f32>()
        + 2.0 * PAD;
    let vp = ui.viewport();
    let mut x = if right { at.x - w } else { at.x };
    x = x
        .min(vp.origin.x + vp.size.width - w - 4.0)
        .max(vp.origin.x + 4.0);
    let mut y = at.y;
    if y + h > vp.origin.y + vp.size.height - 4.0 && y - h >= vp.origin.y + 4.0 {
        y -= h + 4.0; // flip above the anchor
    }
    y = y
        .min(vp.origin.y + vp.size.height - h - 4.0)
        .max(vp.origin.y + 4.0);
    let mut cy = y + PAD;
    let rows = items
        .iter()
        .map(|i| {
            let rh = if i.separator { SEP_H } else { row_h };
            let r = Rectangle::new(Point::new(x + PAD, cy), Size::new(w - 2.0 * PAD, rh));
            cy += rh;
            r
        })
        .collect();
    MenuGeom {
        panel: Rectangle::new(Point::new(x, y), Size::new(w, h)),
        rows,
    }
}

/// Draw commands for a menu panel with `hot` highlighted.
pub(crate) fn menu_cmds(
    ui: &Ui,
    items: &[MenuItem],
    g: &MenuGeom,
    hot: Option<usize>,
) -> Vec<DrawCommand> {
    let t = ui.theme();
    let p = &t.palette;
    let s = t.typography.body;
    let mut c = vec![DrawCommand::Quad {
        bounds: g.panel,
        fill: p.surface_raised,
        radius: t.radii.medium,
        border: Some(Border {
            color: p.border,
            width: 1.0,
        }),
        shadow: Some(t.elevation.medium),
    }];
    for (i, (item, r)) in items.iter().zip(&g.rows).enumerate() {
        if item.separator {
            c.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(r.origin.x + t.spacing.sm, r.origin.y + r.size.height / 2.0),
                    Size::new(r.size.width - 2.0 * t.spacing.sm, 1.0),
                ),
                fill: p.border,
                radius: 0.0,
                border: None,
                shadow: None,
            });
            continue;
        }
        if hot == Some(i) {
            c.push(DrawCommand::Quad {
                bounds: *r,
                fill: p.selection,
                radius: t.radii.small,
                border: None,
                shadow: None,
            });
        }
        let color = if item.disabled {
            p.text_disabled
        } else if item.danger {
            p.danger
        } else {
            p.text
        };
        c.push(label_cmd(
            Point::new(
                r.origin.x + t.spacing.md - PAD + 4.0,
                r.origin.y + (r.size.height - s.line_height) / 2.0,
            ),
            item.label.clone(),
            &s,
            400,
            color,
        ));
        if !item.shortcut.is_empty() {
            let cap = t.typography.caption;
            let w = ui.metrics().width(&item.shortcut, cap.size);
            c.push(label_cmd(
                Point::new(
                    r.origin.x + r.size.width - t.spacing.md + PAD - 4.0 - w,
                    r.origin.y + (r.size.height - cap.line_height) / 2.0,
                ),
                item.shortcut.clone(),
                &cap,
                400,
                if item.disabled {
                    p.text_disabled
                } else {
                    p.text_secondary
                },
            ));
        }
    }
    c
}

/// The next selectable row after `from` in direction `dir` (wrapping).
pub(crate) fn step(items: &[MenuItem], from: Option<usize>, dir: i32) -> Option<usize> {
    let n = items.len() as i32;
    if !items.iter().any(MenuItem::selectable) {
        return None;
    }
    let mut i = match from {
        Some(f) => f as i32,
        None if dir > 0 => -1,
        None => n,
    };
    loop {
        i = (i + dir).rem_euclid(n);
        if items[i as usize].selectable() {
            return Some(i as usize);
        }
    }
}

const K_X: u64 = 0xC7_0001 << 20;
const K_Y: u64 = 0xC7_0002 << 20;
const K_HOT: u64 = 0xC7_0003 << 20;

/// Wraps a child so that right click (or the menu key, or Shift+F10, while the pointer is over
/// it) opens a floating menu of commands at the pointer. Up/Down move, Enter picks, Escape or a
/// click elsewhere closes. The menu is drawn on the overlay layer and kept inside the window.
pub struct ContextMenu<M> {
    ui: Ui,
    id: u64,
    items: Vec<MenuItem>,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
    child: Vec<Element<M>>,
}

impl<M: 'static> ContextMenu<M> {
    /// Attach `items` to `child`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, child: Element<M>, items: Vec<MenuItem>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            items,
            on_select: None,
            child: vec![child],
        }
    }
    /// Called with the index (into `items`) of the picked command.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn is_open(&self) -> bool {
        self.ui.popup() == Some(self.id)
    }
    fn open_at(&self, p: Point) {
        self.ui.set_slot(self.id ^ K_X, Some((p.x * 16.0) as i64));
        self.ui.set_slot(self.id ^ K_Y, Some((p.y * 16.0) as i64));
        self.ui.set_slot(self.id ^ K_HOT, None);
        self.ui.set_popup(Some(self.id));
    }
    fn close(&self) {
        self.ui.set_popup(None);
    }
    fn geom(&self) -> MenuGeom {
        let x = self.ui.slot(self.id ^ K_X).unwrap_or(0) as f32 / 16.0;
        let y = self.ui.slot(self.id ^ K_Y).unwrap_or(0) as f32 / 16.0;
        menu_geom(&self.ui, &self.items, Point::new(x, y), false)
    }
    fn keyboard_hot(&self) -> Option<usize> {
        self.ui.slot(self.id ^ K_HOT).map(|v| v as usize)
    }
}

impl<M: 'static> Widget<M> for ContextMenu<M> {
    fn style(&self) -> Style {
        self.child[0].0.style()
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        util::arrange_widget(self, bounds)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        if self.is_open() {
            let g = self.geom();
            let pick = |i: usize, out: &mut Vec<M>| {
                if let Some(f) = &self.on_select {
                    out.push(f(i));
                }
            };
            match event {
                Event::MousePressed(btn) => {
                    if let Some(p) = self.ui.take_hit(self.id) {
                        self.close();
                        if let Some(i) = g.item_at(&self.items, p) {
                            pick(i, out);
                        }
                        return true;
                    }
                    // A press anywhere else dismisses; only a right click on the
                    // child falls through to reopen at the new spot.
                    self.close();
                    if *btn == MouseButton::Left {
                        return true;
                    }
                }
                Event::KeyPressed(Key::Escape, _) => {
                    self.close();
                    return true;
                }
                Event::KeyPressed(Key::Enter, _) => {
                    let hot = self
                        .keyboard_hot()
                        .or_else(|| g.item_at(&self.items, self.ui.cursor()));
                    self.close();
                    if let Some(i) = hot {
                        pick(i, out);
                    }
                    return true;
                }
                Event::KeyPressed(Key::Named(n), _) => {
                    let cur = self
                        .keyboard_hot()
                        .or_else(|| g.item_at(&self.items, self.ui.cursor()));
                    let next = match n.as_str() {
                        "ArrowDown" => step(&self.items, cur, 1),
                        "ArrowUp" => step(&self.items, cur, -1),
                        "Home" => step(&self.items, None, 1),
                        "End" => step(&self.items, None, -1),
                        _ => return false,
                    };
                    self.ui.set_slot(self.id ^ K_HOT, next.map(|i| i as i64));
                    return true;
                }
                Event::KeyPressed(..) => {
                    self.close();
                }
                Event::CursorMoved(_) => self.ui.set_slot(self.id ^ K_HOT, None),
                _ => {}
            }
        }
        match event {
            Event::MousePressed(MouseButton::Right) if b.contains(cursor) => {
                self.open_at(cursor);
                true
            }
            Event::KeyPressed(Key::Named(n), m)
                if (n == "ContextMenu" || (n == "F10" && m.shift))
                    && b.contains(self.ui.cursor()) =>
            {
                self.open_at(self.ui.cursor());
                true
            }
            _ => util::dispatch(&mut self.child, event, layout, cursor, out),
        }
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.child, layout, out);
        if self.is_open() {
            let g = self.geom();
            let hot = self
                .keyboard_hot()
                .or_else(|| g.item_at(&self.items, self.ui.cursor()));
            let cmds = menu_cmds(&self.ui, &self.items, &g, hot);
            self.ui.overlay(Some(self.id), Some(g.panel), cmds);
        }
    }
}
