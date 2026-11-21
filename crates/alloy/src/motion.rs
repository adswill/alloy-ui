//! Animation in views, one call each. All of them key themselves from the
//! call site (like every other helper here), honour reduced motion, and cost
//! nothing once settled.
//!
//! ```ignore
//! reveal(s.show_help, text("Press ⌘K for the palette").dim())   // fade + slide + collapse
//! let w = animated(if s.wide { 320.0 } else { 160.0 });         // eased number
//! let k = spring(if hot { 1.1 } else { 1.0 });                  // no-overshoot spring
//! ```

use crate::{cx::auto_id, ui, view::Node};
use alloy_ui_widgets::{self as w, Animatable, Spring, Transition};

/// Show `child` while `show` is true: it fades and drifts up 6 px on enter,
/// the reverse on exit, and the space it occupies grows and shrinks with it.
/// Hidden content is not drawn and gets no input.
#[track_caller]
pub fn reveal<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(60);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .slide(6.0)
        .collapse(true)
        .into_element()
        .into()
}

/// Like [`reveal`] but only the opacity changes; layout never moves.
#[track_caller]
pub fn fade<M: 'static>(show: bool, child: impl Into<Node<M>>) -> Node<M> {
    let key = auto_id(61);
    w::Reveal::new(ui(), key, show, child.into().into_element())
        .into_element()
        .into()
}

/// `target` eased over the theme's normal duration: call it every view with
/// the value you want and use what it returns. Works for `f32`, `Point` and
/// `Color`.
#[track_caller]
pub fn animated<T: Animatable>(target: T) -> T {
    let key = auto_id(62);
    let ms = ui().theme().motion.normal_ms;
    ui().animate(key, target, Transition::ms(ms))
}

/// `target` on a critically damped spring (no overshoot). Interrupting it
/// mid-flight keeps its velocity, so it never snaps.
#[track_caller]
pub fn spring<T: Animatable>(target: T) -> T {
    let key = auto_id(63);
    ui().animate(key, target, Transition::Spring(Spring::SNAPPY))
}
