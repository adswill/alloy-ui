//! Status colours: a small dot beside a label whose meaning is obvious from its name.

use crate::{cx, view::Node};
use alloy_ui_core::Color;

/// Success / warning / danger for common status words (`Up`, `Degraded`, `Failed`, ...), or
/// `None` for words with no obvious tone, which stay neutral.
pub(crate) fn tone(label: &str) -> Option<Color> {
    let p = cx::palette();
    let l = label.to_lowercase();
    let is = |ws: &[&str]| ws.iter().any(|w| l == *w);
    if is(&[
        "up", "ok", "online", "active", "done", "healthy", "passed", "success",
    ]) {
        Some(p.success)
    } else if is(&[
        "degraded",
        "warning",
        "slow",
        "pending",
        "paused",
        "review",
        "idle",
        "in progress",
        "no",
    ]) {
        Some(warning())
    } else if is(&["down", "failed", "error", "offline", "critical", "blocked"]) {
        Some(p.danger)
    } else {
        None
    }
}

/// Amber that reads on both light and dark surfaces (the palette has no warning role).
pub(crate) fn warning() -> Color {
    if cx::palette().background.r < 0.5 {
        Color::hex(0xE3A83D)
    } else {
        Color::hex(0xA85F00)
    }
}

/// A filled circle of `d` logical pixels.
pub(crate) fn dot<M: 'static>(c: Color, d: f32) -> Node<M> {
    use crate::view::View;
    crate::view::col(Vec::<Node<M>>::new())
        .w(d)
        .h(d)
        .bg(c)
        .round(d / 2.0)
        .into()
}

/// A status label: its dot, then the text. Words with no tone stay plain text.
pub(crate) fn status<M: 'static>(label: &str) -> Node<M> {
    let Some(c) = tone(label) else {
        return crate::text(label.to_string()).into();
    };
    crate::view::row([dot::<M>(c, 8.0), crate::text(label.to_string()).into()])
        .gap(8.0)
        .align_center()
        .into()
}
