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
use std::cell::Cell;
use std::rc::Rc;

const BAR: f32 = 8.0;
const BAR_ZONE: f32 = 14.0;
const K_DRAG: u64 = 0x7117_0001 << 20;
/// Offsets below this (in pixels) are smooth-scrolled; an `f32` still resolves sub-pixel there.
const GLIDE_LIMIT: f64 = 4_000_000.0;

/// Row heights for a [`VirtualList`] whose rows differ in height.
///
/// Build it once (it holds one running total per row, 8 bytes each, so a million rows cost
/// 8 MB) and keep it in your state; cloning is cheap. Finding the row at a scroll offset is a
/// binary search, so scrolling a million variable-height rows costs the same as twenty.
/// Call [`HeightIndex::new`] again when your data changes.
#[derive(Clone)]
pub struct HeightIndex {
    /// `ends[i]` is the bottom edge of row `i`; the top of row 0 is 0.
    ends: Rc<Vec<f64>>,
}

impl HeightIndex {
    /// Measure `count` rows: row `i` is `height(i)` pixels tall (at least 1).
    pub fn new(count: usize, height: impl Fn(usize) -> f32) -> Self {
        let mut acc = 0.0f64;
        let ends = (0..count)
            .map(|i| {
                acc += height(i).max(1.0) as f64;
                acc
            })
            .collect();
        Self {
            ends: Rc::new(ends),
        }
    }
    /// Number of rows measured.
    pub fn len(&self) -> usize {
        self.ends.len()
    }
    /// True when there are no rows.
    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }
    /// Total height of all rows.
    pub fn total(&self) -> f64 {
        self.ends.last().copied().unwrap_or(0.0)
    }
    /// Top edge of row `i` (the total height for `i == len`).
    pub fn top(&self, i: usize) -> f64 {
        if i == 0 {
            0.0
        } else {
            self.ends[(i - 1).min(self.ends.len() - 1)]
        }
    }
    /// Height of row `i`.
    pub fn height(&self, i: usize) -> f32 {
        (self.ends[i] - self.top(i)) as f32
    }
    /// The row that contains vertical offset `y` (clamped to the last row).
    pub fn row_at(&self, y: f64) -> usize {
        self.ends
            .partition_point(|&e| e <= y)
            .min(self.ends.len().saturating_sub(1))
    }
}

/// Row geometry: one height for every row, or a measured [`HeightIndex`].
#[derive(Clone)]
enum Heights {
    Fixed(f32),
    Var(HeightIndex),
}

type Build<M> = Rc<dyn Fn(usize) -> Element<M>>;

/// A list of `count` rows of fixed height, built on demand by `row(index)`. Only rows in view
/// are built and drawn, per frame, so the cost follows the window, not the data. Selection
/// (click, Up/Down, PageUp/PageDown, Home/End) is reported through `on_select`; the scrollbar
/// thumb drags, and the wheel scrolls.
pub struct VirtualList<M> {
    ui: Ui,
    id: u64,
    count: usize,
    heights: Heights,
    ruled: bool,
    selected: Option<usize>,
    build: Build<M>,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
    /// Bounds of the last draw, so `animating` can look at the rows that were on show.
    shown: Cell<Option<Rectangle>>,
}

fn tree_animating<M>(e: &Element<M>) -> bool {
    e.0.animating() || e.0.children().iter().any(tree_animating)
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
            heights: Heights::Fixed(t.typography.body.line_height + 2.0 * t.spacing.sm),
            ruled: false,
            selected: None,
            build: Rc::new(row),
            on_select: None,
            shown: Cell::new(None),
        }
    }
    /// Row height in pixels (every row is the same height).
    pub fn row_height(mut self, h: f32) -> Self {
        self.heights = Heights::Fixed(h.max(8.0));
        self
    }
    /// Rows of different heights, as measured by `heights` (see [`HeightIndex`]). The list
    /// shows `heights.len()` rows; build the index once and keep it in your state.
    pub fn variable(mut self, heights: &HeightIndex) -> Self {
        self.count = heights.len();
        self.heights = Heights::Var(heights.clone());
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

    // Offsets and content height are f64: past 16.7 million pixels (a million 28 px rows) an f32
    // cannot tell one pixel from the next, so rows would jitter and the end would be unreachable.
    fn content_h(&self) -> f64 {
        match &self.heights {
            Heights::Fixed(h) => self.count as f64 * *h as f64,
            Heights::Var(ix) => ix.total(),
        }
    }
    fn row_top(&self, i: usize) -> f64 {
        match &self.heights {
            Heights::Fixed(h) => i as f64 * *h as f64,
            Heights::Var(ix) => ix.top(i),
        }
    }
    fn row_height_of(&self, i: usize) -> f32 {
        match &self.heights {
            Heights::Fixed(h) => *h,
            Heights::Var(ix) => ix.height(i),
        }
    }
    /// The row under list-space offset `y`, if any.
    fn row_at(&self, y: f64) -> Option<usize> {
        if self.count == 0 || y < 0.0 || y >= self.content_h() {
            return None;
        }
        Some(match &self.heights {
            Heights::Fixed(h) => ((y / *h as f64).floor() as usize).min(self.count - 1),
            Heights::Var(ix) => ix.row_at(y),
        })
    }
    /// Average row height, for paging.
    fn avg_h(&self) -> f32 {
        (self.content_h() / self.count.max(1) as f64).max(1.0) as f32
    }
    fn max_off(&self, b: &Rectangle) -> f64 {
        (self.content_h() - b.size.height as f64).max(0.0)
    }
    /// Where scrolling is heading.
    fn target(&self, b: &Rectangle) -> f64 {
        self.ui
            .scroll_offset_f64(self.id)
            .clamp(0.0, self.max_off(b))
    }
    /// Where the list is drawn: the target followed by the scroll spring. Past `GLIDE_LIMIT`
    /// an `f32` spring cannot resolve single pixels, so the list jumps to the target instead.
    fn offset(&self, b: &Rectangle) -> f64 {
        let t = self.target(b);
        if t < GLIDE_LIMIT {
            (self.ui.scroll_shown(self.id, self.max_off(b) as f32) as f64)
                .clamp(0.0, self.max_off(b))
        } else {
            t
        }
    }
    fn set_offset(&self, b: &Rectangle, off: f64) {
        self.ui
            .set_scroll_offset_f64(self.id, off.clamp(0.0, self.max_off(b)));
    }
    fn range(&self, b: &Rectangle) -> std::ops::Range<usize> {
        let off = self.offset(b);
        let Some(first) = self.row_at(off) else {
            return 0..0;
        };
        // One past the row holding the viewport's last pixel.
        let last = self
            .row_at((off + b.size.height as f64 - 0.01).min(self.content_h() - 0.01))
            .map_or(first, |i| i + 1);
        first..last
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
                b.origin.y + (self.row_top(i) - self.offset(b)) as f32,
            ),
            Size::new((b.size.width - bar).max(0.0), self.row_height_of(i)),
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
    /// Offer `event` to the visible rows, stopping at the first that consumes it.
    fn offer(&self, b: &Rectangle, event: &Event, cursor: Point, out: &mut Vec<M>) -> bool {
        for i in self.range(b) {
            let (mut row, l) = self.laid(b, i);
            if row.0.on_event(event, &l, cursor, out) {
                return true;
            }
        }
        false
    }
    /// Show `event` to every visible row (hover watchers, timers); what they answer is ignored.
    fn broadcast(&self, b: &Rectangle, event: &Event, cursor: Point, out: &mut Vec<M>) {
        for i in self.range(b) {
            let (mut row, l) = self.laid(b, i);
            row.0.on_event(event, &l, cursor, out);
        }
    }
    fn thumb(&self, b: &Rectangle) -> Option<Rectangle> {
        let max = self.max_off(b);
        if max <= 0.0 || b.size.height <= 0.0 {
            return None;
        }
        let h = (b.size.height as f64 * b.size.height as f64 / self.content_h())
            .max(28.0)
            .min(b.size.height as f64) as f32;
        let y = b.origin.y + ((b.size.height - h) as f64 * self.offset(b) / max) as f32;
        Some(Rectangle::new(
            Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
            Size::new(BAR, h),
        ))
    }
    fn drag_to(&self, b: &Rectangle, y: f32, grab: f32) {
        if let Some(th) = self.thumb(b) {
            let travel = (b.size.height - th.size.height).max(1.0);
            let top = (y - grab - b.origin.y).clamp(0.0, travel);
            // The thumb is under the pointer, so the content follows it without lag.
            self.ui.scroll_jump_f64(
                self.id,
                top as f64 / travel as f64 * self.max_off(b),
                self.max_off(b),
            );
        }
    }
    fn reveal(&self, b: &Rectangle, i: usize) {
        let top = self.row_top(i);
        let off = self.target(b);
        let (rh, vh) = (self.row_height_of(i) as f64, b.size.height as f64);
        if top < off {
            self.set_offset(b, top);
        } else if top + rh > off + vh {
            self.set_offset(b, top + rh - vh);
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
        Size::new(240.0, self.avg_h() * self.count.min(8) as f32)
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
        // An overlay press (open popover or context menu in a row) arrives with the pointer
        // parked off-screen: offer it to every visible row so the owner finds it.
        if self.ui.hit_pending() && matches!(event, Event::MousePressed(_) | Event::Scrolled { .. })
        {
            for i in self.range(&b) {
                let (mut row, l) = self.laid(&b, i);
                if row.0.on_event(event, &l, cursor, out) {
                    return true;
                }
            }
            return false;
        }
        // Rows see pointer movement and ticks always (hover, `tick_while_shown`), and while a
        // popup (a row's context menu) is open they also see every key and press, so Escape,
        // arrows, Enter and a click elsewhere reach the menu instead of moving the selection.
        match event {
            Event::CursorMoved(_) | Event::CursorLeft | Event::Tick(_) => {
                self.broadcast(&b, event, cursor, out)
            }
            Event::KeyPressed(..) | Event::MousePressed(_) if self.ui.popup().is_some() => {
                if self.offer(&b, event, cursor, out) {
                    return true;
                }
            }
            _ => {}
        }
        match event {
            Event::Scrolled { dy, .. } if inside => {
                let before = self.target(&b);
                self.set_offset(&b, before - *dy as f64);
                return self.target(&b) != before;
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
                if let Some(i) = self.row_at((cursor.y - b.origin.y) as f64 + self.offset(&b)) {
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
            // Right and middle presses belong to the row's own controls (a context menu).
            Event::MousePressed(_) if inside => {
                if let Some(i) = self.row_at((cursor.y - b.origin.y) as f64 + self.offset(&b)) {
                    let (mut row, l) = self.laid(&b, i);
                    return row.0.on_event(event, &l, cursor, out);
                }
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
                let page = ((b.size.height / self.avg_h()) as usize)
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
    fn animating(&self) -> bool {
        // A row can fade or slide in (`reveal_appear`): keep frames coming while one does.
        let Some(b) = self.shown.get() else {
            return false;
        };
        self.range(&b).any(|i| tree_animating(&self.laid(&b, i).0))
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        let t = self.ui.theme();
        let p = &t.palette;
        self.ui.register(self.id);
        self.shown.set(Some(b));
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
