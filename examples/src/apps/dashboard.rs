//! Dashboard: a request-volume chart and a table of recent jobs. Static data;
//! the point is dense, aligned layout rather than a hero number.
use crate::custom::bars;
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element, Subscription};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::{Button, Column, Container, Root, Row, Text, TextRole, Ui, Variant};
use std::time::Duration;

/// Range shown in the chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// Last 12 hours.
    Hours,
    /// Last 12 days.
    Days,
}

/// Dashboard state.
pub struct Dashboard {
    /// Selected range.
    pub range: Range,
    /// Live series, oldest first, each in 0..=1.
    pub live: Vec<f32>,
    seed: u32,
    ui: Ui,
}

/// Dashboard messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Change range.
    Range(Range),
    /// One second passed: append a sample.
    Tick,
}

const HOURS: [f32; 12] = [
    0.32, 0.41, 0.38, 0.52, 0.61, 0.58, 0.72, 0.69, 0.81, 0.64, 0.55, 0.48,
];
const DAYS: [f32; 12] = [
    0.60, 0.55, 0.71, 0.66, 0.42, 0.38, 0.74, 0.79, 0.83, 0.77, 0.52, 0.45,
];
const JOBS: [(&str, &str, &str); 4] = [
    ("nightly-index", "done", "4m 12s"),
    ("thumbnail-sweep", "done", "38s"),
    ("export-ledger", "failed", "1m 05s"),
    ("sync-calendars", "running", "12s"),
];

impl Dashboard {
    /// Default view.
    pub fn new(ui: &Ui) -> Self {
        Self {
            range: Range::Hours,
            live: HOURS.to_vec(),
            seed: 7,
            ui: ui.clone(),
        }
    }
}

impl Application for Dashboard {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Range(r) => {
                self.range = r;
                self.live = match r {
                    Range::Hours => HOURS.to_vec(),
                    Range::Days => DAYS.to_vec(),
                };
            }
            Msg::Tick => {
                // Deterministic pseudo-random walk so runs are reproducible.
                self.seed = self
                    .seed
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223);
                let step = ((self.seed >> 16) % 100) as f32 / 100.0 - 0.5;
                let last = self.live.last().copied().unwrap_or(0.5);
                self.live.remove(0);
                self.live.push((last + step * 0.25).clamp(0.1, 0.95));
            }
        }
        Command::None
    }

    fn subscription(&self) -> Vec<Subscription<Msg>> {
        vec![Subscription::every(1, Duration::from_secs(1), || Msg::Tick)]
    }

    fn view(&self) -> Element<Msg> {
        let ui = &self.ui;
        let t = ui.theme();
        let p = &t.palette;
        let data = &self.live;
        let peak = data.iter().cloned().fold(0.0, f32::max);
        let seg = |text: &'static str, r: Range| -> Element<Msg> {
            let v = if self.range == r {
                Variant::Secondary
            } else {
                Variant::Ghost
            };
            Button::new(ui, text)
                .on_press(Msg::Range(r))
                .variant(v)
                .into_element()
        };
        let cell = |text: &str, w: f32, secondary: bool, danger: bool| -> Element<Msg> {
            let mut c = Text::new(ui, text.to_string())
                .role(TextRole::Caption)
                .width(Length::Fixed(w))
                .no_wrap();
            if secondary {
                c = c.secondary();
            }
            if danger {
                c = c.color(p.danger);
            }
            c.into_element()
        };
        let mut table: Vec<Element<Msg>> = vec![Row::new(vec![
            cell("Job", 170.0, true, false),
            cell("State", 90.0, true, false),
            cell("Time", 80.0, true, false),
        ])
        .into_element()];
        for (name, state, time) in JOBS {
            table.push(
                Row::new(vec![
                    cell(name, 170.0, false, false),
                    cell(state, 90.0, false, state == "failed"),
                    cell(time, 80.0, false, false),
                ])
                .into_element(),
            );
        }
        let body = Column::new(vec![
            Row::new(vec![
                Text::new(ui, "Requests")
                    .role(TextRole::Heading)
                    .width(Length::Fill(1))
                    .into_element(),
                seg("12 hours", Range::Hours),
                seg("12 days", Range::Days),
            ])
            .spacing(4.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into_element(),
            Text::new(ui, format!("Peak {:.0}k per interval", peak * 100.0))
                .role(TextRole::Caption)
                .secondary()
                .into_element(),
            bars(data.clone(), p.accent, p.surface).into(),
            Text::new(ui, "Recent jobs")
                .role(TextRole::Subheading)
                .into_element(),
            Column::new(table).spacing(8.0).into_element(),
        ])
        .spacing(16.0)
        .padding(Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .into_element();
        Root::new(
            ui,
            Container::new(body)
                .background(p.background)
                .width(Length::Fill(1))
                .height(Length::Fill(1))
                .into_element(),
        )
        .into_element()
    }
}

/// Headless demo: switch to the 12-day range.
pub fn demo() -> Demo<Dashboard> {
    Demo {
        name: "dashboard",
        app: Dashboard::new(&crate::run::headless_ui(Theme::light())),
        size: (520.0, 460.0),
        steps: vec![Step::Click("12 days")],
    }
}
