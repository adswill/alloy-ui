//! Layout and event helpers shared by the widgets.

use crate::ui::Ui;
use alloy_ui_core::{
    style::Style, DrawCommand, DrawList, Element, Event, Key, Layout, MouseButton, Point,
    Rectangle, Size, Widget,
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

/// A focus ring drawn just outside `bounds`: 2px wide with a 1px gap, so it reads as a halo
/// and never merges with the control's own border.
pub(crate) fn focus_ring(
    bounds: Rectangle,
    radius: f32,
    color: alloy_ui_core::Color,
) -> DrawCommand {
    let b = Rectangle::new(
        Point::new(bounds.origin.x - 3.0, bounds.origin.y - 3.0),
        Size::new(bounds.size.width + 6.0, bounds.size.height + 6.0),
    );
    DrawCommand::Quad {
        bounds: b,
        fill: alloy_ui_core::Color::TRANSPARENT,
        radius: radius + 3.0,
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

/// `top` painted over `base` (straight alpha); the result is as opaque as `base` and `top` allow.
pub(crate) fn over(base: alloy_ui_core::Color, top: alloy_ui_core::Color) -> alloy_ui_core::Color {
    let a = top.a + base.a * (1.0 - top.a);
    if a <= 0.0 {
        return alloy_ui_core::Color::TRANSPARENT;
    }
    let c = |t: f32, b: f32| (t * top.a + b * base.a * (1.0 - top.a)) / a;
    alloy_ui_core::Color {
        r: c(top.r, base.r),
        g: c(top.g, base.g),
        b: c(top.b, base.b),
        a,
    }
}

/// Shared press/release/Space handling of the click-to-activate controls (checkbox, switch, radio).
///
/// Returns `(consumed, activated)`: whether the widget took the event, and whether it
/// completed an activation (a click released over it, or Space while focused).
pub(crate) fn activation(
    ui: &Ui,
    id: Option<u64>,
    enabled: bool,
    event: &Event,
    b: &Rectangle,
    cursor: Point,
) -> (bool, bool) {
    match event {
        Event::MousePressed(MouseButton::Left) if b.contains(cursor) => {
            if enabled {
                ui.set_pressed(Some(key_of(b)));
                if id.is_some() {
                    ui.set_focus(id);
                }
            }
            (true, false)
        }
        Event::MouseReleased(MouseButton::Left) => {
            let was = ui.pressed() == Some(key_of(b));
            (was, was && b.contains(cursor))
        }
        Event::KeyPressed(Key::Character(c), _) if enabled && c == " " && ui.is_focused(id) => {
            (true, true)
        }
        _ => (false, false),
    }
}

/// The body-text label of a control, vertically centred in `b` and starting `indent` from its left.
pub(crate) fn control_label(
    ui: &Ui,
    label: &str,
    b: &Rectangle,
    indent: f32,
    enabled: bool,
) -> DrawCommand {
    let t = ui.theme();
    let s = t.typography.body;
    DrawCommand::Text {
        family: alloy_ui_core::FontFamily::Sans,
        weight: 400,
        line_height: None,
        origin: Point::new(
            b.origin.x + indent + t.spacing.sm,
            b.origin.y + (b.size.height - s.line_height) / 2.0,
        ),
        content: label.to_owned(),
        size: s.size,
        color: if enabled {
            t.palette.text
        } else {
            t.palette.text_disabled
        },
        max_width: None,
    }
}

/// `cmd` with its colours' alpha scaled by `alpha` and its geometry moved by
/// `(dx, dy)`. This is how a group fades without a render layer: every
/// primitive is faded on its own, which is exact for non-overlapping content.
pub(crate) fn fade_translate(cmd: DrawCommand, alpha: f32, dx: f32, dy: f32) -> DrawCommand {
    use alloy_ui_core::{Border, Shadow};
    let c = |mut c: alloy_ui_core::Color| {
        c.a *= alpha;
        c
    };
    let r = |b: Rectangle| Rectangle::new(Point::new(b.origin.x + dx, b.origin.y + dy), b.size);
    let p = |q: Point| Point::new(q.x + dx, q.y + dy);
    let border = |b: Option<Border>| {
        b.map(|b| Border {
            color: c(b.color),
            ..b
        })
    };
    let shadow = |s: Option<Shadow>| {
        s.map(|s| Shadow {
            color: c(s.color),
            ..s
        })
    };
    match cmd {
        DrawCommand::Quad {
            bounds,
            fill,
            radius,
            border: b,
            shadow: s,
        } => DrawCommand::Quad {
            bounds: r(bounds),
            fill: c(fill),
            radius,
            border: border(b),
            shadow: shadow(s),
        },
        DrawCommand::Text {
            origin,
            content,
            size,
            color,
            max_width,
            weight,
            family,
            line_height,
        } => DrawCommand::Text {
            origin: p(origin),
            content,
            size,
            color: c(color),
            max_width,
            weight,
            family,
            line_height,
        },
        DrawCommand::Line {
            from,
            to,
            width,
            color,
        } => DrawCommand::Line {
            from: p(from),
            to: p(to),
            width,
            color: c(color),
        },
        DrawCommand::GradientQuad {
            bounds,
            from,
            to,
            angle_deg,
            radius,
            border: b,
            shadow: s,
        } => DrawCommand::GradientQuad {
            bounds: r(bounds),
            from: c(from),
            to: c(to),
            angle_deg,
            radius,
            border: border(b),
            shadow: shadow(s),
        },
        DrawCommand::Image { bounds, id } => DrawCommand::Image {
            bounds: r(bounds),
            id,
        },
        DrawCommand::PushClip(b) => DrawCommand::PushClip(r(b)),
        other => other,
    }
}
