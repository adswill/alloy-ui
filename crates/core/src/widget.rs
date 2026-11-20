//! The widget contract.

use crate::{
    draw::DrawList,
    event::Event,
    geometry::{Point, Rectangle, Size},
};

/// Resolved position and size of a widget, with its children's layouts.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    /// Absolute bounds in logical pixels.
    pub bounds: Rectangle,
    /// Child layouts, in widget child order.
    pub children: Vec<Layout>,
}

/// A node in the view tree. `M` is the application's message type.
///
/// Widgets are rebuilt every `view`; persistent widget state (focus,
/// scroll offset, caret) lives in the runtime's state tree keyed by position.
pub trait Widget<M> {
    /// Layout style (sizing, padding, direction). Default: shrink-wrapped.
    fn style(&self) -> crate::style::Style {
        crate::style::Style::default()
    }
    /// Preferred size given the available space (layout engine entry point).
    fn measure(&self, available: Size) -> Size;
    /// Child elements, for layout and event traversal.
    fn children(&self) -> &[Element<M>] {
        &[]
    }
    /// Place children inside `bounds`. Default: no children.
    fn arrange(&self, bounds: Rectangle) -> Layout {
        Layout {
            bounds,
            children: Vec::new(),
        }
    }
    /// Handle an event; push resulting messages into `out`. Return true if consumed.
    fn on_event(
        &mut self,
        _event: &Event,
        _layout: &Layout,
        _cursor: Point,
        _out: &mut Vec<M>,
    ) -> bool {
        false
    }
    /// Emit draw commands.
    fn draw(&self, layout: &Layout, out: &mut DrawList);
}

/// A boxed widget: the unit `view` functions return.
pub struct Element<M>(pub Box<dyn Widget<M>>);

impl<M> Element<M> {
    /// Box a widget as an element.
    pub fn new(widget: impl Widget<M> + 'static) -> Self {
        Self(Box::new(widget))
    }
}
