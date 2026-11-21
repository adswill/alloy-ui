//! Ready-made modules: whole interface pieces that look finished with one call.
//!
//! ```ignore
//! kpi("Revenue", "$48.2k").delta(12.4).trend(series(24))
//! gauge(0.68).label("CPU").bands()
//! timeline([moment("2 min ago", "Deploy finished").success()])
//! empty_state(Art::Inbox, "You are all caught up", "New mail shows up here.")
//! ```
//!
//! Static modules are generic over the message type, so they drop into any view. Interactive ones
//! (`notification_center`, `login`, `chat`, `plan_picker`, `file_browser`, `tour`, `command_bar`)
//! take closures over your state, like every other helper in the facade.

#![allow(clippy::type_complexity)]

use crate::{cx, dsl::Act, view::Node};
use alloy_ui_core::{Color, Point};
use alloy_ui_widgets as w;

mod chat;
mod empty;
mod feed;
mod files;
mod gauge;
mod kpi;
mod nav;
mod people;
mod plans;
mod scaffold;
mod search;
mod tour;

pub use chat::*;
pub use empty::*;
pub use feed::*;
pub use files::*;
pub use gauge::*;
pub use kpi::*;
pub use nav::*;
pub use people::*;
pub use plans::*;
pub use scaffold::*;
pub use search::*;
pub use tour::*;

/// `a` toward `b` by `t` (0 is `a`).
pub(crate) fn mix(a: Color, b: Color, t: f32) -> Color {
    Color {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a + (b.a - a.a) * t,
    }
}

pub(crate) fn tone_color(t: w::Tone) -> Color {
    t.color(&cx::theme())
}

/// The neutral "no tone" colour used for dots and rails.
pub(crate) fn quiet() -> Color {
    cx::palette().border_strong
}

pub(crate) fn hash(s: &str) -> u32 {
    s.bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619))
}

pub(crate) fn text_width(s: &str, size: f32, weight: u16) -> f32 {
    cx::ui()
        .metrics()
        .width_styled(s, size, weight, alloy_ui_core::FontFamily::Sans)
}

/// Make `child` clickable: runs `f` on the state.
pub(crate) fn tapped<S: 'static>(
    child: impl Into<Node<Act<S>>>,
    f: impl Fn(&mut S) + Send + Sync + 'static,
) -> w::Tap<Act<S>> {
    w::Tap::new(&cx::ui(), child.into().into_element()).on_press(Act::new(f))
}

/// Explicit conversion, for places where `.into()` is ambiguous.
pub(crate) fn nn<M: 'static>(v: impl Into<Node<M>>) -> Node<M> {
    v.into()
}

/// A spacer that only grows sideways (the facade's `hfill()` also grows vertically, which
/// would stretch the row it is in).
pub(crate) fn hfill<M: 'static>() -> Node<M> {
    Node::from(w::Space::fill_w().into_element())
}

pub(crate) fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

/// A canvas element of fixed size, generic over the message type.
pub(crate) fn paint<M: 'static>(
    width: alloy_ui_layout::Length,
    height: f32,
    f: impl Fn(&mut w::Frame) + 'static,
) -> Node<M> {
    let id = cx::auto_id(70);
    Node::from(
        w::Canvas::<M>::new(
            &cx::ui(),
            id,
            width,
            alloy_ui_layout::Length::Fixed(height),
            f,
        )
        .into_element(),
    )
}
