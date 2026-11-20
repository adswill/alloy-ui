//! Row and column containers.

use crate::util;
use alloy_ui_core::{
    style::{Align, Axis, Padding, Style},
    DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::Length;

macro_rules! flex {
    ($name:ident, $axis:expr, $doc:literal) => {
        #[doc = $doc]
        pub struct $name<M> {
            style: Style,
            children: Vec<Element<M>>,
        }

        impl<M: 'static> $name<M> {
            /// New container holding `children`.
            pub fn new(children: Vec<Element<M>>) -> Self {
                Self {
                    style: Style {
                        axis: $axis,
                        ..Style::default()
                    },
                    children,
                }
            }
            /// Gap between children.
            pub fn spacing(mut self, s: f32) -> Self {
                self.style.spacing = s;
                self
            }
            /// Padding around the children.
            pub fn padding(mut self, p: Padding) -> Self {
                self.style.padding = p;
                self
            }
            /// Cross-axis alignment of children.
            pub fn align(mut self, a: Align) -> Self {
                self.style.align = a;
                self
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
            /// Append a child.
            pub fn push(mut self, child: impl Into<Element<M>>) -> Self {
                self.children.push(child.into());
                self
            }
            /// Box as an element.
            pub fn into_element(self) -> Element<M> {
                Element::new(self)
            }
        }

        impl<M: 'static> Widget<M> for $name<M> {
            fn style(&self) -> Style {
                self.style
            }
            fn measure(&self, _available: Size) -> Size {
                // `node_of` treats a childless widget as a leaf and calls `measure`, which
                // would call `node_of` again: an empty row or column recursed until the
                // stack overflowed. An empty container has no content.
                if self.children.is_empty() {
                    return alloy_ui_layout::Node {
                        style: self.style,
                        intrinsic: Size::default(),
                        children: Vec::new(),
                    }
                    .preferred_size();
                }
                util::preferred(self)
            }
            fn children(&self) -> &[Element<M>] {
                &self.children
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
                util::dispatch(&mut self.children, event, layout, cursor, out)
            }
            fn animating(&self) -> bool {
                util::any_animating(&self.children)
            }
            fn draw(&self, layout: &Layout, out: &mut DrawList) {
                util::draw_children(&self.children, layout, out);
            }
        }
    };
}

flex!(Row, Axis::Horizontal, "Lays children out left to right.");
flex!(Column, Axis::Vertical, "Lays children out top to bottom.");
