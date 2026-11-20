//! Dashboard: a request-volume chart and a table of recent jobs. Static data;
//! the point is dense, aligned layout rather than a hero number.
use crate::kit::{bars, button, column, label, row, spacer, Emphasis};
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element};
use alloy_ui_theme::Theme;

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
    theme: Theme,
}

/// Dashboard messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Change range.
    Range(Range),
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
    pub fn new(theme: Theme) -> Self {
        Self {
            range: Range::Hours,
            theme,
        }
    }
}

impl Application for Dashboard {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        let Msg::Range(r) = m;
        self.range = r;
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let p = &self.theme.palette;
        let data = match self.range {
            Range::Hours => HOURS,
            Range::Days => DAYS,
        };
        let peak = data.iter().cloned().fold(0.0, f32::max);
        let seg = |t: &'static str, r: Range| -> Element<Msg> {
            button(t, Emphasis::Secondary, p, Msg::Range(r))
                .selected(self.range == r)
                .into()
        };
        let cell = |t: &str, w: f32, c| -> Element<Msg> {
            column(vec![label(t, 13.0, c).into()])
                .width(Length::Fixed(w))
                .into()
        };
        let mut table: Vec<Element<Msg>> = vec![row(vec![
            cell("Job", 170.0, p.text_secondary),
            cell("State", 90.0, p.text_secondary),
            cell("Time", 80.0, p.text_secondary),
        ])
        .into()];
        for (name, state, time) in JOBS {
            let c = if state == "failed" { p.danger } else { p.text };
            table.push(
                row(vec![
                    cell(name, 170.0, p.text),
                    cell(state, 90.0, c),
                    cell(time, 80.0, p.text),
                ])
                .into(),
            );
        }
        column(vec![
            row(vec![
                label("Requests", 20.0, p.text).into(),
                spacer().into(),
                seg("12 hours", Range::Hours),
                seg("12 days", Range::Days),
            ])
            .spacing(6.0)
            .align(Align::Center)
            .width(Length::Fill(1))
            .into(),
            label(
                format!("peak {:.0}k per interval", peak * 100.0),
                12.0,
                p.text_secondary,
            )
            .into(),
            bars(data.to_vec(), p.accent, p.surface).into(),
            label("Recent jobs", 16.0, p.text).into(),
            column(table).spacing(6.0).into(),
        ])
        .spacing(12.0)
        .padding(Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .background(p.background)
        .into()
    }
}

/// Headless demo: switch to the 12-day range.
pub fn demo() -> Demo<Dashboard> {
    Demo {
        name: "dashboard",
        app: Dashboard::new(Theme::light()),
        size: (520.0, 400.0),
        steps: vec![Step::Click("12 days")],
    }
}
