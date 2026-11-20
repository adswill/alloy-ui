//! Notes: searchable list, editor per note, pin, sort, delete with confirmation, autosave, theme.
use alloy_ui::prelude::*;

#[derive(Record, Clone, Default)]
struct Note {
    #[form(long)]
    body: String,
    pinned: bool,
    #[form(skip)]
    edited: Stamp,
}

fn main() {
    app("Notes")
        .page("Notes", || {
            let notes = store(|| {
                Note::rows([
                    vals!["Welcome\nEverything you type is saved as you go.", false],
                    vals!["Shopping\nCoffee, oat milk, lemons", false],
                    vals!["Ideas\nA native notes app with no webview", false],
                ])
            });
            browse_of(&notes)
                .foot(|n| caption(format!("{} words", n.body.split_whitespace().count())))
        })
        .shortcut_as("mod+s", "Save", || toast("Saved"))
        .undoable()
        .theme_page()
        .persist()
        .run()
}
