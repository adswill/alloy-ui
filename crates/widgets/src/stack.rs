//! Layering: children drawn on top of each other in one box. Needed for
//! frosted glass (there must be something behind it to blur), badges on
//! avatars, and overlays inside a card.

use crate::util;
use alloy_ui_core::{
    style::{Align, Style},
    DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

/// Where a layer sits inside the stack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// Horizontal alignment.
    pub x: Align,
    /// Vertical alignment.
    pub y: Align,
    /// Extra shift in logical pixels after alignment.
    pub offset: Point,
}

impl Default for Place {
    fn default() -> Self {
        Self {
            x: Align::Start,
            y: Align::Start,
            offset: Point::new(0.0, 0.0),
        }
    }
}

/// Children layered back to front. The box is as large as its largest child;
/// a child with `Fill` sizing stretches over it, any other is placed by its
/// [`Place`]. Later children are on top and see events first.
pub struct Stack<M> {
    children: Vec<Element<M>>,
    places: Vec<Place>,
    width: Length,
    height: Length,
}

impl<M: 'static> Stack<M> {
    /// A stack of `(child, place)` layers, first at the back.
    pub fn new(layers: Vec<(Element<M>, Place)>) -> Self {
        let (children, places) = layers.into_iter().unzip();
        Self {
            children,
            places,
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }
    /// Width sizing.
    pub fn width(mut self, w: Length) -> Self {
        self.width = w;
        self
    }
    /// Height sizing.
    pub fn height(mut self, h: Length) -> Self {
        self.height = h;
        self
    }
    /// As an element.
    pub fn into_element(self) -> Element<M> {
        Element::new(self)
    }
}

impl<M: 'static> Widget<M> for Stack<M> {
    fn style(&self) -> Style {
        util::sized(self.width, self.height)
    }
    /// The largest layer: layers overlap, so sizes do not add up. The layout
    /// engine sees the stack as a leaf (see [`Widget::children`]).
    fn measure(&self, _available: Size) -> Size {
        let mut size = Size::new(0.0, 0.0);
        for c in &self.children {
            let p = util::node_of(c.0.as_ref()).preferred_size();
            size.width = size.width.max(p.width);
            size.height = size.height.max(p.height);
        }
        size
    }
    fn arrange(&self, bounds: Rectangle) -> Layout {
        let children = self
            .children
            .iter()
            .zip(&self.places)
            .map(|(c, p)| {
                let st = c.0.style();
                let pref = util::node_of(c.0.as_ref()).preferred_size();
                let w = if matches!(st.width, Length::Fill(_)) {
                    bounds.size.width
                } else {
                    pref.width.min(bounds.size.width)
                };
                let h = if matches!(st.height, Length::Fill(_)) {
                    bounds.size.height
                } else {
                    pref.height.min(bounds.size.height)
                };
                let place = |a: Align, free: f32| match a {
                    Align::Start => 0.0,
                    Align::Center => free / 2.0,
                    Align::End => free,
                };
                let x = bounds.origin.x + place(p.x, bounds.size.width - w) + p.offset.x;
                let y = bounds.origin.y + place(p.y, bounds.size.height - h) + p.offset.y;
                c.0.arrange(Rectangle::new(Point::new(x, y), Size::new(w, h)))
            })
            .collect();
        Layout { bounds, children }
    }
    fn on_event(
        &mut self,
        event: &Event,
        layout: &Layout,
        cursor: Point,
        out: &mut Vec<M>,
    ) -> bool {
        let inner = self.arrange(layout.bounds);
        util::dispatch(&mut self.children, event, &inner, cursor, out)
    }
    fn animating(&self) -> bool {
        util::any_animating(&self.children)
    }
    fn draw(&self, layout: &Layout, out: &mut DrawList) {
        let inner = self.arrange(layout.bounds);
        util::draw_children(&self.children, &inner, out);
    }
}
