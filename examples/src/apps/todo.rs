//! Todo list: input line, scrolling list of checkboxes, filter.
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element, Key};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::{
    Button, Checkbox, Column, Container, Root, Row, Scrollable, Text, TextInput, TextRole, Ui,
    Variant,
};

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
    ui: Ui,
}

/// Todo messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// The draft text changed.
    Draft(String),
    /// Commit the draft as a task.
    Add,
    /// Set a task's done flag.
    Set(usize, bool),
    /// Change filter.
    Filter(Filter),
    /// Remove finished tasks.
    ClearDone,
}

const INPUT_ID: u64 = 1;
const LIST_ID: u64 = 2;

impl Todo {
    /// A list with a few starter tasks.
    pub fn new(ui: &Ui) -> Self {
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
            ui: ui.clone(),
        }
    }
}

impl Application for Todo {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Draft(s) => self.draft = s,
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
            Msg::Set(i, v) => {
                if let Some(t) = self.tasks.get_mut(i) {
                    t.done = v;
                }
            }
            Msg::Filter(f) => self.filter = f,
            Msg::ClearDone => self.tasks.retain(|t| !t.done),
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let ui = &self.ui;
        let bg = ui.theme().palette.background;
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
            .map(|(i, t)| {
                Checkbox::new(ui, t.title.clone(), t.done)
                    .on_toggle(move |v| Msg::Set(i, v))
                    .into_element()
            })
            .collect();
        let seg = |text: &'static str, f: Filter| {
            let v = if self.filter == f {
                Variant::Secondary
            } else {
                Variant::Ghost
            };
            Button::new(ui, text)
                .on_press(Msg::Filter(f))
                .variant(v)
                .into_element()
        };
        let body = Column::new(vec![
            Column::new(vec![
                Text::new(ui, "Tasks")
                    .role(TextRole::Heading)
                    .into_element(),
                Text::new(ui, format!("{open} open"))
                    .role(TextRole::Caption)
                    .secondary()
                    .into_element(),
            ])
            .spacing(2.0)
            .into_element(),
            Row::new(vec![
                TextInput::new(ui, INPUT_ID, self.draft.clone())
                    .placeholder("What needs doing?")
                    .on_input(Msg::Draft)
                    .on_submit(|| Msg::Add)
                    .width(Length::Fill(1))
                    .into_element(),
                Button::new(ui, "Add task")
                    .on_press(Msg::Add)
                    .variant(Variant::Primary)
                    .into_element(),
            ])
            .spacing(8.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into_element(),
            Scrollable::new(
                ui,
                LIST_ID,
                Column::new(rows)
                    .spacing(8.0)
                    .width(Length::Fill(1))
                    .into_element(),
            )
            .height(Length::Fill(1))
            .into_element(),
            Row::new(vec![
                seg("All", Filter::All),
                seg("Open", Filter::Open),
                seg("Done", Filter::Done),
                Button::new(ui, "Clear done")
                    .on_press(Msg::ClearDone)
                    .variant(Variant::Ghost)
                    .into_element(),
            ])
            .spacing(4.0)
            .into_element(),
        ])
        .spacing(18.0)
        .padding(Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .into_element();
        Root::new(
            ui,
            Container::new(body)
                .background(bg)
                .width(Length::Fill(1))
                .height(Length::Fill(1))
                .into_element(),
        )
        .into_element()
    }
}

/// Headless demo: type a task, submit it, tick one, filter to open.
pub fn demo() -> Demo<Todo> {
    Demo {
        name: "todo",
        app: Todo::new(&crate::run::headless_ui(Theme::light())),
        size: (420.0, 340.0),
        steps: vec![
            Step::Click("What needs doing?"),
            Step::Type("Water the plants"),
            Step::Press(Key::Enter),
            Step::Click("Book dentist"),
            Step::Click("Open"),
        ],
    }
}
