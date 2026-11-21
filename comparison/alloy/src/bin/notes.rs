use alloy_ui::prelude::*;

struct Note {
    title: String,
    body: String,
    pinned: bool,
}

struct Notes {
    notes: Vec<Note>,
    cur: usize,
    query: String,
}

fn note(title: &str, body: &str, pinned: bool) -> Note {
    Note { title: title.into(), body: body.into(), pinned }
}

impl Notes {
    /// Indices of the notes that match the search, pinned first.
    fn shown(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        let mut ix: Vec<usize> = (0..self.notes.len())
            .filter(|&i| {
                let n = &self.notes[i];
                n.title.to_lowercase().contains(&q) || n.body.to_lowercase().contains(&q)
            })
            .collect();
        ix.sort_by_key(|&i| !self.notes[i].pinned);
        ix
    }
    fn add(&mut self) {
        self.notes.push(note("Untitled", "", false));
        self.query.clear();
        self.cur = self.notes.len() - 1;
    }
    fn remove(&mut self) {
        self.notes.remove(self.cur);
        self.cur = self.cur.saturating_sub(1);
    }
}

fn view(s: &Notes) -> impl View<Act<Notes>> {
    set_accent(Color::hex(0x4F6BED));
    let shown = s.shown();
    let at = shown.iter().position(|&i| i == s.cur);
    let rows = shown.iter().map(|&i| {
        let n = &s.notes[i];
        let first = n.body.lines().next().unwrap_or("").to_string();
        col![
            row![text(&n.title).label(), space(), caption(if n.pinned { "Pinned" } else { "" }).accent()],
            caption(first).dim().no_wrap(),
        ]
        .gap(2.0)
    });
    let sidebar = col![
        row![
            input(&s.query).hint("Search notes").bind(|s: &mut Notes| &mut s.query).fill_w(),
            button("New").primary().act(Notes::add),
        ]
        .gap(8.0),
        list(rows, at).act(move |s: &mut Notes, k| s.cur = s.shown()[k]).fill_h(),
    ]
    .gap(10.0)
    .pad(14.0)
    .surface()
    .w(300.0)
    .fill_h();
    let i = s.cur;
    let editor_pane = match s.notes.get(i) {
        Some(n) => col![
            row![
                input(&n.title).bind(move |s: &mut Notes| &mut s.notes[i].title).fill_w(),
                button(if n.pinned { "Unpin" } else { "Pin" }).act(move |s: &mut Notes| s.notes[i].pinned ^= true),
                button("Delete").act(Notes::remove).confirm("Delete this note?"),
            ]
            .gap(8.0)
            .align_center(),
            editor(&n.body).bind(move |s: &mut Notes| &mut s.notes[i].body).fill(),
            caption(format!("{} words", n.body.split_whitespace().count())).dim(),
        ]
        .gap(12.0),
        None => col![heading("No notes"), caption("Create one with New.").dim()].gap(4.0),
    };
    row![sidebar, editor_pane.pad(24.0).fill()].gap(0.0).fill()
}

fn main() {
    let notes = vec![
        note("Welcome", "Everything you type is saved as you go.\nPin the notes you reach for most.", true),
        note("Shopping", "Coffee, oat milk, lemons, rye bread", false),
        note("Ideas", "A native notes app with no webview.\nStartup under 100 ms.", true),
        note("Meeting, Tuesday", "Roadmap review.\nShip the importer before the holidays.", false),
        note("Reading list", "The Mythical Man-Month\nA Philosophy of Software Design", false),
    ];
    sketch(Notes { notes, cur: 0, query: String::new() }, view).run_sized("Notes", 960.0, 640.0)
}
