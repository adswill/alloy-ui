//! Todo list: input line, list with checkboxes, filter.
use crate::kit::{button, checkbox, column, editor, label, row, spacer, Emphasis, Input};
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element, Key};
use alloy_ui_theme::Theme;

/// Which items are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Everything.
    All,
    /// Not yet done.
    Open,
    /// Completed.
    Done,
}

/// One task.
#[derive(Debug, Clone)]
pub struct Task {
    /// Text of the task.
    pub title: String,
    /// Completed?
    pub done: bool,
}

/// Todo state.
pub struct Todo {
    /// All tasks.
    pub tasks: Vec<Task>,
    /// Text being typed.
    pub draft: String,
    /// Active filter.
    pub filter: Filter,
    theme: Theme,
}

/// Todo messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Input from the draft field.
    Edit(Input),
    /// Commit the draft as a task.
    Add,
    /// Flip a task.
    Toggle(usize),
    /// Change filter.
    Filter(Filter),
    /// Remove finished tasks.
    ClearDone,
}

impl Todo {
    /// A list with a few starter tasks.
    pub fn new(theme: Theme) -> Self {
        let t = |s: &str, d| Task {
            title: s.into(),
            done: d,
        };
        Self {
            tasks: vec![
                t("Order replacement fan", true),
                t("Write release notes", false),
                t("Book dentist", false),
            ],
            draft: String::new(),
            filter: Filter::All,
            theme,
        }
    }
}

impl Application for Todo {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Edit(Input::Text(s)) => self.draft.push_str(&s),
            Msg::Edit(Input::Key(Key::Backspace)) => {
                self.draft.pop();
            }
            Msg::Edit(Input::Key(Key::Enter)) => return self.update(Msg::Add),
            Msg::Edit(_) => {}
            Msg::Add => {
                let t = self.draft.trim();
                if !t.is_empty() {
                    self.tasks.push(Task {
                        title: t.to_string(),
                        done: false,
                    });
                    self.draft.clear();
                }
            }
            Msg::Toggle(i) => {
                if let Some(t) = self.tasks.get_mut(i) {
                    t.done = !t.done;
                }
            }
            Msg::Filter(f) => self.filter = f,
            Msg::ClearDone => self.tasks.retain(|t| !t.done),
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let p = &self.theme.palette;
        let open = self.tasks.iter().filter(|t| !t.done).count();
        let rows: Vec<Element<Msg>> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| match self.filter {
                Filter::All => true,
                Filter::Open => !t.done,
                Filter::Done => t.done,
            })
            .map(|(i, t)| checkbox(t.title.clone(), t.done, p, Msg::Toggle(i)).into())
            .collect();
        let seg = |text: &'static str, f: Filter| {
            button(text, Emphasis::Secondary, p, Msg::Filter(f))
                .selected(self.filter == f)
                .into()
        };
        column(vec![
            label("Tasks", 20.0, p.text).into(),
            label(format!("{open} open"), 12.0, p.text_secondary).into(),
            row(vec![
                editor(
                    vec![self.draft.clone()],
                    (0, self.draft.chars().count()),
                    p,
                    Msg::Edit,
                )
                .height(36.0)
                .into(),
                button("Add task", Emphasis::Primary, p, Msg::Add).into(),
            ])
            .spacing(8.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into(),
            column(rows)
                .spacing(4.0)
                .width(Length::Fill(1))
                .height(Length::Fill(1))
                .into(),
            row(vec![
                seg("All", Filter::All),
                seg("Open", Filter::Open),
                seg("Done", Filter::Done),
                spacer().into(),
                button("Clear done", Emphasis::Quiet, p, Msg::ClearDone).into(),
            ])
            .spacing(6.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into(),
        ])
        .spacing(12.0)
        .padding(Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .background(p.background)
        .into()
    }
}

/// Headless demo: type a task, add it, tick one, filter to open.
pub fn demo() -> Demo<Todo> {
    Demo {
        name: "todo",
        app: Todo::new(Theme::light()),
        size: (420.0, 340.0),
        steps: vec![
            Step::Type("Water the plants"),
            Step::Press(Key::Enter),
            Step::Click("Book dentist"),
            Step::Click("Open"),
        ],
    }
}
