//! Layout and event helpers shared by the widgets.

use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle, Size, Widget,
};
use alloy_ui_layout::{compute, Length, Node};

/// Build a layout node tree from a widget tree.
pub(crate) fn node_of<M>(w: &dyn Widget<M>) -> Node {
    let kids = w.children();
    Node {
        style: w.style(),
        intrinsic: if kids.is_empty() {
            w.measure(Size::new(f32::INFINITY, f32::INFINITY))
        } else {
            Size::default()
        },
        children: kids.iter().map(|e| node_of(e.0.as_ref())).collect(),
    }
}

/// Lay a widget (and its descendants) out inside exactly `bounds`.
///
/// Container widgets implement `Widget::arrange` with this.
pub fn arrange_widget<M>(w: &dyn Widget<M>, bounds: Rectangle) -> Layout {
    let mut node = node_of(w);
    node.style.width = Length::Fixed(bounds.size.width);
    node.style.height = Length::Fixed(bounds.size.height);
    node.style.limits = alloy_ui_core::style::Limits::NONE;
    compute(&node, bounds.origin, bounds.size)
}

/// Preferred size of a container from its style and children.
pub(crate) fn preferred<M>(w: &dyn Widget<M>) -> Size {
    node_of(w).preferred_size()
}

/// A copy of `l` shifted by `(dx, dy)`.
pub(crate) fn translate(l: &Layout, dx: f32, dy: f32) -> Layout {
    Layout {
        bounds: Rectangle::new(
            Point::new(l.bounds.origin.x + dx, l.bounds.origin.y + dy),
            l.bounds.size,
        ),
        children: l.children.iter().map(|c| translate(c, dx, dy)).collect(),
    }
}

/// Offer an event to children, topmost (last) first, until one consumes it.
pub(crate) fn dispatch<M>(
    children: &mut [Element<M>],
    event: &Event,
    layout: &Layout,
    cursor: Point,
    out: &mut Vec<M>,
) -> bool {
    for (child, l) in children.iter_mut().zip(&layout.children).rev() {
        if child.0.on_event(event, l, cursor, out) {
            return true;
        }
    }
    false
}

/// Draw children in order.
pub(crate) fn draw_children<M>(children: &[Element<M>], layout: &Layout, out: &mut DrawList) {
    for (child, l) in children.iter().zip(&layout.children) {
        child.0.draw(l, out);
    }
}

/// A stable key for "the widget occupying these bounds", used to remember
/// which widget a press started on across view rebuilds.
pub(crate) fn key_of(r: &Rectangle) -> (i32, i32) {
    (r.origin.x.round() as i32, r.origin.y.round() as i32)
}

/// A focus ring drawn just outside `bounds`.
pub(crate) fn focus_ring(
    bounds: Rectangle,
    radius: f32,
    color: alloy_ui_core::Color,
) -> DrawCommand {
    let b = Rectangle::new(
        Point::new(bounds.origin.x - 2.0, bounds.origin.y - 2.0),
        Size::new(bounds.size.width + 4.0, bounds.size.height + 4.0),
    );
    DrawCommand::Quad {
        bounds: b,
        fill: alloy_ui_core::Color::TRANSPARENT,
        radius: radius + 2.0,
        border: Some(alloy_ui_core::Border { color, width: 2.0 }),
        shadow: None,
    }
}

/// Style helper: widget style with explicit sizes.
pub(crate) fn sized(width: Length, height: Length) -> Style {
    Style {
        width,
        height,
        ..Style::default()
    }
}

/// True when any child reports `animating`.
pub(crate) fn any_animating<M>(children: &[Element<M>]) -> bool {
    children.iter().any(|c| c.0.animating())
}

/// Linear blend of two colours (straight alpha, sRGB-space; fine for small UI transitions).
pub(crate) fn mix(
    a: alloy_ui_core::Color,
    b: alloy_ui_core::Color,
    t: f32,
) -> alloy_ui_core::Color {
    let t = t.clamp(0.0, 1.0);
    let l = |x: f32, y: f32| x + (y - x) * t;
    alloy_ui_core::Color {
        r: l(a.r, b.r),
        g: l(a.g, b.g),
        b: l(a.b, b.b),
        a: l(a.a, b.a),
    }
}
