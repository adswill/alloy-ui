//! Notes: searchable list, editor per note, pin, sort, delete with confirmation, autosave, settings.
use alloy_ui::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

choices! { enum Sort { Recent, Alphabetical } }

#[derive(Record, Clone, Default)]
struct Note {
    body: String,
    pinned: bool,
    edited: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Note {
    fn new(body: &str) -> Self {
        Note {
            body: body.into(),
            edited: now(),
            ..Default::default()
        }
    }
    /// The body, for editing: touching it stamps the edit time.
    fn edit(&mut self) -> &mut String {
        self.edited = now();
        &mut self.body
    }
}

fn main() {
    app("Notes")
        .page("Notes", || {
            let notes = store(|| {
                vec![
                    Note::new("Welcome\nEverything you type is saved as you go."),
                    Note::new("Shopping\nCoffee, oat milk, lemons"),
                    Note::new("Ideas\nA native notes app with no webview"),
                ]
            });
            let sort = store_as("sort", Sort::default);
            browse(&notes)
                .doc(|n| &n.body)
                .pin(|n| n.pinned)
                .order(move |a, b| match sort.get() {
                    Sort::Recent => b.edited.cmp(&a.edited),
                    Sort::Alphabetical => a.body.to_lowercase().cmp(&b.body.to_lowercase()),
                })
                .new(|| Note::new(""))
                .empty("Select a note or create a new one")
                .detail(|n| {
                    let body = n.field(Note::edit);
                    col![
                        body.editor(),
                        row![
                            caption(format!("{} words", body.read().split_whitespace().count()))
                                .fill_w(),
                            n.field(|n| &mut n.pinned).check("Pinned"),
                            button("Delete")
                                .ghost()
                                .act(move |_| n.remove())
                                .confirm("Delete this note?"),
                        ]
                        .align_center()
                        .gap(12.0),
                    ]
                    .gap(10.0)
                    .pad(16.0)
                    .fill()
                })
        })
        .settings(|| section("Sort notes by", store_as("sort", Sort::default).options()))
        .persist()
        .run()
}
