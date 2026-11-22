//! Vertical scroll container.

use crate::{
    ui::Ui,
    util::{self, translate},
};
use alloy_ui_core::{
    style::{Axis, Style},
    DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point, Rectangle, Size,
    Widget,
};
use alloy_ui_layout::Length;

const BAR: f32 = 6.0;
/// Width of the strip at the right edge that grabs the scrollbar.
const BAR_ZONE: f32 = 14.0;
/// Slot (in [`Ui`]) holding where on the thumb a drag took hold, in 1/16 px.
const K_DRAG: u64 = 0x5C02_0000_0000;
/// Memo (in [`Ui`]) of a touch-style drag: `[last_move_ms, last_y, velocity, active]`.
const K_TOUCH: u64 = 0x5C04_0000_0000;
/// Pointer travel (px) before a press on content becomes a scroll drag.
const TOUCH_SLOP: f32 = 4.0;
/// Slot (in [`Ui`]) holding the last seen maximum offset of a stick-to-bottom area, in 1/16 px.
const K_STICK: u64 = 0x5C05_0000_0000;

/// Scrolls one child vertically. Give the child `Fill` width and `Shrink`
/// height so it is as wide as the viewport and as tall as its content.
/// The offset lives in the [`Ui`] under `id`.
pub struct Scrollable<M> {
    ui: Ui,
    id: u64,
    style: Style,
    touch: bool,
    stick: bool,
    child: Vec<Element<M>>,
}

impl<M: 'static> Scrollable<M> {
    /// Scroll `child`; `id` must be unique per window.
    pub fn new(ui: &Ui, id: u64, child: Element<M>) -> Self {
        Self {
            ui: ui.clone(),
            id,
            style: Style {
                axis: Axis::Vertical,
                width: Length::Fill(1),
                height: Length::Fill(1),
                ..Style::default()
            },
            touch: false,
            stick: false,
            child: vec![child],
        }
    }
    /// Let a pointer drag on the content scroll it, the way a touch screen
    /// does: it follows the pointer, stretches past the ends with resistance,
    /// and keeps coasting when released. Presses that land on a button or
    /// other control still go to the control.
    pub fn touch_drag(mut self) -> Self {
        self.touch = true;
        self
    }
    /// Stay pinned to the bottom, like a chat log: while the view is at the bottom (or has never
    /// been scrolled) it follows content that grows, and it starts at the bottom. Scroll up and
    /// it stays where you are until you scroll back down to the end.
    pub fn stick_bottom(mut self) -> Self {
        self.stick = true;
        self
    }
    /// Follow growing content when pinned (see [`Scrollable::stick_bottom`]).
    fn follow(&self, layout: &Layout) {
        let max = Self::max_offset(layout);
        let key = self.id ^ K_STICK;
        match self.ui.slot(key).map(|v| v as f32 / 16.0) {
            None => self.ui.scroll_jump(self.id, max, max),
            Some(last) if (max - last).abs() > 0.01 => {
                // Pinned if the target was at (or within a pixel of) the old end.
                if self.ui.scroll_offset(self.id) >= last - 1.0 {
                    self.ui.scroll_jump(self.id, max, max);
                }
            }
            Some(_) => {}
        }
        self.ui.set_slot(key, Some((max * 16.0) as i64));
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.style.width = w;
        self
    }
    /// Height sizing.
    pub fn height(mut self, h: Length) -> Self {
        self.style.height = h;
        self
    }
    /// Box as an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }

    fn content_height(layout: &Layout) -> f32 {
        layout
            .children
            .first()
            .map_or(0.0, |c| c.bounds.size.height)
    }
    fn max_offset(layout: &Layout) -> f32 {
        (Self::content_height(layout) - layout.bounds.size.height).max(0.0)
    }
    /// Where scrolling is heading (the clamped target).
    fn offset(&self, layout: &Layout) -> f32 {
        self.ui
            .scroll_offset(self.id)
            .clamp(0.0, Self::max_offset(layout))
    }
    /// The child layout, shifted by the scroll offset and positioned at the viewport top.
    /// Where the content is drawn: the target followed by the scroll spring.
    fn shown(&self, layout: &Layout) -> f32 {
        self.ui
            .scroll_view(self.id, Self::max_offset(layout), layout.bounds.size.height)
    }
    /// The scrollbar thumb for the current drawn offset, if the content overflows.
    fn thumb(&self, layout: &Layout) -> Option<Rectangle> {
        let b = layout.bounds;
        let content = Self::content_height(layout);
        if content <= b.size.height + 0.5 || b.size.height <= 0.0 {
            return None;
        }
        let thumb_h = (b.size.height * b.size.height / content)
            .max(24.0)
            .min(b.size.height);
        let travel = b.size.height - thumb_h;
        let max = Self::max_offset(layout);
        let y = if max > 0.0 {
            b.origin.y + travel * self.shown(layout).clamp(0.0, max) / max
        } else {
            b.origin.y
        };
        Some(Rectangle::new(
            Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
            Size::new(BAR, thumb_h),
        ))
    }
    /// Put the thumb's top at `y - grab` and scroll to match, with no glide.
    fn drag_to(&self, layout: &Layout, y: f32, grab: f32) {
        if let Some(th) = self.thumb(layout) {
            let b = layout.bounds;
            let travel = (b.size.height - th.size.height).max(1.0);
            let top = (y - grab - b.origin.y).clamp(0.0, travel);
            let max = Self::max_offset(layout);
            self.ui.scroll_jump(self.id, top / travel * max, max);
        }
    }
    fn shifted(&self, layout: &Layout) -> Layout {
        let off = self.shown(layout);
        Layout {
            bounds: layout.bounds,
            children: layout
                .children
                .iter()
                .map(|c| translate(c, 0.0, -off))
                .collect(),
        }
    }
}

impl<M: 'static> Scrollable<M> {
    /// Touch-style dragging. `Some(used)` when this event was handled here.
    fn touch_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> Option<bool> {
        let b = layout.bounds;
        let key = self.id ^ K_TOUCH;
        let m = self.ui.memo(key);
        let wall = self.ui.wall_ms();
        match event {
            Event::MousePressed(MouseButton::Left)
                if b.contains(cursor) && cursor.x < b.origin.x + b.size.width - BAR_ZONE =>
            {
                // Children first: a control keeps its press.
                let shifted = self.shifted(layout);
                if util::dispatch(&mut self.child, event, &shifted, cursor, out) {
                    return Some(true);
                }
                self.ui.scroll_catch(self.id);
                self.ui.set_memo(key, [wall as f32, cursor.y, 0.0, 1.0]);
                Some(true)
            }
            Event::CursorMoved(p) if m[3] >= 1.0 => {
                let dy = p.y - m[1];
                // Not yet a drag: wait for the slop, remembering where the press began.
                if m[3] < 2.0 && dy.abs() < TOUCH_SLOP {
                    return Some(true);
                }
                let dt = (wall as f32 - m[0]).max(1.0) / 1000.0;
                let inst = dy / dt;
                let v = if m[3] < 2.0 {
                    inst
                } else {
                    0.6 * inst + 0.4 * m[2]
                };
                self.ui.scroll_wheel_elastic(
                    self.id,
                    dy,
                    Self::max_offset(layout),
                    b.size.height,
                    true,
                );
                self.ui.set_memo(key, [wall as f32, p.y, v, 2.0]);
                Some(true)
            }
            Event::MouseReleased(MouseButton::Left) if m[3] >= 1.0 => {
                self.ui.set_memo(key, [0.0; 4]);
                if m[3] >= 2.0 {
                    let quiet = wall as f32 - m[0] > 90.0;
                    let v = if quiet { 0.0 } else { m[2] };
                    // Pointer moving down means the offset is decreasing.
                    self.ui.scroll_fling(self.id, -v);
                    return Some(true);
                }
                None
            }
            _ => None,
        }
    }
}

impl<M: 'static> Widget<M> for Scrollable<M> {
    fn style(&self) -> Style {
        // A scrollable adopts the child's width unless told otherwise; it never
        // grows to the content height when constrained.
        self.style
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
        // An overlay press (open popover or context menu inside the content) arrives with the
        // pointer parked off-screen; the owner is somewhere below, so forward it.
        if self.ui.hit_pending() && matches!(event, Event::MousePressed(_) | Event::Scrolled { .. })
        {
            let shifted = self.shifted(layout);
            return util::dispatch(&mut self.child, event, &shifted, cursor, out);
        }
        let inside = layout.bounds.contains(cursor);
        let b = layout.bounds;
        let max = Self::max_offset(layout);
        // Dragging the scrollbar thumb owns the pointer until release.
        if let Some(grab) = self.ui.slot(self.id ^ K_DRAG) {
            match event {
                Event::CursorMoved(p) => {
                    self.drag_to(layout, p.y, grab as f32 / 16.0);
                    return true;
                }
                Event::MouseReleased(MouseButton::Left) => {
                    self.ui.set_slot(self.id ^ K_DRAG, None);
                    return true;
                }
                _ => {}
            }
        }
        if let (Event::MousePressed(MouseButton::Left), Some(th)) = (event, self.thumb(layout)) {
            if inside && cursor.x >= b.origin.x + b.size.width - BAR_ZONE {
                if th.contains(Point::new(th.origin.x + 1.0, cursor.y)) {
                    let grab = cursor.y - th.origin.y;
                    self.ui
                        .set_slot(self.id ^ K_DRAG, Some((grab * 16.0) as i64));
                } else {
                    // A click on the track pages toward the pointer, gliding.
                    let page = (b.size.height * 0.9).max(1.0);
                    let dir = if cursor.y < th.origin.y { -1.0 } else { 1.0 };
                    self.ui
                        .scroll_to(self.id, self.offset(layout) + dir * page, max);
                }
                return true;
            }
        }
        if let Event::Scrolled { dy, .. } = event {
            if inside {
                let before = self.offset(layout);
                let moved = self
                    .ui
                    .scroll_wheel_elastic(self.id, *dy, max, b.size.height, false);
                // Let nested scrollables go first; consume only if we moved.
                let shifted = self.shifted(layout);
                if util::dispatch(&mut self.child, event, &shifted, cursor, out) {
                    self.ui.set_scroll_offset(self.id, before);
                    return true;
                }
                return moved;
            }
            return false;
        }
        if self.touch {
            if let Some(used) = self.touch_event(event, layout, cursor, out) {
                return used;
            }
        }
        let is_pointer = matches!(
            event,
            Event::MousePressed(_) | Event::MouseReleased(_) | Event::CursorMoved(_)
        );
        if is_pointer
            && !inside
            && !matches!(event, Event::MouseReleased(_) | Event::CursorMoved(_))
        {
            return false;
        }
        let shifted = self.shifted(layout);
        // Pointer presses outside the viewport must not reach clipped children.
        if util::dispatch(&mut self.child, event, &shifted, cursor, out) {
            return true;
        }
        // Paging keys scroll the area under the pointer when nothing else wanted them.
        if let (Event::KeyPressed(Key::Named(k), _), true) = (event, inside && max > 0.0) {
            let page = (b.size.height * 0.9).max(1.0);
            let target = self.offset(layout);
            let to = match k.as_str() {
                "PageDown" => target + page,
                "PageUp" => target - page,
                "Home" => 0.0,
                "End" => max,
                _ => return false,
            };
            self.ui.scroll_to(self.id, to, max);
            return true;
        }
        false
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        if self.stick {
            self.follow(layout);
        }
        out.push(DrawCommand::PushClip(b));
        let shifted = self.shifted(layout);
        let outer = self.ui.clip();
        self.ui.set_clip(Some(b));
        util::draw_children(&self.child, &shifted, out);
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);

        if let Some(th) = self.thumb(layout) {
            let t = self.ui.theme();
            let dragging = self.ui.slot(self.id ^ K_DRAG).is_some();
            let near = dragging
                || (b.contains(self.ui.cursor())
                    && self.ui.cursor().x >= b.origin.x + b.size.width - BAR_ZONE);
            // The thumb firms up when the pointer is over the scrollbar strip.
            let k = self.ui.ease(0x5C03_0000_0000 ^ self.id, near as u8 as f32);
            out.push(DrawCommand::Quad {
                bounds: th,
                fill: t.palette.border_strong.with_alpha(0.55 + 0.30 * k),
                radius: BAR / 2.0,
                border: None,
                shadow: None,
            });
        }
    }
}
