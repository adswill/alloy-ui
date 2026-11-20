use alloy_ui::prelude::*;
use std::path::PathBuf;

#[derive(Clone)]
struct Entry {
    name: String,
    dir: bool,
    size: u64,
}

struct Files {
    dir: PathBuf,
    entries: Result<Vec<Entry>, String>,
    hidden: bool,
    query: String,
    selected: Option<String>,
}

fn load(dir: &PathBuf) -> Result<Vec<Entry>, String> {
    let mut entries: Vec<Entry> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some(Entry {
                name: e.file_name().to_string_lossy().into_owned(),
                dir: meta.is_dir(),
                size: meta.len(),
            })
        })
        .collect();
    entries.sort_by(|a, b| {
        b.dir
            .cmp(&a.dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

fn human(n: u64) -> String {
    match n {
        0..1024 => format!("{n} B"),
        1024..1_048_576 => format!("{:.1} KB", n as f64 / 1024.0),
        _ => format!("{:.1} MB", n as f64 / 1_048_576.0),
    }
}

impl Files {
    fn go(&mut self, dir: PathBuf) {
        self.entries = load(&dir);
        self.dir = dir;
        self.selected = None;
        self.query.clear();
    }
}

fn view(s: &Files) -> impl View<Act<Files>> {
    let body: Node<Act<Files>> = match &s.entries {
        Err(e) => text(format!("Cannot read folder: {e}")).danger().into(),
        Ok(entries) => {
            let q = s.query.to_lowercase();
            let rows: Vec<Entry> = entries
                .iter()
                .filter(|e| {
                    (s.hidden || !e.name.starts_with('.')) && e.name.to_lowercase().contains(&q)
                })
                .cloned()
                .collect();
            if rows.is_empty() {
                text("Nothing here").into()
            } else {
                let picked = rows
                    .iter()
                    .position(|e| Some(&e.name) == s.selected.as_ref());
                let by_index = rows.clone();
                let count = rows.len();
                let list = table(&rows)
                    .col("Name", |e| {
                        if e.dir {
                            format!("{}/", e.name)
                        } else {
                            e.name.clone()
                        }
                    })
                    .num(
                        "Size",
                        |e| if e.dir { String::new() } else { human(e.size) },
                    )
                    .key(|e| e.size as f64)
                    .selected(picked)
                    .on_select(move |s: &mut Files, i| {
                        let e = &by_index[i];
                        if e.dir {
                            s.go(s.dir.join(&e.name));
                        } else {
                            s.selected = Some(e.name.clone());
                        }
                    });
                let detail = picked.map(|i| {
                    col![
                        heading(&rows[i].name),
                        rule(),
                        format!("Size: {}", human(rows[i].size)),
                        caption(s.dir.join(&rows[i].name).display().to_string()).dim(),
                    ]
                    .gap(8.0)
                    .card()
                    .pad(16.0)
                    .w(280.0)
                });
                col![
                    row![list].push(detail).gap(16.0).fill(),
                    caption(format!("{count} items"))
                ]
                .gap(8.0)
                .fill()
                .into()
            }
        }
    };
    col![
        row![
            button("Up").ghost().act(|s: &mut Files| {
                if let Some(p) = s.dir.parent() {
                    s.go(p.to_path_buf());
                }
            }),
            text(s.dir.display().to_string()).fill_w(),
            check("Show hidden", s.hidden).bind(|s: &mut Files| &mut s.hidden),
            input(&s.query)
                .hint("Filter")
                .bind(|s: &mut Files| &mut s.query)
                .w(200.0),
        ]
        .align_center()
        .gap(12.0),
        body,
    ]
    .gap(14.0)
    .pad(20.0)
    .fill()
}

fn main() {
    let dir = std::env::var_os("FILES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let s = Files {
        entries: load(&dir),
        dir,
        hidden: false,
        query: String::new(),
        selected: None,
    };
    sketch(s, view).run_sized("Files", 900.0, 600.0)
}
