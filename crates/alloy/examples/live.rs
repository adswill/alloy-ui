//! A live view: `cargo run --example live` (or `alloy dev`), then edit `ui/counter.alloy`.
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        live!("ui/counter.alloy")
            .var("name", "Alloy")
            .var("n", n)
            .on("add", |n: &mut i64| *n += 1)
            .on("sub", |n| *n -= 1)
            .on("reset", |n| *n = 0)
    })
    .run_sized("Live", 420.0, 200.0)
}
