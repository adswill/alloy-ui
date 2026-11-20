//! Selectable, scrolling list.

use crate::{
    scrollable::Scrollable,
    ui::Ui,
    util::{self, key_of},
    Column,
};
use alloy_ui_core::{
    style::{Padding, Style},
    DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

/// One selectable row.
struct Row<M> {
    ui: Ui,
    index: usize,
    selected: bool,
    on_select: Rc<dyn Fn(usize) -> M>,
    child: Vec<Element<M>>,
}

impl<M: 'static> Widget<M> for Row<M> {
    fn style(&self) -> Style {
        Style {
            axis: alloy_ui_core::style::Axis::Vertical,
            padding: Padding::symmetric(self.ui.theme().spacing.md, self.ui.theme().spacing.xs),
            width: Length::Fill(1),
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
        if util::dispatch(&mut self.child, event, layout, cursor, out) {
            return true;
        }
        match event {
            Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
                self.ui.set_pressed(Some(key_of(&b)));
                true
            }
            Event::MouseReleased(MouseButton::Left) => {
                let was = self.ui.pressed() == Some(key_of(&b));
                if was && b.contains(cursor) {
                    out.push((self.on_select)(self.index));
                }
                was
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let b = layout.bounds;
        let fill = if self.selected {
            Some(t.palette.selection)
        } else if self.ui.hovered(&b) {
            Some(t.palette.surface)
        } else {
            None
        };
        if let Some(fill) = fill {
            out.push(DrawCommand::Quad {
                bounds: b,
                fill,
                radius: t.radii.small,
                border: None,
                shadow: None,
            });
        }
        util::draw_children(&self.child, layout, out);
    }
}

/// A scrolling list of rows with single selection. Up/Down move the
/// selection while the list has focus. Rows can contain any element.
pub struct List<M> {
    ui: Ui,
    id: u64,
    count: usize,
    selected: Option<usize>,
    on_select: Rc<dyn Fn(usize) -> M>,
    inner: Vec<Element<M>>,
}

impl<M: 'static> List<M> {
    /// Build a list from row contents. `id` is used for focus and scroll state.
    pub fn new(
        ui: &Ui,
        id: u64,
        items: Vec<Element<M>>,
        selected: Option<usize>,
        on_select: impl Fn(usize) -> M + 'static,
    ) -> Self {
        let on_select: Rc<dyn Fn(usize) -> M> = Rc::new(on_select);
        let count = items.len();
        let rows = items
            .into_iter()
            .enumerate()
            .map(|(i, item)| {
                Element::new(Row {
                    ui: ui.clone(),
                    index: i,
                    selected: selected == Some(i),
                    on_select: on_select.clone(),
                    child: vec![item],
                })
            })
            .collect();
        let column = Column::new(rows).width(Length::Fill(1)).into_element();
        Self {
            ui: ui.clone(),
            id,
            count,
            selected,
            on_select,
            inner: vec![Scrollable::new(ui, id, column).into_element()],
        }
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for List<M> {
    fn style(&self) -> Style {
        self.inner[0].0.style()
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.inner
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
        match event {
            Event::MousePressed(MouseButton::Left) if layout.bounds.contains(cursor) => {
                self.ui.set_focus(Some(self.id));
            }
            Event::KeyPressed(Key::Named(k), _)
                if self.ui.is_focused(Some(self.id)) && self.count > 0 =>
            {
                let last = self.count - 1;
                let next = match (k.as_str(), self.selected) {
                    ("ArrowDown", None) | ("Home", _) => Some(0),
                    ("ArrowDown", Some(i)) => Some((i + 1).min(last)),
                    ("ArrowUp", None) | ("End", _) => Some(last),
                    ("ArrowUp", Some(i)) => Some(i.saturating_sub(1)),
                    _ => None,
                };
                if let Some(i) = next {
                    out.push((self.on_select)(i));
                    return true;
                }
            }
            _ => {}
        }
        util::dispatch(&mut self.inner, event, layout, cursor, out)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(util::focus_ring(
                layout.bounds,
                self.ui.theme().radii.small,
                self.ui.theme().palette.focus_ring,
            ));
        }
        util::draw_children(&self.inner, layout, out);
    }
}
