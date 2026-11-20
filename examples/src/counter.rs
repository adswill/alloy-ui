//! Counter: the smallest useful app. No message enum, no `update`.
use alloy_ui::prelude::*;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (360.0, 200.0);

/// The app.
pub fn app() -> impl App {
    sketch(0i64, |n| {
        col![
            col![heading("Counter"), text(format!("{n} so far")).dim()].gap(2.0),
            row![
                button("Subtract").act(|n| *n -= 1),
                button("Add").primary().act(|n| *n += 1),
                button("Reset").ghost().act(|n| *n = 0),
            ]
            .align_center(),
        ]
        .gap(18.0)
        .pad(24.0)
        .fill()
    })
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Counter", SIZE.0, SIZE.1)
}
