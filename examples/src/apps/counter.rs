//! Counter: the smallest complete Elm-style app.
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::{Button, Column, Container, Root, Row, Text, TextRole, Ui, Variant};

/// Counter state.
pub struct Counter {
    /// Current value.
    pub value: i64,
    ui: Ui,
}

/// Counter messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Add one.
    Inc,
    /// Subtract one.
    Dec,
    /// Back to zero.
    Reset,
}

impl Counter {
    /// A counter at zero.
    pub fn new(ui: &Ui) -> Self {
        Self {
            value: 0,
            ui: ui.clone(),
        }
    }
}

impl Application for Counter {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Inc => self.value += 1,
            Msg::Dec => self.value -= 1,
            Msg::Reset => self.value = 0,
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let ui = &self.ui;
        let bg = ui.theme().palette.background;
        let body = Column::new(vec![
            Text::new(ui, "Counter")
                .role(TextRole::Heading)
                .into_element(),
            Text::new(ui, format!("{} so far", self.value))
                .secondary()
                .into_element(),
            Row::new(vec![
                Button::new(ui, "Subtract")
                    .on_press(Msg::Dec)
                    .into_element(),
                Button::new(ui, "Add")
                    .on_press(Msg::Inc)
                    .variant(Variant::Primary)
                    .into_element(),
                Button::new(ui, "Reset")
                    .on_press(Msg::Reset)
                    .variant(Variant::Ghost)
                    .into_element(),
            ])
            .spacing(8.0)
            .align(Align::Center)
            .into_element(),
        ])
        .spacing(14.0)
        .padding(Padding::all(24.0))
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

/// The headless demo: click Add three times and Subtract once.
pub fn demo() -> Demo<Counter> {
    Demo {
        name: "counter",
        app: Counter::new(&Ui::new(Theme::light())),
        size: (360.0, 180.0),
        steps: vec![
            Step::Click("Add"),
            Step::Click("Add"),
            Step::Click("Add"),
            Step::Click("Subtract"),
        ],
    }
}
