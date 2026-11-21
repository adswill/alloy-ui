//! Enter and exit transitions: fade, slide and collapse a child as it is
//! shown or hidden, then stop drawing it once the exit has played.

use crate::{motion::Presence, ui::Ui, util};
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// Shows or hides one child with a short ease-out transition.
///
/// By default it fades. [`Reveal::slide`] adds a small upward drift on enter,
/// and [`Reveal::collapse`] animates the occupied height too, so content
/// below glides rather than jumps (accordions, inline errors, banners).
/// Under reduced motion it switches instantly.
pub struct Reveal<M> {
    ui: Ui,
    key: u64,
    show: bool,
    ms: u32,
    slide: f32,
    collapse: bool,
    child: Vec<Element<M>>,
}

impl<M: 'static> Reveal<M> {
    /// Show `child` while `show` is true. `key` identifies this transition
    /// across view rebuilds (the facade derives it from the call site).
    pub fn new(ui: Ui, key: u64, show: bool, child: Element<M>) -> Self {
        let ms = ui.theme().motion.normal_ms;
        Self {
            ui,
            key,
            show,
            ms,
            slide: 0.0,
            collapse: false,
            child: vec![child],
        }
    }
    /// Drift the child up by `px` logical pixels as it enters (and back down
    /// as it leaves). A few pixels is plenty.
    pub fn slide(mut self, px: f32) -> Self {
        self.slide = px;
        self
    }
    /// Animate the occupied height as well as opacity.
    pub fn collapse(mut self, on: bool) -> Self {
        self.collapse = on;
        self
    }
    /// Duration in milliseconds (default: the theme's normal motion).
    pub fn duration(mut self, ms: u32) -> Self {
        self.ms = ms;
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
    fn presence(&self) -> Presence {
        self.ui.presence(self.key, self.show, self.ms)
    }
    fn natural(&self) -> Size {
        util::node_of(self.child[0].0.as_ref()).preferred_size()
    }
}

impl<M: 'static> Widget<M> for Reveal<M> {
    fn style(&self) -> Style {
        let p = self.presence();
        let inner = self.child[0].0.style();
        let mut s = Style {
            width: inner.width,
            height: Length::Shrink,
            ..Style::default()
        };
        if !p.mounted {
            s.height = Length::Fixed(0.0);
            s.width = Length::Fixed(0.0);
        } else if self.collapse && p.progress < 1.0 {
            s.height = Length::Fixed(self.natural().height * p.progress);
        }
        s
    }
    fn measure(&self, _available: Size) -> Size {
        util::preferred(self)
    }
    fn children(&self) -> &[Element<M>] {
        &self.child
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        // The child keeps its natural height while the box grows or shrinks.
        let h = if self.collapse && self.presence().mounted {
            self.natural().height
        } else {
            bounds.size.height
        };
        let child = self.child[0].0.arrange(Rectangle::new(
            bounds.origin,
            Size::new(bounds.size.width, h),
        ));
        Layout {
            bounds,
            children: vec![child],
        }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        // Content on its way out is no longer interactive.
        if !self.show {
            return false;
        }
        util::dispatch(&mut self.child, event, layout, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.child)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let p = self.presence();
        if !p.mounted {
            return;
        }
        if p.progress >= 0.999 {
            util::draw_children(&self.child, layout, out);
            return;
        }
        let mut tmp = DrawList::default();
        util::draw_children(&self.child, layout, &mut tmp);
        let dy = (1.0 - p.progress) * self.slide;
        let clip = self.collapse;
        if clip {
            out.push(DrawCommand::PushClip(layout.bounds));
        }
        for c in tmp.0 {
            out.push(util::fade_translate(c, p.progress, 0.0, dy));
        }
        if clip {
            out.push(DrawCommand::PopClip);
        }
    }
}
