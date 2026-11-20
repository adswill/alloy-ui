//! The todo example. `--headless [dir]` writes a PNG instead of opening a window.
use alloy_ui_examples::apps::todo::{demo, Todo};

fn main() {
    alloy_ui_examples::run::main_app("Todo", (420.0, 340.0), Todo::new, demo);
}
