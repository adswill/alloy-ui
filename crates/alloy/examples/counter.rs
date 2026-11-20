//! Counter, closure style: no message enum.
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![
            heading("Counter"),
            text(format!("{n} so far")).dim(),
            row![
                button("Subtract").act(|n| *n -= 1),
                button("Add").primary().act(|n| *n += 1),
                button("Reset").ghost().act(|n| *n = 0),
            ]
            .align_center(),
        ]
        .gap(14.0)
        .pad(24.0)
        .fill()
    })
    .run_sized("Counter", 360.0, 180.0)
}
