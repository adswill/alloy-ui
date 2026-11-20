//! The settings example. `--headless [dir]` writes a PNG instead of opening a window.
use alloy_ui_examples::apps::settings::{demo, Settings};

fn main() {
    alloy_ui_examples::run::main_app("Settings", (560.0, 300.0), Settings::new, demo);
}
