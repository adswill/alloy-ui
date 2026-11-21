//! Tree view: nested, expandable, selectable rows.

use crate::util::{self, focus_ring, label_cmd};
use crate::{a11y::Role, ui::Ui};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

const ROW_H: f32 = 28.0;
const INDENT: f32 = 18.0;

/// One node of a [`Tree`]. `id` identifies it across rebuilds, so keep it stable (a database id,
/// a path hash), not an index.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeItem {
    /// Stable identity.
    pub id: u64,
    /// Row text.
    pub label: String,
    /// Child nodes; a node with children shows a chevron.
    pub children: Vec<TreeItem>,
}

impl TreeItem {
    /// A leaf.
    pub fn new(id: u64, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            children: Vec::new(),
        }
    }
    /// Add children.
    pub fn children(mut self, children: impl IntoIterator<Item = TreeItem>) -> Self {
        self.children.extend(children);
        self
    }
}

struct Row<'a> {
    item: &'a TreeItem,
    depth: usize,
    parent: Option<u64>,
}

/// An outline of [`TreeItem`]s. Controlled: you keep which ids are `expanded` and which is
/// `selected`, and get `on_toggle` / `on_select` callbacks. Arrow keys move, Left and Right
/// collapse and expand, Enter or Space toggles a branch.
pub struct Tree<M> {
    ui: Ui,
    items: Vec<TreeItem>,
    expanded: Vec<u64>,
    selected: Option<u64>,
    on_select: Option<Box<dyn Fn(u64) -> M>>,
    on_toggle: Option<Box<dyn Fn(u64) -> M>>,
    id: Option<u64>,
}

impl<M: 'static> Tree<M> {
    /// A tree of root `items`.
    pub fn new(ui: &Ui, items: impl Into<Vec<TreeItem>>) -> Self {
        Self {
            ui: ui.clone(),
            items: items.into(),
            expanded: Vec::new(),
            selected: None,
            on_select: None,
            on_toggle: None,
            id: None,
        }
    }
    /// Ids of the open branches.
    pub fn expanded(mut self, ids: impl IntoIterator<Item = u64>) -> Self {
        self.expanded = ids.into_iter().collect();
        self
    }
    /// The highlighted row.
    pub fn selected(mut self, id: Option<u64>) -> Self {
        self.selected = id;
        self
    }
    /// Called with the id of a row the user picked.
    pub fn on_select(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }
    /// Called with the id of a branch the user opened or closed.
    pub fn on_toggle(mut self, f: impl Fn(u64) -> M + 'static) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }
    /// Make keyboard-focusable under `id`.
    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn open(&self, id: u64) -> bool {
        self.expanded.contains(&id)
    }
    fn rows(&self) -> Vec<Row<'_>> {
        fn walk<'a, M>(
            t: &Tree<M>,
            items: &'a [TreeItem],
            depth: usize,
            parent: Option<u64>,
            out: &mut Vec<Row<'a>>,
        ) {
            for item in items {
                out.push(Row {
                    item,
                    depth,
                    parent,
                });
                if !item.children.is_empty() && t.expanded.contains(&item.id) {
                    walk(t, &item.children, depth + 1, Some(item.id), out);
                }
            }
        }
        let mut out = Vec::new();
        walk(self, &self.items, 0, None, &mut out);
        out
    }
    fn row_rect(b: &Rectangle, i: usize) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + i as f32 * ROW_H),
            Size::new(b.size.width, ROW_H),
        )
    }
}

impl<M: 'static> Widget<M> for Tree<M> {
    fn style(&self) -> Style {
        util::sized(Length::Fill(1), Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(160.0, self.rows().len() as f32 * ROW_H)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let rows = self.rows();
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                let i = ((cursor.y - b.origin.y) / ROW_H) as usize;
                let Some(r) = rows.get(i) else { return true };
                if self.id.is_some() {
                    self.ui.set_focus(self.id);
                }
                let chevron_end = b.origin.x + 8.0 + r.depth as f32 * INDENT + INDENT;
                if !r.item.children.is_empty() && cursor.x < chevron_end {
                    if let Some(f) = &self.on_toggle {
                        out.push(f(r.item.id));
                    }
                } else if let Some(f) = &self.on_select {
                    out.push(f(r.item.id));
                }
                true
            }
            Event::KeyPressed(Key::Named(k), _) if self.ui.is_focused(self.id) => {
                let cur = rows.iter().position(|r| Some(r.item.id) == self.selected);
                let select = |i: usize, out: &mut Vec<M>| {
                    if let (Some(f), Some(r)) = (&self.on_select, rows.get(i)) {
                        out.push(f(r.item.id));
                    }
                };
                let toggle = |id: u64, out: &mut Vec<M>| {
                    if let Some(f) = &self.on_toggle {
                        out.push(f(id));
                    }
                };
                match (k.as_str(), cur.and_then(|i| rows.get(i))) {
                    ("ArrowDown", _) => select(cur.map_or(0, |i| (i + 1).min(rows.len() - 1)), out),
                    ("ArrowUp", _) => select(cur.map_or(0, |i| i.saturating_sub(1)), out),
                    ("Home", _) => select(0, out),
                    ("End", _) if !rows.is_empty() => select(rows.len() - 1, out),
                    ("ArrowRight", Some(r)) if !r.item.children.is_empty() => {
                        if !self.open(r.item.id) {
                            toggle(r.item.id, out)
                        } else if let Some(i) = cur {
                            select(i + 1, out)
                        }
                    }
                    ("ArrowLeft", Some(r)) => {
                        if !r.item.children.is_empty() && self.open(r.item.id) {
                            toggle(r.item.id, out)
                        } else if let Some(pi) = r
                            .parent
                            .and_then(|pid| rows.iter().position(|x| x.item.id == pid))
                        {
                            select(pi, out)
                        }
                    }
                    ("Enter", Some(r)) if !r.item.children.is_empty() => toggle(r.item.id, out),
                    _ => return false,
                }
                true
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let s = t.typography.body;
        let b = layout.bounds;
        let rows = self.rows();
        self.ui.describe(Role::Label, "Tree", &b, |n| {
            n.id = self.id;
        });
        if let Some(id) = self.id {
            self.ui.register(id);
        }
        out.push(DrawCommand::PushClip(b));
        for (i, r) in rows.iter().enumerate() {
            let rr = Self::row_rect(&b, i);
            let sel = Some(r.item.id) == self.selected;
            let inner = Rectangle::new(
                Point::new(rr.origin.x + 2.0, rr.origin.y + 1.0),
                Size::new(rr.size.width - 4.0, rr.size.height - 2.0),
            );
            if sel {
                out.push(DrawCommand::Quad {
                    bounds: inner,
                    fill: p.selection,
                    radius: t.radii.medium,
                    border: None,
                    shadow: None,
                });
                if self.ui.is_focused(self.id) {
                    out.push(focus_ring(inner, t.radii.medium, p.focus_ring));
                }
            } else if self.ui.hovered(&rr) {
                out.push(DrawCommand::Quad {
                    bounds: inner,
                    fill: p.hover,
                    radius: t.radii.medium,
                    border: None,
                    shadow: None,
                });
            }
            let x0 = rr.origin.x + 8.0 + r.depth as f32 * INDENT;
            let cy = rr.origin.y + ROW_H / 2.0;
            if !r.item.children.is_empty() {
                // Chevron: right when closed, down when open.
                let (cx, a) = (x0 + INDENT / 2.0, 3.0);
                let (p0, p1, p2) = if self.open(r.item.id) {
                    (
                        Point::new(cx - a, cy - a / 2.0),
                        Point::new(cx, cy + a / 2.0),
                        Point::new(cx + a, cy - a / 2.0),
                    )
                } else {
                    (
                        Point::new(cx - a / 2.0, cy - a),
                        Point::new(cx + a / 2.0, cy),
                        Point::new(cx - a / 2.0, cy + a),
                    )
                };
                for (f, to) in [(p0, p1), (p1, p2)] {
                    out.push(DrawCommand::Line {
                        from: f,
                        to,
                        width: 1.5,
                        color: p.text_secondary,
                    });
                }
            }
            out.push(label_cmd(
                Point::new(x0 + INDENT + 2.0, cy - s.line_height / 2.0),
                r.item.label.clone(),
                &s,
                if sel { 600 } else { 400 },
                p.text,
            ));
        }
        out.push(DrawCommand::PopClip);
    }
}
