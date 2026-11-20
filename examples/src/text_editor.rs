//! Text editor: the real multi-line `TextEditor` with a status line.
use alloy_ui::prelude::*;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (640.0, 420.0);

const NOTES: &str = "Release notes, 0.4\n\n\
- Text input handles IME composition and selection.\n\
- The editor scrolls to keep the caret in view.\n\
- Idle apps use no CPU: nothing redraws until something changes.\n\n\
Try typing here, select with the mouse or shift and the arrow keys.\n";

/// The app.
pub fn app() -> impl App {
    sketch(NOTES.to_string(), |t: &String| {
        let words = t.split_whitespace().count();
        col![
            row![
                heading("notes.txt").fill_w(),
                button("Clear").ghost().act(|t: &mut String| t.clear()),
            ]
            .align_center()
            .fill_w(),
            editor(t).hint("Start typing").bind(|t: &mut String| t),
            row![
                caption(format!("{} lines", t.lines().count()))
                    .dim()
                    .fill_w(),
                caption(format!("{words} words, {} characters", t.chars().count())).dim(),
            ]
            .fill_w(),
        ]
        .gap(12.0)
        .pad(20.0)
        .fill()
    })
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Text editor", SIZE.0, SIZE.1)
}
