//! The extension surface: everything a third-party widget crate needs that the built-in
//! widgets use internally, as a small, documented set of free functions.
//!
//! A widget outside this crate implements [`Widget`](alloy_ui_core::Widget) like the built-ins
//! do. These helpers cover what that needs beyond the trait itself: joining the Tab order,
//! drawing the same focus ring, remembering a press across rebuilds, small per-widget state
//! that survives `view`, and colour maths on theme colours. They are the stable part of the
//! plugin API; the `pub(crate)` internals behind them may change without notice.

use crate::a11y::{A11yNode, Role};
use crate::ui::Ui;
use crate::util;
use alloy_ui_core::{Color, DrawCommand, DrawList, Element, Event, Layout, Point, Rectangle};

/// Add widget `id` to this frame's Tab order. Call from `draw`, for focusable widgets only.
pub fn focusable(ui: &Ui, id: u64) {
    ui.register(id);
}

/// The focus ring the built-in controls draw around `bounds` (a halo just outside it).
pub fn focus_ring(bounds: Rectangle, radius: f32, color: Color) -> DrawCommand {
    util::focus_ring(bounds, radius, color)
}

/// Remembered integer state of widget `id` (a drag anchor, a hover index, a counter). Widgets
/// are rebuilt every `view`, so anything that must outlive a frame lives here. `None` until set.
pub fn state(ui: &Ui, id: u64) -> Option<i64> {
    ui.slot(id)
}

/// Set (or with `None` clear) the remembered integer state of widget `id`.
/// Pick ids that cannot collide with another widget's, e.g. `own_id ^ (MY_TAG << 20)`.
pub fn set_state(ui: &Ui, id: u64, value: Option<i64>) {
    ui.set_slot(id, value);
}

/// True when the pointer is over `bounds`.
pub fn hovered(ui: &Ui, bounds: &Rectangle) -> bool {
    ui.hovered(bounds)
}

/// Record that a press started on the widget occupying `bounds`, so its release (possibly after
/// a rebuild) can be recognised with [`was_pressed`].
pub fn press(ui: &Ui, bounds: &Rectangle) {
    ui.set_pressed(Some(util::key_of(bounds)));
}

/// True when the press in progress started on the widget occupying `bounds`.
pub fn was_pressed(ui: &Ui, bounds: &Rectangle) -> bool {
    ui.is_pressed(bounds)
}

/// Forget the press in progress.
pub fn release(ui: &Ui) {
    ui.set_pressed(None);
}

/// True when `r` lies wholly outside the scroll viewport being drawn, so a widget inside a long
/// scrollable can skip drawing itself.
pub fn culled(ui: &Ui, r: &Rectangle) -> bool {
    ui.culled(r)
}

/// Linear blend of two colours, `t` in `0..=1` (0 is `a`).
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    util::mix(a, b, t)
}

/// `top` composited over opaque `base`.
pub fn over(base: Color, top: Color) -> Color {
    util::over(base, top)
}

/// Draw `children` (given their `layout`) into `out`, for container widgets.
pub fn draw_children<M>(children: &[Element<M>], layout: &Layout, out: &mut DrawList) {
    util::draw_children(children, layout, out);
}

/// Deliver `event` to `children` in reverse paint order (topmost first) and stop at the first
/// that consumes it; the usual `on_event` body of a container widget.
pub fn dispatch<M>(
    children: &mut [Element<M>],
    event: &Event,
    layout: &Layout,
    cursor: Point,
    out: &mut Vec<M>,
) -> bool {
    util::dispatch(children, event, layout, cursor, out)
}

/// Describe the widget for screen readers while drawing it: its role, name and, in `f`, any of
/// value, checked state, focus id and enabled flag. Call once per `draw`.
pub fn describe(
    ui: &Ui,
    role: Role,
    label: &str,
    bounds: &Rectangle,
    f: impl FnOnce(&mut A11yNode),
) {
    ui.describe(role, label, bounds, f);
}
