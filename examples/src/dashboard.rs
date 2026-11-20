//! Dashboard: a request-volume chart (the widgets' `BarChart`) and a table of recent jobs.
use alloy_ui::prelude::*;
use alloy_ui::widgets::BarChart;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (640.0, 560.0);

#[derive(Clone, Copy, PartialEq)]
enum Range {
    Hours,
    Days,
}

const HOURS: [f32; 12] = [
    32.0, 41.0, 38.0, 52.0, 61.0, 58.0, 72.0, 69.0, 81.0, 64.0, 55.0, 48.0,
];
const DAYS: [f32; 12] = [
    60.0, 55.0, 71.0, 66.0, 42.0, 38.0, 74.0, 79.0, 83.0, 77.0, 52.0, 45.0,
];
const HOUR_LABELS: [&str; 12] = [
    "9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
];
const DAY_LABELS: [&str; 12] = [
    "Nov 9", "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20",
];
const JOBS: [(&str, &str, &str); 5] = [
    ("nightly-index", "done", "4m 12s"),
    ("thumbnail-sweep", "done", "38s"),
    ("export-ledger", "failed", "1m 05s"),
    ("sync-calendars", "running", "12s"),
    ("rotate-keys", "queued", "-"),
];

struct Dash {
    range: Range,
    live: Vec<f32>,
    seed: u32,
}

fn view(d: &Dash) -> impl View<Act<Dash>> {
    let peak = d.live.iter().cloned().fold(0.0, f32::max);
    let labels = if d.range == Range::Hours {
        HOUR_LABELS
    } else {
        DAY_LABELS
    };
    let chart = BarChart::new(&alloy_ui::ui(), d.live.clone())
        .labels(labels)
        .max(100.0)
        .height(180.0);
    col![
        row![
            col![
                heading("Requests"),
                caption(format!("Peak {peak:.0}k per interval")).dim()
            ]
            .gap(2.0)
            .fill_w(),
            choose(
                [("12 hours", Range::Hours), ("12 days", Range::Days)],
                &d.range
            )
            .act(|d: &mut Dash, r| {
                d.range = r;
                d.live = if r == Range::Hours { HOURS } else { DAYS }.to_vec();
            }),
        ]
        .align_center()
        .fill_w(),
        custom(chart.into_element()),
        col![
            text("Recent jobs").sub(),
            table(&JOBS)
                .col("Job", |j| j.0.to_string())
                .col("State", |j| j.1.to_string())
                .num("Took", |j| j.2.to_string()),
        ]
        .gap(10.0),
    ]
    .gap(24.0)
    .pad(24.0)
    .fill()
}

/// The app. A one-second tick moves the chart so the idle cost of a live view is visible.
pub fn app() -> impl App {
    sketch(
        Dash {
            range: Range::Hours,
            live: HOURS.to_vec(),
            seed: 7,
        },
        view,
    )
    .tick(secs(1.0), |d| {
        d.seed = d.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let step = ((d.seed >> 16) % 100) as f32 / 100.0 - 0.5;
        let last = d.live.last().copied().unwrap_or(50.0);
        d.live.remove(0);
        d.live.push((last + step * 25.0).clamp(10.0, 95.0));
    })
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Dashboard", SIZE.0, SIZE.1)
}
