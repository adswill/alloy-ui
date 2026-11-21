//! Alloy's mobile starter: a touch-first counter.
//!
//! The same code runs on the desktop (`cargo run -p alloy-ui-mobile-counter`),
//! on Android (`alloy bundle --target android`) and on iOS
//! (`alloy bundle --target ios`). Only [`mobile_entry!`](alloy_ui::mobile_entry)
//! differs per platform.
use alloy_ui::prelude::*;

/// The app: one big number, big buttons, content kept clear of the notch and
/// system bars with [`pad_safe`](alloy_ui::View::pad_safe). Pan and tap work
/// without any touch code, because the shell turns a finger into pointer input.
pub fn app() -> impl App {
    sketch(0i64, |n| {
        col![
            col![
                caption("Alloy on mobile"),
                heading("Counter"),
                text(n.to_string()).display(),
                text("Tap the buttons.").dim(),
            ]
            .gap(6.0),
            col![
                button("Add one").primary().fill_w().act(|n| *n += 1),
                button("Subtract one").fill_w().act(|n| *n -= 1),
                button("Reset").ghost().fill_w().act(|n| *n = 0),
            ]
            .gap(12.0),
        ]
        .gap(32.0)
        .pad_safe(24.0)
        .fill()
    })
}

/// Open the window and run until the app exits.
pub fn run() {
    app().run_sized("Alloy Counter", 390.0, 780.0);
}

alloy_ui::mobile_entry!(run);
