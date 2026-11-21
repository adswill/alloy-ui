//! Notes: searchable list on the left, editor on the right.

#[path = "../ui.rs"]
mod ui;

use iced::widget::{
    button, column, container, row, scrollable, space, text, text_editor, text_input, toggler,
};
use iced::{widget, Element, Length, Settings, Task, Theme};

fn main() -> iced::Result {
    iced::application(Notes::new, Notes::update, Notes::view)
        .title("Notes")
        .theme(Notes::theme)
        .settings(Settings {
            default_text_size: 14.into(),
            ..Settings::default()
        })
        .window_size((960.0, 640.0))
        .centered()
        .run()
}

const TITLE_ID: widget::Id = widget::Id::new("note-title");

struct Note {
    id: u32,
    title: String,
    body: text_editor::Content,
    pinned: bool,
}

impl Note {
    fn new(id: u32, title: &str, body: &str, pinned: bool) -> Self {
        Self {
            id,
            title: title.to_owned(),
            body: text_editor::Content::with_text(body),
            pinned,
        }
    }

    fn preview(&self) -> String {
        self.body
            .lines()
            .map(|line| line.text.trim().to_owned())
            .find(|line| !line.is_empty())
            .unwrap_or_else(|| "No content".to_owned())
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        query.is_empty()
            || self.title.to_lowercase().contains(&query)
            || self.body.text().to_lowercase().contains(&query)
    }
}

struct Notes {
    notes: Vec<Note>,
    selected: Option<u32>,
    search: String,
    next_id: u32,
    dark: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Search(String),
    Select(u32),
    New,
    TitleChanged(String),
    Edit(text_editor::Action),
    TogglePin,
    Delete,
    ToggleDark(bool),
}

impl Notes {
    fn new() -> Self {
        let notes = vec![
            Note::new(
                1,
                "Roadmap Q3",
                "Ship the sync engine beta.\nPolish onboarding and cut the first-run time in half.\nDecide on pricing before the August review.",
                true,
            ),
            Note::new(
                2,
                "Grocery list",
                "Oat milk, lemons, sourdough, olive oil\nSomething green for Thursday dinner.",
                false,
            ),
            Note::new(
                3,
                "Meeting notes: design sync",
                "Agreed on an eight pixel radius everywhere.\nOne accent colour, no gradients.\nFollow up on the dark theme contrast.",
                true,
            ),
            Note::new(
                4,
                "Book recommendations",
                "The Design of Everyday Things\nA Pattern Language\nThe Mythical Man-Month",
                false,
            ),
            Note::new(
                5,
                "Trip ideas",
                "Lisbon in October, train to Porto.\nSlow week in the Dolomites, maybe in June.",
                false,
            ),
        ];
        Self {
            selected: Some(1),
            notes,
            search: String::new(),
            next_id: 6,
            dark: false,
        }
    }

    fn theme(&self) -> Theme {
        ui::theme(self.dark, ui::INDIGO)
    }

    fn current_mut(&mut self) -> Option<&mut Note> {
        let id = self.selected?;
        self.notes.iter_mut().find(|note| note.id == id)
    }

    /// Notes matching the search, pinned ones first.
    fn visible(&self) -> Vec<&Note> {
        let mut notes: Vec<&Note> = self
            .notes
            .iter()
            .filter(|note| note.matches(&self.search))
            .collect();
        notes.sort_by_key(|note| !note.pinned);
        notes
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Search(search) => self.search = search,
            Message::Select(id) => self.selected = Some(id),
            Message::New => {
                let id = self.next_id;
                self.next_id += 1;
                self.notes.insert(0, Note::new(id, "", "", false));
                self.selected = Some(id);
                self.search.clear();
                return widget::operation::focus(TITLE_ID);
            }
            Message::TitleChanged(title) => {
                if let Some(note) = self.current_mut() {
                    note.title = title;
                }
            }
            Message::Edit(action) => {
                if let Some(note) = self.current_mut() {
                    note.body.perform(action);
                }
            }
            Message::TogglePin => {
                if let Some(note) = self.current_mut() {
                    note.pinned = !note.pinned;
                }
            }
            Message::Delete => {
                if let Some(id) = self.selected.take() {
                    self.notes.retain(|note| note.id != id);
                    self.selected = self.visible().first().map(|note| note.id);
                }
            }
            Message::ToggleDark(dark) => self.dark = dark,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        row![self.sidebar(), self.editor()].into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let header = row![
            ui::heading("Notes"),
            space::horizontal(),
            button(text("New note").size(13))
                .padding([6, 12])
                .style(ui::primary)
                .on_press(Message::New),
        ]
        .align_y(iced::Center);

        let search = text_input("Search notes", &self.search)
            .on_input(Message::Search)
            .padding([8, 10])
            .style(ui::input);

        let visible = self.visible();
        let list: Element<'_, Message> = if visible.is_empty() {
            container(ui::caption("No notes match your search"))
                .padding(16)
                .center_x(Length::Fill)
                .into()
        } else {
            scrollable(
                column(visible.into_iter().map(|note| self.list_item(note)))
                    .spacing(2)
                    .padding([0, 12]),
            )
            .style(ui::scroll)
            .height(Length::Fill)
            .into()
        };

        let footer = row![
            ui::caption("Dark"),
            space::horizontal(),
            toggler(self.dark)
                .on_toggle(Message::ToggleDark)
                .size(18)
                .style(ui::switch),
        ]
        .align_y(iced::Center);

        container(
            column![
                column![header, search].spacing(12).padding([16, 16]),
                list,
                container(footer).padding([12, 16]),
            ]
            .height(Length::Fill),
        )
        .width(290)
        .height(Length::Fill)
        .style(ui::sidebar)
        .into()
    }

    fn list_item<'a>(&self, note: &'a Note) -> Element<'a, Message> {
        let selected = self.selected == Some(note.id);
        let title = if note.title.trim().is_empty() {
            "Untitled"
        } else {
            note.title.as_str()
        };

        let mut content = column![].spacing(2);
        if note.pinned {
            content = content.push(text("Pinned").size(12).style(ui::accent));
        }
        content = content
            .push(
                text(title)
                    .size(14)
                    .font(ui::SEMIBOLD)
                    .wrapping(text::Wrapping::None),
            )
            .push(
                text(note.preview())
                    .size(12)
                    .style(ui::muted)
                    .wrapping(text::Wrapping::None),
            );

        button(container(content).width(Length::Fill).clip(true))
            .width(Length::Fill)
            .padding([9, 12])
            .style(ui::list_item(selected))
            .on_press(Message::Select(note.id))
            .into()
    }

    fn editor(&self) -> Element<'_, Message> {
        let Some(note) = self
            .selected
            .and_then(|id| self.notes.iter().find(|note| note.id == id))
        else {
            return container(ui::caption("Select a note or create a new one"))
                .center(Length::Fill)
                .style(ui::page)
                .into();
        };

        let words = note.body.text().split_whitespace().count();
        let count = format!("{words} {}", if words == 1 { "word" } else { "words" });

        let title = text_input("Title", &note.title)
            .id(TITLE_ID)
            .on_input(Message::TitleChanged)
            .size(20)
            .font(ui::SEMIBOLD)
            .padding([6, 14])
            .style(ui::input_bare);

        let body = text_editor(&note.body)
            .placeholder("Start writing")
            .on_action(Message::Edit)
            .padding([12, 14])
            .height(Length::Fill)
            .style(ui::editor);

        let actions = row![
            ui::caption(count),
            space::horizontal(),
            button(text(if note.pinned { "Unpin" } else { "Pin" }).size(13))
                .padding([6, 14])
                .style(ui::secondary)
                .on_press(Message::TogglePin),
            button(text("Delete").size(13))
                .padding([6, 14])
                .style(ui::destructive)
                .on_press(Message::Delete),
        ]
        .spacing(8)
        .align_y(iced::Center);

        container(column![title, body, actions].spacing(14))
            .padding([20, 28])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(ui::page)
            .into()
    }
}
