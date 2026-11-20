use alloy_ui::{fs, prelude::*};
use std::path::PathBuf;

#[derive(Default)]
struct Files {
    dir: PathBuf,
    hidden: bool,
    query: String,
    picked: Option<fs::Entry>,
}

impl Files {
    fn go(&mut self, dir: Option<PathBuf>) {
        self.dir = dir.unwrap_or_else(|| self.dir.clone());
        self.picked = None;
        self.query.clear();
    }

    fn open(&mut self, e: &fs::Entry) {
        match e.dir {
            true => self.go(Some(e.path.clone())),
            false => self.picked = Some(e.clone()),
        }
    }

    fn up(&mut self) {
        self.go(self.dir.parent().map(PathBuf::from));
    }
}

fn view(s: &Files) -> impl View<Act<Files>> {
    let q = s.query.to_lowercase();
    let list = attempt(fs::list(&s.dir), "Cannot read folder", |files| {
        let detail = s.picked.as_ref().map(|e| {
            col![
                heading(&e.name),
                rule(),
                format!("Size: {}", e.size_text()),
                caption(e.path.display().to_string()).dim()
            ]
            .card()
            .w(280.0)
        });
        let list = table(&files)
            .keep(|e| (s.hidden || !e.hidden()) && e.name.to_lowercase().contains(&q))
            .col("Name", fs::Entry::label)
            .num("Size", fs::Entry::size_text)
            .key(|e| e.size as f64)
            .selected_where(|e| Some(e) == s.picked.as_ref())
            .on_row(Files::open)
            .count("items")
            .empty("Nothing here");
        row![list].push(detail).gap(16.0).fill()
    });
    col![
        row![
            button("Up").ghost().act(Files::up),
            text(s.dir.display().to_string()).fill_w(),
            check("Show hidden", at!(s.hidden)),
            input(at!(s.query)).hint("Filter").w(200.0),
        ]
        .align_center(),
        list,
    ]
    .page()
    .fill()
}

fn main() {
    let files = Files {
        dir: fs::cwd(),
        ..Default::default()
    };
    sketch(files, view).run_sized("Files", 900.0, 600.0)
}
