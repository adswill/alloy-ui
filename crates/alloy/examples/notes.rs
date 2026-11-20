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
    fn title(&self) -> &str {
        self.body
            .lines()
            .next()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .unwrap_or("Untitled")
    }
    fn preview(&self) -> String {
        let more = self.body.lines().skip(1).find(|l| !l.trim().is_empty());
        more.unwrap_or("No more text")
            .trim()
            .chars()
            .take(48)
            .collect()
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
            let (cur, sort) = (store(|| 0), store_as("sort", Sort::default));
            let (query, confirm) = (local(String::new()), local(false));

            let all = notes.get();
            let q = query.get().to_lowercase();
            let mut shown: Vec<usize> = (0..all.len())
                .filter(|&i| all[i].body.to_lowercase().contains(&q))
                .collect();
            shown.sort_by(|&a, &b| {
                all[b]
                    .pinned
                    .cmp(&all[a].pinned)
                    .then_with(|| match sort.get() {
                        Sort::Recent => all[b].edited.cmp(&all[a].edited),
                        Sort::Alphabetical => all[a]
                            .title()
                            .to_lowercase()
                            .cmp(&all[b].title().to_lowercase()),
                    })
            });
            let at = shown.iter().position(|&i| i == cur.get());
            let rows = shown.iter().map(|&i| {
                let star = if all[i].pinned { "★ " } else { "" };
                col![
                    text(format!("{star}{}", all[i].title())),
                    caption(all[i].preview()).dim()
                ]
                .gap(2.0)
            });
            let side = col![
                row![
                    input(query.get()).hint("Search").local(&query).fill_w(),
                    button("New").act(move |_: &mut ()| {
                        notes.with(|v| v.push(Note::new("")));
                        cur.set(notes.with(|v| v.len() - 1));
                    })
                ]
                .gap(8.0),
                list(rows, at).act(move |_: &mut (), p| {
                    cur.set(shown[p]);
                    confirm.set(false);
                }),
            ]
            .gap(12.0)
            .pad(12.0)
            .w(300.0)
            .fill_h();

            let i = cur.get();
            let Some(note) = all.get(i) else {
                return row![side, text("Select a note or create a new one")];
            };
            let delete: Node<_> = if confirm.get() {
                row![
                    caption("Delete this note?"),
                    button("Delete").primary().act(move |_: &mut ()| {
                        notes.with(|v| v.remove(i));
                        cur.set(0);
                        confirm.set(false);
                    }),
                    button("Cancel")
                        .ghost()
                        .act(move |_: &mut ()| confirm.set(false)),
                ]
                .align_center()
                .gap(8.0)
                .into()
            } else {
                button("Delete")
                    .ghost()
                    .act(move |_: &mut ()| confirm.set(true))
                    .into()
            };
            let pane = col![
                editor(&note.body)
                    .id(i as u64 + 1)
                    .act(move |_: &mut (), v| notes.with(|n| {
                        n[i].body = v;
                        n[i].edited = now();
                    })),
                row![
                    caption(format!(
                        "{} words · {} characters",
                        note.body.split_whitespace().count(),
                        note.body.chars().count()
                    ))
                    .fill_w(),
                    check("Pinned", note.pinned)
                        .act(move |_: &mut (), p| notes.with(|n| n[i].pinned = p)),
                    delete,
                ]
                .align_center()
                .gap(12.0),
            ]
            .gap(10.0)
            .pad(16.0)
            .fill();
            row![side, pane].fill()
        })
        .page("Settings", || {
            col![
                heading("Settings"),
                section(
                    "Sort notes by",
                    options(&store_as("sort", Sort::default).get())
                ),
                section("Theme", theme_switch())
            ]
        })
        .persist()
        .run()
}
