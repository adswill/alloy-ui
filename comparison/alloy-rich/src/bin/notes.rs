use alloy_ui::prelude::*;
use rich::*;

choices! { enum Tag { Work, Personal, Ideas } }

#[derive(Clone, PartialEq)]
struct Note {
    title: String,
    body: String,
    pinned: bool,
    tag: Tag,
    edited: u32,
}

#[derive(Clone, PartialEq)]
struct Notes {
    notes: Vec<Note>,
    cur: usize,
    query: String,
    only: Option<Tag>,
    dark: bool,
    sort: usize,
}

fn note(title: &str, body: &str, pinned: bool, tag: Tag, edited: u32) -> Note {
    Note { title: title.into(), body: body.into(), pinned, tag, edited }
}

fn ago(minutes: u32) -> String {
    match minutes {
        0..=1 => "just now".into(),
        2..=59 => format!("{minutes} min ago"),
        60..=1439 => format!("{} h ago", minutes / 60),
        _ => format!("{} d ago", minutes / 1440),
    }
}

impl Notes {
    /// Indices of the notes that match the search and tag filter, pinned first.
    fn shown(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        let mut ix: Vec<usize> = (0..self.notes.len())
            .filter(|&i| {
                let n = &self.notes[i];
                self.only.map_or(true, |t| t == n.tag)
                    && (n.title.to_lowercase().contains(&q) || n.body.to_lowercase().contains(&q))
            })
            .collect();
        ix.sort_by(|&a, &b| {
            let (x, y) = (&self.notes[a], &self.notes[b]);
            y.pinned.cmp(&x.pinned).then(match self.sort {
                1 => x.title.to_lowercase().cmp(&y.title.to_lowercase()),
                2 => y.body.len().cmp(&x.body.len()),
                _ => x.edited.cmp(&y.edited),
            })
        });
        ix
    }
    fn add(&mut self) {
        self.notes.push(note("Untitled", "", false, self.only.unwrap_or(Tag::Work), 0));
        self.query.clear();
        self.cur = self.notes.len() - 1;
    }
    fn remove(&mut self) {
        if self.cur < self.notes.len() {
            self.notes.remove(self.cur);
            self.cur = self.cur.saturating_sub(1);
            toast("Note deleted").undo();
        }
    }
    fn duplicate(&mut self) {
        if let Some(n) = self.notes.get(self.cur).cloned() {
            self.notes.push(Note { title: format!("{} copy", n.title), pinned: false, edited: 0, ..n });
            self.cur = self.notes.len() - 1;
            toast("Duplicated");
        }
    }
    fn pin(&mut self) {
        if let Some(n) = self.notes.get_mut(self.cur) {
            n.pinned ^= true;
            toast(if n.pinned { "Pinned to the top" } else { "Unpinned" });
        }
    }
}

fn sidebar(s: &Notes) -> Node<Act<Notes>> {
    let shown = s.shown();
    let at = shown.iter().position(|&i| i == s.cur);
    let rows = shown.iter().map(|&i| {
        let n = &s.notes[i];
        let first = n.body.lines().next().unwrap_or("No content yet").to_string();
        col![
            row![text(&n.title).label().no_wrap(), space(), caption(if n.pinned { "Pinned" } else { "" }).accent()],
            caption(first).dim().no_wrap(),
            row![chip(name(n.tag), false), caption(ago(n.edited)).dim()].gap(8.0).align_center(),
        ]
        .gap(3.0)
    });
    let filters = choose(
        std::iter::once(("All", None)).chain(Tag::all().iter().map(|&(l, t)| (l, Some(t)))),
        s.only,
    )
    .act(|s: &mut Notes, t| s.only = t);
    let body: Node<_> = if shown.is_empty() {
        empty("No matching notes", "Try another word, or clear the filter.", button("Clear filters").act(|s: &mut Notes| {
            s.query.clear();
            s.only = None;
        }))
    } else {
        list(rows, at).act(move |s: &mut Notes, k| s.cur = s.shown()[k]).fill_h().into()
    };
    let sort = pick(["Recent", "A to Z", "Longest"], Some(s.sort)).act(|s: &mut Notes, i| s.sort = i);
    col![
        row![heading("Notes"), caption(format!("{} of {}", shown.len(), s.notes.len())).dim(), space(), button("New note").primary().act(Notes::add)]
            .gap(8.0)
            .align_center(),
        row![input(&s.query).hint("Search notes").bind(|s: &mut Notes| &mut s.query).fill_w(), sort].gap(8.0),
        filters,
        body,
        row![toggle("Dark", at!(s.dark)), space(), kbd("mod+n"), caption("new").dim(), kbd("mod+k"), caption("commands").dim()]
            .gap(6.0)
            .align_center(),
    ]
    .gap(10.0)
    .pad(14.0)
    .surface()
    .w(320.0)
    .fill_h()
    .into()
}

fn editor_pane(s: &Notes) -> Node<Act<Notes>> {
    let i = s.cur;
    let Some(n) = s.notes.get(i) else {
        return empty("Nothing selected", "Pick a note on the left, or start a new one.", button("New note").primary().act(Notes::add)).fill().into();
    };
    let words = n.body.split_whitespace().count();
    col![
        row![
            input(&n.title).bind(move |s: &mut Notes| &mut s.notes[i].title).fill_w(),
            options(n.tag).bind(move |s: &mut Notes| &mut s.notes[i].tag),
        ]
        .gap(10.0)
        .align_center(),
        editor(&n.body).hint("Start typing...").bind(move |s: &mut Notes| &mut s.notes[i].body).fill(),
        row![
            caption(format!("{words} words")).dim(),
            caption(format!("{} min read", (words / 200).max(1))).dim(),
            caption(format!("Edited {}", ago(n.edited))).dim(),
            space(),
            progress(0.0..=100.0, (words as f32).min(100.0)).w(70.0).tip("Progress towards a 100 word note"),
            button("Duplicate").act(Notes::duplicate),
            button(if n.pinned { "Unpin" } else { "Pin" }).act(Notes::pin),
            button("Delete").act(Notes::remove).confirm("Delete this note?"),
        ]
        .gap(12.0)
        .align_center(),
    ]
    .gap(12.0)
    .pad(24.0)
    .fill()
    .into()
}

fn view(s: &Notes) -> impl View<Act<Notes>> {
    set_accent(Color::hex(INDIGO));
    set_dark(s.dark);
    row![sidebar(s), editor_pane(s)].gap(0.0).fill()
}

fn main() {
    let notes = vec![
        note("Roadmap Q3", "Ship the sync engine beta.\nPolish onboarding and cut the first-run time in half.\nDecide on pricing before the August review.", true, Tag::Work, 12),
        note("Meeting notes: design sync", "Agreed on an eight pixel radius everywhere.\nOne accent colour, used sparingly.\nHairlines instead of shadows.", true, Tag::Work, 95),
        note("Grocery list", "Oat milk, lemons, sourdough, olive oil", false, Tag::Personal, 1500),
        note("Book recommendations", "The Design of Everyday Things\nA Philosophy of Software Design\nThe Pragmatic Programmer", false, Tag::Ideas, 4300),
        note("Trip ideas", "Lisbon in October, train to Porto.\nBook the window seats early.", false, Tag::Personal, 10100),
    ];
    sketch(Notes { notes, cur: 0, query: String::new(), only: None, dark: false, sort: 0 }, view)
        .undoable()
        .shortcut_as("mod+n", "New note", Notes::add)
        .shortcut_as("mod+shift+d", "Duplicate note", Notes::duplicate)
        .shortcut_as("mod+p", "Pin or unpin", Notes::pin)
        .shortcut_as("mod+d", "Toggle dark", |s| s.dark ^= true)
        .run_sized("Notes", 960.0, 640.0)
}
