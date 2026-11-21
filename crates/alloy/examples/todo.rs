//! Todo list: input line, scrolling list of checkboxes, filter.
use alloy_ui::prelude::*;

alloy_ui::choices! { enum Filter { All, Open, Done } }

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
            input(at!(s.draft))
                .hint("What needs doing?")
                .submit(Todo::add)
                .fill_w(),
            button("Add task").primary().act(Todo::add),
        ]
        .align_center()
        .fill_w(),
        scroll(col(shown.map(|(i, t)| check(&t.0, at!(s.tasks[i].1))))),
        row![
            options(s.filter).bind(|s: &mut Todo| &mut s.filter),
            button("Clear done").ghost().keep(at!(s.tasks), |t| !t.1),
        ],
    ]
    .gap(18.0)
    .pad(24.0)
    .fill()
}

fn main() {
    let t = |s: &str, done| (s.to_string(), done);
    let tasks = vec![
        t("Order replacement fan", true),
        t("Write release notes", false),
        t("Book dentist", false),
    ];
    sketch(
        Todo {
            tasks,
            draft: String::new(),
            filter: Filter::All,
        },
        view,
    )
    .run_sized("Todo", 420.0, 460.0)
}
