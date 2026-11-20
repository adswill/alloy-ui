//! Todo list: input line, scrolling checklist, filter.
use alloy_ui::prelude::*;

alloy_ui::choices! { enum Filter { All, Open, Done } }

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (440.0, 470.0);

struct Todo {
    tasks: Vec<(String, bool)>,
    draft: String,
    filter: Filter,
}

impl Todo {
    fn add(&mut self) {
        let title = self.draft.trim().to_string();
        if !title.is_empty() {
            self.tasks.push((title, false));
            self.draft.clear();
        }
    }
}

fn view(s: &Todo) -> impl View<Act<Todo>> {
    let open = s.tasks.iter().filter(|t| !t.1).count();
    let shown = s.tasks.iter().enumerate().filter(|(_, t)| match s.filter {
        Filter::All => true,
        Filter::Open => !t.1,
        Filter::Done => t.1,
    });
    col![
        col![heading("Tasks"), caption(format!("{open} open")).dim()].gap(2.0),
        row![
            input(&s.draft)
                .hint("What needs doing?")
                .bind(|s: &mut Todo| &mut s.draft)
                .submit(Todo::add)
                .fill_w(),
            button("Add task").primary().act(Todo::add),
        ]
        .align_center()
        .fill_w(),
        scroll(
            col(shown
                .map(|(i, t)| { check(&t.0, t.1).act(move |s: &mut Todo, v| s.tasks[i].1 = v) }))
            .gap(10.0)
        ),
        row![
            options(&s.filter).bind(|s: &mut Todo| &mut s.filter),
            space(),
            button("Clear done")
                .ghost()
                .act(|s: &mut Todo| s.tasks.retain(|t| !t.1)),
        ]
        .align_center()
        .fill_w(),
    ]
    .gap(20.0)
    .pad(24.0)
    .fill()
}

/// The app.
pub fn app() -> impl App {
    let task = |s: &str, done| (s.to_string(), done);
    sketch(
        Todo {
            tasks: vec![
                task("Order replacement fan", true),
                task("Write release notes for 0.4", false),
                task("Book dentist", false),
                task("Renew domain before the 28th", false),
                task("Reply to Marta about the invoice", true),
            ],
            draft: String::new(),
            filter: Filter::All,
        },
        view,
    )
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Todo", SIZE.0, SIZE.1)
}
