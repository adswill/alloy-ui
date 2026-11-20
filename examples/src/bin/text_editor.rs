//! The text_editor example. `--headless [dir]` writes a PNG instead of opening a window.
use alloy_ui_examples::apps::text_editor::{demo, TextEditor};

fn main() {
    alloy_ui_examples::run::main_app("Text editor", (520.0, 260.0), TextEditor::new, demo);
}
