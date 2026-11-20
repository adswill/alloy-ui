//! Text editor: multi-line buffer with a status line.
use alloy_ui::prelude::*;

fn main() {
    let text = "Alloy draws every pixel itself.\nNo webview, no HTML.\n".to_string();
    sketch(text, |t: &String| {
        col![
            editor(t).bind(|t: &mut String| t),
            row![
                caption(format!("{} lines", t.lines().count()))
                    .dim()
                    .fill_w(),
                caption(format!("{} words", t.split_whitespace().count())).dim(),
            ],
        ]
        .gap(8.0)
        .pad(16.0)
        .fill()
    })
    .run_sized("Text editor", 520.0, 260.0)
}
