//! Resizable two-pane split.

use crate::{
    ui::Ui,
    util::{self, focus_ring, mix},
};
use alloy_ui_core::{
    style::{Axis, Limits, Style},
    DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;

/// Width of the draggable gutter between the panes.
const GAP: f32 = 9.0;
const K_DRAG: u64 = 0x5917_0001 << 20;
const K_RATIO: u64 = 0x5917_0002 << 20;
/// Slot: the first pane's laid-out length last drawn, in 1/16 px.
const K_LEN: u64 = 0x5917_0003 << 20;
/// Slot: the length a view last read through [`SplitPane::first_len_of`].
const K_SEEN: u64 = 0x5917_0004 << 20;
const KEY_STEP: f32 = 24.0;

/// Sizing shell around one pane. Its share of the space is read from the [`Ui`] every time
/// layout runs, so dragging re-lays-out without rebuilding the view.
struct Pane<M> {
    ui: Ui,
    split: u64,
    initial: f32,
    second: bool,
    vertical: bool,
    min: f32,
    child: Vec<Element<M>>,
}

fn ratio(ui: &Ui, split: u64, initial: f32) -> f32 {
    ui.slot(split ^ K_RATIO)
        .map_or(initial, |v| v as f32 / 10_000.0)
}

impl<M: 'static> Widget<M> for Pane<M> {
    fn style(&self) -> Style {
        let w1 = (ratio(&self.ui, self.split, self.initial) * 1000.0)
            .round()
            .clamp(1.0, 999.0) as u16;
        let along = Length::Fill(if self.second { 1000 - w1 } else { w1 });
        let across = Length::Fill(1);
        let min = if self.vertical {
            Size::new(0.0, self.min)
        } else {
            Size::new(self.min, 0.0)
        };
        Style {
            axis: Axis::Vertical,
            width: if self.vertical { across } else { along },
            height: if self.vertical { along } else { across },
            limits: Limits::new(min, Size::new(f32::INFINITY, f32::INFINITY)),
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
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        out.push(DrawCommand::PushClip(layout.bounds));
        util::draw_children(&self.child, layout, out);
        out.push(DrawCommand::PopClip);
    }
}

/// Two panes with a draggable divider. Drag it with the pointer, or focus it (Tab) and use the
/// arrow keys, Home and End. The split position is remembered in the [`Ui`] (as a ratio, so it
/// survives window resizes); `ratio` only sets where it starts. Each pane keeps at least
/// `min` pixels. Panes should size themselves with `Fill` to use the space they are given.
pub struct SplitPane<M> {
    ui: Ui,
    id: u64,
    vertical: bool,
    ratio: f32,
    min: f32,
    panes: Vec<Element<M>>,
}

impl<M: 'static> SplitPane<M> {
    /// `first` and `second` side by side. `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, first: Element<M>, second: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            vertical: false,
            ratio: 0.5,
            min: 120.0,
            panes: vec![first, second],
        }
    }
    /// Stack the panes (first on top) instead of side by side.
    pub fn vertical(mut self) -> Self {
        self.vertical = true;
        self
    }
    /// Where the divider starts, as the fraction of the space the first pane takes.
    pub fn ratio(mut self, r: f32) -> Self {
        self.ratio = r.clamp(0.0, 1.0);
        self
    }
    /// Smallest size in pixels either pane can be dragged to.
    pub fn min(mut self, px: f32) -> Self {
        self.min = px;
        self
    }
    /// The first pane's current length in pixels (its width, or its height for a
    /// [`vertical`](Self::vertical) split) as of the last frame drawn, for the split with `id`;
    /// 0 before the first frame. Reading it inside the view makes the split ask for one more
    /// frame whenever the size has since changed (a drag, a window resize), so the view always
    /// catches up.
    pub fn first_len_of(ui: &Ui, id: u64) -> f32 {
        let len = ui.slot(id ^ K_LEN);
        ui.set_slot(id ^ K_SEEN, Some(len.unwrap_or(0)));
        len.unwrap_or(0) as f32 / 16.0
    }
    /// Box as an element.
    pub fn into_element(mut self) -> Element<M> {
        // Wrap the panes now that every option is known.
        let kids = std::mem::take(&mut self.panes);
        let mut it = kids.into_iter();
        let (a, b) = (it.next(), it.next());
        for (i, child) in [a, b].into_iter().flatten().enumerate() {
            self.panes.push(Element::new(Pane {
                ui: self.ui.clone(),
                split: self.id,
                initial: self.ratio,
                second: i == 1,
                vertical: self.vertical,
                min: self.min,
                child: vec![child],
            }));
        }
        Element::new(self)
    }

    /// The gutter between the two laid-out panes.
    fn gutter(&self, layout: &Layout) -> Rectangle {
        let b = layout.bounds;
        match (layout.children.first(), layout.children.get(1)) {
            (Some(a), Some(c)) if self.vertical => {
                let y = a.bounds.origin.y + a.bounds.size.height;
                Rectangle::new(
                    Point::new(b.origin.x, y),
                    Size::new(b.size.width, (c.bounds.origin.y - y).max(0.0)),
                )
            }
            (Some(a), Some(c)) => {
                let x = a.bounds.origin.x + a.bounds.size.width;
                Rectangle::new(
                    Point::new(x, b.origin.y),
                    Size::new((c.bounds.origin.x - x).max(0.0), b.size.height),
                )
            }
            _ => Rectangle::default(),
        }
    }
    /// Space available to the two panes (excluding the gutter).
    fn total(&self, b: &Rectangle) -> f32 {
        (if self.vertical {
            b.size.height
        } else {
            b.size.width
        } - GAP)
            .max(0.0)
    }
    fn first_len(&self, layout: &Layout) -> f32 {
        layout.children.first().map_or(0.0, |a| {
            if self.vertical {
                a.bounds.size.height
            } else {
                a.bounds.size.width
            }
        })
    }
    fn set_first_len(&self, b: &Rectangle, len: f32) {
        let total = self.total(b);
        if total > 0.0 {
            let min = self.min.min(total / 2.0);
            let len = len.clamp(min, (total - min).max(min));
            let r = (len / total).clamp(0.0, 1.0);
            self.ui
                .set_slot(self.id ^ K_RATIO, Some((r * 10_000.0).round() as i64));
        }
    }
    fn dragging(&self) -> bool {
        self.ui.slot(self.id ^ K_DRAG).is_some()
    }
}

impl<M: 'static> Widget<M> for SplitPane<M> {
    fn style(&self) -> Style {
        Style {
            axis: if self.vertical {
                Axis::Vertical
            } else {
                Axis::Horizontal
            },
            spacing: GAP,
            width: Length::Fill(1),
            height: Length::Fill(1),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.panes
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
        let gutter = self.gutter(layout);
        match event {
            Event::MousePressed(MouseButton::Left) if gutter.contains(cursor) => {
                self.ui.set_slot(self.id ^ K_DRAG, Some(1));
                self.ui.set_focus(Some(self.id));
                return true;
            }
            Event::CursorMoved(p) if self.dragging() => {
                let pos = if self.vertical {
                    p.y - b.origin.y
                } else {
                    p.x - b.origin.x
                };
                self.set_first_len(&b, pos - GAP / 2.0);
                return true;
            }
            Event::MouseReleased(MouseButton::Left) if self.dragging() => {
                self.ui.set_slot(self.id ^ K_DRAG, None);
                return true;
            }
            Event::KeyPressed(Key::Named(n), _) if self.ui.is_focused(Some(self.id)) => {
                let (dec, inc) = if self.vertical {
                    ("ArrowUp", "ArrowDown")
                } else {
                    ("ArrowLeft", "ArrowRight")
                };
                let f = self.first_len(layout);
                let total = self.total(&b);
                let to = match n.as_str() {
                    k if k == dec => f - KEY_STEP,
                    k if k == inc => f + KEY_STEP,
                    "Home" => 0.0,
                    "End" => total,
                    _ => return false,
                };
                self.set_first_len(&b, to);
                return true;
            }
            _ => {}
        }
        util::dispatch(&mut self.panes, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        // A view that read the pane size needs another build once the size moved.
        let stale = self
            .ui
            .slot(self.id ^ K_SEEN)
            .is_some_and(|seen| Some(seen) != self.ui.slot(self.id ^ K_LEN));
        stale || util::any_animating(&self.panes)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        util::draw_children(&self.panes, layout, out);
        self.ui.register(self.id);
        self.ui.set_slot(
            self.id ^ K_LEN,
            Some((self.first_len(layout) * 16.0).round() as i64),
        );
        let t = self.ui.theme();
        let p = &t.palette;
        let g = self.gutter(layout);
        let focused = self.ui.is_focused(Some(self.id));
        let active = self.dragging() || focused || self.ui.hovered(&g);
        let e = self
            .ui
            .ease(0x5917_7000_0000 ^ self.id, active as u8 as f32);
        let w = 1.0 + e;
        let (line, grip) = if self.vertical {
            (
                Rectangle::new(
                    Point::new(g.origin.x, g.origin.y + (g.size.height - w) / 2.0),
                    Size::new(g.size.width, w),
                ),
                Rectangle::new(
                    Point::new(
                        g.origin.x + (g.size.width - 32.0) / 2.0,
                        g.origin.y + (g.size.height - 4.0) / 2.0,
                    ),
                    Size::new(32.0, 4.0),
                ),
            )
        } else {
            (
                Rectangle::new(
                    Point::new(g.origin.x + (g.size.width - w) / 2.0, g.origin.y),
                    Size::new(w, g.size.height),
                ),
                Rectangle::new(
                    Point::new(
                        g.origin.x + (g.size.width - 4.0) / 2.0,
                        g.origin.y + (g.size.height - 32.0) / 2.0,
                    ),
                    Size::new(4.0, 32.0),
                ),
            )
        };
        out.push(DrawCommand::Quad {
            bounds: line,
            fill: mix(p.border, p.accent, e),
            radius: 0.0,
            border: None,
            shadow: None,
        });
        // A short grip sits on the line so the divider reads as draggable.
        out.push(DrawCommand::Quad {
            bounds: grip,
            fill: mix(p.border_strong, p.accent, e),
            radius: 2.0,
            border: None,
            shadow: None,
        });
        if focused {
            out.push(focus_ring(grip, 2.0, p.focus_ring));
        }
    }
}
