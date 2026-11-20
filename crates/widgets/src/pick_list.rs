//! Drop-down selection.

use crate::{
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::Style, Border, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// Chooses one of several strings. The popup is drawn on the overlay layer
/// (above everything, never clipped by a scrollable) and clicks on it are
/// routed to the list by [`Root`](crate::Root).
pub struct PickList<M> {
    ui: Ui,
    id: u64,
    options: Vec<String>,
    selected: Option<usize>,
    placeholder: String,
    on_select: Option<Box<dyn Fn(usize) -> M>>,
    width: Length,
}

impl<M: 'static> PickList<M> {
    /// A pick list over `options`. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, options: Vec<String>, selected: Option<usize>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            options,
            selected,
            placeholder: String::new(),
            on_select: None,
            width: Length::Shrink,
        }
    }
    /// Text shown while nothing is selected.
    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }
    /// Called with the chosen index; without it the list is disabled.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Box::new(f));
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

    fn row_h(&self) -> f32 {
        let t = self.ui.theme();
        t.typography.body.line_height + 2.0 * t.spacing.xs
    }
    fn popup(&self, b: &Rectangle) -> Rectangle {
        Rectangle::new(
            Point::new(b.origin.x, b.origin.y + b.size.height + 4.0),
            Size::new(b.size.width, self.row_h() * self.options.len() as f32 + 8.0),
        )
    }
    fn option_at(&self, b: &Rectangle, p: Point) -> Option<usize> {
        let pop = self.popup(b);
        if !pop.contains(p) {
            return None;
        }
        let i = ((p.y - pop.origin.y - 4.0) / self.row_h()).floor();
        (i >= 0.0 && (i as usize) < self.options.len()).then_some(i as usize)
    }
    fn is_open(&self) -> bool {
        self.ui.popup() == Some(self.id)
    }
    fn current_label(&self) -> &str {
        match self.selected.and_then(|i| self.options.get(i)) {
            Some(s) => s,
            None => &self.placeholder,
        }
    }
}

impl<M: 'static> Widget<M> for PickList<M> {
    fn style(&self) -> Style {
        util::sized(self.width, Length::Shrink)
    }
    fn measure(&self, _available: Size) -> Size {
        let t = self.ui.theme();
        let s = t.typography.body;
        let m = self.ui.metrics();
        let widest = self
            .options
            .iter()
            .map(|o| m.width(o, s.size))
            .fold(m.width(&self.placeholder, s.size), f32::max);
        Size::new(
            widest + 2.0 * t.spacing.md + 18.0,
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
        let Some(on_select) = &self.on_select else {
            return false;
        };
        match event {
            Event::MousePressed(MouseButton::Left) => {
                // A press that landed on the popup, routed here by `Root`.
                if let Some(p) = self.ui.take_hit(self.id) {
                    self.ui.set_popup(None);
                    if let Some(i) = self.option_at(&b, p) {
                        out.push(on_select(i));
                    }
                    return true;
                }
                if self.is_open() {
                    self.ui.set_popup(None);
                    if let Some(i) = self.option_at(&b, cursor) {
                        out.push(on_select(i));
                        return true;
                    }
                    // A press on the field itself just closes it.
                    return b.contains(cursor);
                }
                if b.contains(cursor) {
                    self.ui.set_focus(Some(self.id));
                    self.ui.set_popup(Some(self.id));
                    return true;
                }
                false
            }
            Event::MouseReleased(MouseButton::Left) => false,
            Event::KeyPressed(Key::Escape, _) if self.is_open() => {
                self.ui.set_popup(None);
                true
            }
            Event::KeyPressed(key, _) if self.ui.is_focused(Some(self.id)) => {
                let last = self.options.len().checked_sub(1);
                let go = |i: usize, out: &mut Vec<M>| out.push(on_select(i));
                match (key, last) {
                    (Key::Enter, _) => {
                        self.ui
                            .set_popup(if self.is_open() { None } else { Some(self.id) });
                        true
                    }
                    (Key::Character(c), _) if c == " " => {
                        self.ui
                            .set_popup(if self.is_open() { None } else { Some(self.id) });
                        true
                    }
                    (Key::Named(n), Some(last)) if n == "ArrowDown" => {
                        go(self.selected.map_or(0, |i| (i + 1).min(last)), out);
                        true
                    }
                    (Key::Named(n), Some(last)) if n == "ArrowUp" => {
                        go(self.selected.map_or(last, |i| i.saturating_sub(1)), out);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let t = self.ui.theme();
        let p = &t.palette;
        let b = layout.bounds;
        let enabled = self.on_select.is_some();
        let s = t.typography.body;
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.medium, p.focus_ring));
        }
        let hover = enabled && self.ui.hovered(&b);
        out.push(DrawCommand::Quad {
            bounds: b,
            fill: if enabled {
                if hover {
                    p.surface
                } else {
                    p.surface_raised
                }
            } else {
                p.surface
            },
            radius: t.radii.medium,
            border: Some(Border {
                color: if enabled { p.border_strong } else { p.border },
                width: 1.0,
            }),
            shadow: None,
        });
        let empty = self.selected.is_none();
        out.push(DrawCommand::Text {
            family: alloy_ui_core::FontFamily::Sans,
            weight: 400,
            line_height: None,
            origin: Point::new(
                b.origin.x + t.spacing.md,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: self.current_label().to_owned(),
            size: s.size,
            color: if !enabled {
                p.text_disabled
            } else if empty {
                p.text_secondary
            } else {
                p.text
            },
            max_width: None,
        });
        out.push(DrawCommand::Text {
            family: alloy_ui_core::FontFamily::Sans,
            weight: 400,
            line_height: None,
            origin: Point::new(
                b.origin.x + b.size.width - t.spacing.md - 10.0,
                b.origin.y + (b.size.height - s.line_height) / 2.0,
            ),
            content: "\u{25BE}".into(),
            size: s.size,
            color: p.text_secondary,
            max_width: None,
        });

        if self.is_open() {
            let pop = self.popup(&b);
            let mut cmds = Vec::new();
            cmds.push(DrawCommand::Quad {
                bounds: pop,
                fill: p.surface_raised,
                radius: t.radii.medium,
                border: Some(Border {
                    color: p.border,
                    width: 1.0,
                }),
                shadow: Some(t.elevation.medium),
            });
            let hot = self.option_at(&b, self.ui.cursor());
            for (i, o) in self.options.iter().enumerate() {
                let row = Rectangle::new(
                    Point::new(
                        pop.origin.x + 4.0,
                        pop.origin.y + 4.0 + i as f32 * self.row_h(),
                    ),
                    Size::new(pop.size.width - 8.0, self.row_h()),
                );
                if hot == Some(i) || self.selected == Some(i) {
                    cmds.push(DrawCommand::Quad {
                        bounds: row,
                        fill: if hot == Some(i) {
                            p.surface
                        } else {
                            p.selection
                        },
                        radius: t.radii.small,
                        border: None,
                        shadow: None,
                    });
                }
                cmds.push(DrawCommand::Text {
                    family: alloy_ui_core::FontFamily::Sans,
                    weight: 400,
                    line_height: None,
                    origin: Point::new(
                        row.origin.x + t.spacing.sm,
                        row.origin.y + (row.size.height - s.line_height) / 2.0,
                    ),
                    content: o.clone(),
                    size: s.size,
                    color: p.text,
                    max_width: None,
                });
            }
            // Overlay layer: above later siblings and outside scroll clips.
            self.ui.overlay(Some(self.id), Some(pop), cmds);
        }
    }
}
