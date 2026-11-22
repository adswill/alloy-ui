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

fn arrange_uncached<M>(w: &dyn Widget<M>, bounds: Rectangle) -> Layout {
    let mut node = node_of(w);
    node.style.width = Length::Fixed(bounds.size.width);
    node.style.height = Length::Fixed(bounds.size.height);
    node.style.limits = alloy_ui_core::style::Limits::NONE;
    compute(&node, bounds.origin, bounds.size)
}

/// Hash everything `node_of` + `compute` read from a widget tree: each node's
/// style, the child count, and each leaf's measured size. Allocation-free.
fn fingerprint<M>(
    w: &dyn Widget<M>,
    h: &mut std::collections::hash_map::DefaultHasher,
    nodes: &mut usize,
) {
    *nodes += 1;
    use alloy_ui_core::style::Align;
    use std::hash::Hasher;
    let s = w.style();
    let len = |h: &mut std::collections::hash_map::DefaultHasher, l: Length| match l {
        Length::Fill(n) => {
            h.write_u8(0);
            h.write_u16(n)
        }
        Length::Shrink => h.write_u8(1),
        Length::Fixed(v) => {
            h.write_u8(2);
            h.write_u32(v.to_bits())
        }
    };
    let align = |a: Align| match a {
        Align::Start => 0u8,
        Align::Center => 1,
        Align::End => 2,
    };
    h.write_u8(s.axis as u8);
    h.write_u8(align(s.align));
    h.write_u8(s.align_self.map_or(3, align));
    h.write_u32(s.spacing.to_bits());
    for v in [
        s.padding.top,
        s.padding.right,
        s.padding.bottom,
        s.padding.left,
        s.limits.min.width,
        s.limits.min.height,
        s.limits.max.width,
        s.limits.max.height,
    ] {
        h.write_u32(v.to_bits());
    }
    len(h, s.width);
    len(h, s.height);
    let kids = w.children();
    h.write_usize(kids.len());
    if kids.is_empty() {
        let m = w.measure(Size::new(f32::INFINITY, f32::INFINITY));
        h.write_u32(m.width.to_bits());
        h.write_u32(m.height.to_bits());
    }
    for k in kids {
        fingerprint(k.0.as_ref(), h, nodes);
    }
}

/// Trees smaller than this are cheaper to lay out than to fingerprint and store.
const CACHE_MIN_NODES: usize = 24;

thread_local! {
    /// Recent layouts, keyed by (tree fingerprint, bounds): most frames re-lay
    /// out a tree that did not change.
    static LAYOUTS: std::cell::RefCell<Vec<(u64, Layout)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Lay a widget (and its descendants) out inside exactly `bounds`.
///
/// Container widgets implement `Widget::arrange` with this. Big trees are
/// memoised by (tree fingerprint, bounds): an unchanged tree at unchanged bounds
/// returns the stored layout instead of rebuilding and re-solving the node
/// tree. The result is identical either way.
pub fn arrange_widget<M>(w: &dyn Widget<M>, bounds: Rectangle) -> Layout {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for v in [
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    ] {
        h.write_u32(v.to_bits());
    }
    let mut nodes = 0;
    fingerprint(w, &mut h, &mut nodes);
    if nodes < CACHE_MIN_NODES {
        return arrange_uncached(w, bounds);
    }
    let key = h.finish();
    if let Some(l) = LAYOUTS.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, l)| l.clone())
    }) {
        return l;
    }
    let l = arrange_uncached(w, bounds);
    LAYOUTS.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() >= 8 {
            c.remove(0);
        }
        c.push((key, l.clone()));
    });
    l
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
    crate::xform::transform(cmd, &crate::xform::Xf::fade_move(alpha, dx, dy))
}

/// A single-line text command in the sans family.
pub(crate) fn label_cmd(
    origin: Point,
    content: impl Into<String>,
    style: &alloy_ui_theme::TextStyle,
    weight: u16,
    color: alloy_ui_core::Color,
) -> DrawCommand {
    DrawCommand::Text {
        family: alloy_ui_core::FontFamily::Sans,
        weight,
        line_height: Some(style.line_height),
        origin,
        content: content.into(),
        size: style.size,
        color,
        max_width: None,
    }
}

/// A one-or-two-line chart tooltip on the overlay layer: `head` (optional, bold) over `value`,
/// placed beside `anchor` and kept inside `within`.
pub(crate) fn chart_tip(
    ui: &crate::ui::Ui,
    anchor: Point,
    within: Rectangle,
    head: &str,
    value: &str,
) {
    use alloy_ui_core::{Border, Size};
    let t = ui.theme();
    let p = &t.palette;
    let cap = t.typography.caption;
    let m = ui.metrics();
    let w = |s: &str, wt: u16| m.width_styled(s, cap.size, wt, alloy_ui_core::FontFamily::Sans);
    let pad = 8.0;
    let inner = w(head, 600).max(w(value, 600));
    let lines = if head.is_empty() { 1.0 } else { 2.0 };
    let (bw, bh) = (inner + 2.0 * pad, lines * cap.line_height + 2.0 * pad);
    let mut x = anchor.x + 12.0;
    if x + bw > within.origin.x + within.size.width {
        x = anchor.x - 12.0 - bw;
    }
    x = x.max(within.origin.x);
    let y = (anchor.y - bh - 10.0).max(within.origin.y);
    let mut cmds = vec![DrawCommand::Quad {
        bounds: Rectangle::new(Point::new(x, y), Size::new(bw, bh)),
        fill: p.surface_raised,
        radius: t.radii.medium,
        border: Some(Border {
            color: p.border,
            width: 1.0,
        }),
        shadow: Some(t.elevation.medium),
    }];
    let mut ty = y + pad;
    if !head.is_empty() {
        cmds.push(label_cmd(
            Point::new(x + pad, ty),
            head,
            &cap,
            400,
            p.text_secondary,
        ));
        ty += cap.line_height;
    }
    cmds.push(label_cmd(Point::new(x + pad, ty), value, &cap, 600, p.text));
    ui.overlay(None, None, cmds);
}
