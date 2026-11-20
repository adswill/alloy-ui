//! Counter: the smallest complete Elm-style app.
use crate::kit::{button, column, label, row, spacer, Emphasis};
use alloy_ui_core::style::{Align, Length, Padding};
use crate::run::{Demo, Step};
use alloy_ui_core::{Application, Command, Element};
use alloy_ui_theme::Theme;

/// Counter state.
pub struct Counter {
    /// Current value.
    pub value: i64,
    theme: Theme,
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
    pub fn new(theme: Theme) -> Self {
        Self { value: 0, theme }
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
        let p = &self.theme.palette;
        column(vec![
            label("Counter", 20.0, p.text).into(),
            label(format!("{} so far", self.value), 14.0, p.text_secondary).into(),
            spacer().into(),
            row(vec![
                button("Subtract", Emphasis::Secondary, p, Msg::Dec).into(),
                button("Add", Emphasis::Primary, p, Msg::Inc).into(),
                spacer().into(),
                button("Reset", Emphasis::Quiet, p, Msg::Reset).into(),
            ])
            .spacing(8.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into(),
        ])
        .spacing(4.0)
        .padding(Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .background(p.background)
        .into()
    }
}

/// The headless demo: click Add three times and Subtract once.
pub fn demo() -> Demo<Counter> {
    Demo {
        name: "counter",
        app: Counter::new(Theme::light()),
        size: (360.0, 180.0),
        steps: vec![Step::Click("Add"), Step::Click("Add"), Step::Click("Add"), Step::Click("Subtract")],
    }
}
