//! Virtualised list: only the visible rows exist, so 100 000 rows cost the same as 20.

use crate::{
    ui::Ui,
    util::{self, focus_ring},
};
use alloy_ui_core::{
    style::{Axis, Style},
    DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;
use std::rc::Rc;

const BAR: f32 = 8.0;
const BAR_ZONE: f32 = 14.0;
const K_DRAG: u64 = 0x7117_0001 << 20;

type Build<M> = Rc<dyn Fn(usize) -> Element<M>>;

/// A list of `count` rows of fixed height, built on demand by `row(index)`. Only rows in view
/// are built and drawn, per frame, so the cost follows the window, not the data. Selection
/// (click, Up/Down, PageUp/PageDown, Home/End) is reported through `on_select`; the scrollbar
/// thumb drags, and the wheel scrolls.
pub struct VirtualList<M> {
    ui: Ui,
    id: u64,
    count: usize,
    row_h: f32,
    ruled: bool,
    selected: Option<usize>,
    build: Build<M>,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
}

impl<M: 'static> VirtualList<M> {
    /// `count` rows, row `i` built by `row(i)`. `id` must be unique per window.
    pub fn new(
        ui: &Ui,
        id: u64,
        count: usize,
        row: impl Fn(usize) -> Element<M> + 'static,
    ) -> Self {
        let t = ui.theme();
        Self {
            ui: ui.clone(),
            id,
            count,
            row_h: t.typography.body.line_height + 2.0 * t.spacing.sm,
            ruled: false,
            selected: None,
            build: Rc::new(row),
            on_select: None,
        }
    }
    /// Row height in pixels (every row is the same height).
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_h = h.max(8.0);
        self
    }
    /// A hairline between rows.
    pub fn ruled(mut self) -> Self {
        self.ruled = true;
        self
    }
    /// The highlighted row.
    pub fn selected(mut self, i: Option<usize>) -> Self {
        self.selected = i;
        self
    }
    /// Called with the index of the row the user picked.
    pub fn on_select(mut self, f: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn content_h(&self) -> f32 {
        self.count as f32 * self.row_h
    }
    fn max_off(&self, b: &Rectangle) -> f32 {
        (self.content_h() - b.size.height).max(0.0)
    }
    fn offset(&self, b: &Rectangle) -> f32 {
        self.ui.scroll_offset(self.id).clamp(0.0, self.max_off(b))
    }
    fn set_offset(&self, b: &Rectangle, off: f32) {
        self.ui
            .set_scroll_offset(self.id, off.clamp(0.0, self.max_off(b)));
    }
    fn range(&self, b: &Rectangle) -> std::ops::Range<usize> {
        let off = self.offset(b);
        let first = (off / self.row_h).floor() as usize;
        let last = (((off + b.size.height) / self.row_h).ceil() as usize).min(self.count);
        first.min(self.count)..last
    }
    fn row_rect(&self, b: &Rectangle, i: usize) -> Rectangle {
        let bar = if self.max_off(b) > 0.0 {
            BAR + 4.0
        } else {
            0.0
        };
        Rectangle::new(
            Point::new(
                b.origin.x,
                b.origin.y + i as f32 * self.row_h - self.offset(b),
            ),
            Size::new((b.size.width - bar).max(0.0), self.row_h),
        )
    }
    /// The built row element and its laid-out content, centred vertically in the row.
    fn laid(&self, b: &Rectangle, i: usize) -> (Element<M>, Layout) {
        let r = self.row_rect(b, i);
        let pad = self.ui.theme().spacing.md;
        let e = (self.build)(i);
        let h = util::preferred(e.0.as_ref()).height.min(r.size.height);
        let area = Rectangle::new(
            Point::new(r.origin.x + pad, r.origin.y + (r.size.height - h) / 2.0),
            Size::new((r.size.width - 2.0 * pad).max(0.0), h),
        );
        let l = e.0.arrange(area);
        (e, l)
    }
    fn thumb(&self, b: &Rectangle) -> Option<Rectangle> {
        let max = self.max_off(b);
        if max <= 0.0 || b.size.height <= 0.0 {
            return None;
        }
        let h = (b.size.height * b.size.height / self.content_h())
            .max(28.0)
            .min(b.size.height);
        let y = b.origin.y + (b.size.height - h) * self.offset(b) / max;
        Some(Rectangle::new(
            Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
            Size::new(BAR, h),
        ))
    }
    fn drag_to(&self, b: &Rectangle, y: f32, grab: f32) {
        if let Some(th) = self.thumb(b) {
            let travel = (b.size.height - th.size.height).max(1.0);
            let top = (y - grab - b.origin.y).clamp(0.0, travel);
            self.set_offset(b, top / travel * self.max_off(b));
        }
    }
    fn reveal(&self, b: &Rectangle, i: usize) {
        let top = i as f32 * self.row_h;
        let off = self.offset(b);
        if top < off {
            self.set_offset(b, top);
        } else if top + self.row_h > off + b.size.height {
            self.set_offset(b, top + self.row_h - b.size.height);
        }
    }
}

impl<M: 'static> Widget<M> for VirtualList<M> {
    fn style(&self) -> Style {
        Style {
            axis: Axis::Vertical,
            width: Length::Fill(1),
            height: Length::Fill(1),
            ..Style::default()
        }
    }
    fn measure(&self, _available: Size) -> Size {
        Size::new(240.0, self.row_h * self.count.min(8) as f32)
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let b = layout.bounds;
        let inside = b.contains(cursor);
        match event {
            Event::Scrolled { dy, .. } if inside => {
                let before = self.offset(&b);
                self.set_offset(&b, before - dy);
                return self.offset(&b) != before;
            }
            Event::MousePressed(MouseButton::Left) if inside => {
                self.ui.set_focus(Some(self.id));
                let in_bar = cursor.x >= b.origin.x + b.size.width - BAR_ZONE;
                if let (true, Some(th)) = (in_bar, self.thumb(&b)) {
                    // Grab the thumb where it was hit, or centre it under the pointer.
                    let grab = if th.contains(Point::new(th.origin.x + 1.0, cursor.y)) {
                        cursor.y - th.origin.y
                    } else {
                        th.size.height / 2.0
                    };
                    self.ui
                        .set_slot(self.id ^ K_DRAG, Some((grab * 16.0) as i64));
                    self.drag_to(&b, cursor.y, grab);
                    return true;
                }
                let i = ((cursor.y - b.origin.y + self.offset(&b)) / self.row_h).floor();
                if i >= 0.0 && (i as usize) < self.count {
                    let i = i as usize;
                    let (mut row, l) = self.laid(&b, i);
                    if row.0.on_event(event, &l, cursor, out) {
                        return true;
                    }
                    if let Some(f) = &self.on_select {
                        out.push(f(i));
                    }
                }
                return true;
            }
            Event::CursorMoved(_) => {
                if let Some(g) = self.ui.slot(self.id ^ K_DRAG) {
                    self.drag_to(&b, cursor.y, g as f32 / 16.0);
                    return true;
                }
            }
            Event::MouseReleased(MouseButton::Left) => {
                if self.ui.slot(self.id ^ K_DRAG).is_some() {
                    self.ui.set_slot(self.id ^ K_DRAG, None);
                    return true;
                }
                // Let controls inside a row finish their press.
                for i in self.range(&b) {
                    let (mut row, l) = self.laid(&b, i);
                    if row.0.on_event(event, &l, cursor, out) {
                        return true;
                    }
                }
            }
            Event::KeyPressed(Key::Named(k), _)
                if self.ui.is_focused(Some(self.id)) && self.count > 0 =>
            {
                let last = self.count - 1;
                let page = ((b.size.height / self.row_h) as usize)
                    .saturating_sub(1)
                    .max(1);
                let next = match (k.as_str(), self.selected) {
                    ("ArrowDown", None) | ("Home", _) => Some(0),
                    ("ArrowDown", Some(i)) => Some((i + 1).min(last)),
                    ("ArrowUp", None) | ("End", _) => Some(last),
                    ("ArrowUp", Some(i)) => Some(i.saturating_sub(1)),
                    ("PageDown", i) => Some((i.unwrap_or(0) + page).min(last)),
                    ("PageUp", i) => Some(i.unwrap_or(0).saturating_sub(page)),
                    _ => None,
                };
                if let Some(i) = next {
                    self.reveal(&b, i);
                    if let Some(f) = &self.on_select {
                        out.push(f(i));
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let p = &t.palette;
        self.ui.register(self.id);
        if self.ui.is_focused(Some(self.id)) {
            out.push(focus_ring(b, t.radii.small, p.focus_ring));
        }
        out.push(DrawCommand::PushClip(b));
        let outer = self.ui.clip();
        self.ui.set_clip(Some(b));
        let cursor = self.ui.cursor();
        for i in self.range(&b) {
            let r = self.row_rect(&b, i);
            let hot = b.contains(cursor) && r.contains(cursor);
            if self.selected == Some(i) {
                out.push(DrawCommand::Quad {
                    bounds: r,
                    fill: p.selection,
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            } else if hot {
                out.push(DrawCommand::Quad {
                    bounds: r,
                    fill: p.hover,
                    radius: t.radii.small,
                    border: None,
                    shadow: None,
                });
            }
            let (row, l) = self.laid(&b, i);
            row.0.draw(&l, out);
            if self.ruled && self.selected != Some(i) {
                out.push(DrawCommand::Quad {
                    bounds: Rectangle::new(
                        Point::new(r.origin.x + t.spacing.md, r.origin.y + r.size.height - 1.0),
                        Size::new((r.size.width - 2.0 * t.spacing.md).max(0.0), 1.0),
                    ),
                    fill: p.border.with_alpha(0.6),
                    radius: 0.0,
                    border: None,
                    shadow: None,
                });
            }
        }
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);
        if let Some(th) = self.thumb(&b) {
            let live = self.ui.slot(self.id ^ K_DRAG).is_some() || self.ui.hovered(&th);
            out.push(DrawCommand::Quad {
                bounds: th,
                fill: if live {
                    p.border_strong
                } else {
                    p.border_strong.with_alpha(0.55)
                },
                radius: BAR / 2.0,
                border: None,
                shadow: None,
            });
        }
    }
}
