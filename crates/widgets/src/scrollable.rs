//! Vertical scroll container.

use crate::{
    ui::Ui,
    util::{self, translate},
};
use alloy_ui_core::{
    style::{Axis, Style},
    DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

const BAR: f32 = 6.0;

/// Scrolls one child vertically. Give the child `Fill` width and `Shrink`
/// height so it is as wide as the viewport and as tall as its content.
/// The offset lives in the [`Ui`] under `id`.
pub struct Scrollable<M> {
    ui: Ui,
    id: u64,
    style: Style,
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
            child: vec![child],
        }
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
    fn shown(&self, layout: &Layout) -> f32 {
        let m = self.ui.theme().motion;
        self.ui.ease_with(
            0x5C01_0000_0000 ^ self.id,
            self.offset(layout),
            m.normal_ms as u64,
            m.easing,
        )
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
        let inside = layout.bounds.contains(cursor);
        if let Event::Scrolled { dy, .. } = event {
            if inside {
                let before = self.offset(layout);
                let next = (before - dy).clamp(0.0, Self::max_offset(layout));
                self.ui.set_scroll_offset(self.id, next);
                // Let nested scrollables go first; consume only if we moved.
                let shifted = self.shifted(layout);
                if util::dispatch(&mut self.child, event, &shifted, cursor, out) {
                    self.ui.set_scroll_offset(self.id, before);
                    return true;
                }
                return next != before;
            }
            return false;
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
        util::dispatch(&mut self.child, event, &shifted, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let b = layout.bounds;
        out.push(DrawCommand::PushClip(b));
        let shifted = self.shifted(layout);
        let outer = self.ui.clip();
        self.ui.set_clip(Some(b));
        util::draw_children(&self.child, &shifted, out);
        self.ui.set_clip(outer);
        out.push(DrawCommand::PopClip);

        let content = Self::content_height(layout);
        if content > b.size.height + 0.5 && b.size.height > 0.0 {
            let t = self.ui.theme();
            let frac = b.size.height / content;
            let thumb_h = (b.size.height * frac).max(24.0).min(b.size.height);
            let travel = b.size.height - thumb_h;
            let max = Self::max_offset(layout);
            let y = if max > 0.0 {
                b.origin.y + travel * self.shown(layout).clamp(0.0, max) / max
            } else {
                b.origin.y
            };
            out.push(DrawCommand::Quad {
                bounds: Rectangle::new(
                    Point::new(b.origin.x + b.size.width - BAR - 2.0, y),
                    Size::new(BAR, thumb_h),
                ),
                fill: t.palette.border_strong.with_alpha(0.55),
                radius: BAR / 2.0,
                border: None,
                shadow: None,
            });
        }
    }
}
