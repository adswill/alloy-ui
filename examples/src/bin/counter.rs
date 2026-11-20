//! The counter example. `--headless [dir]` writes a PNG instead of opening a window.
use alloy_ui_examples::apps::counter::{demo, Counter};

fn main() {
    alloy_ui_examples::run::main_app("Counter", (360.0, 180.0), Counter::new, demo);
}
