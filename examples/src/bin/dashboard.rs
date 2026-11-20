//! The dashboard example. `--headless [dir]` writes a PNG instead of opening a window.
use alloy_ui_examples::apps::dashboard::{demo, Dashboard};

fn main() {
    alloy_ui_examples::run::main_app("Dashboard", (520.0, 460.0), Dashboard::new, demo);
}
