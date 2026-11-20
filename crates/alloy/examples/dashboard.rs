//! Dashboard: a request-volume chart and a table of recent jobs.
use alloy_ui::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Range {
    Hours,
    Days,
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

struct Dash {
    range: Range,
    live: Vec<f32>,
    seed: u32,
}

fn view(d: &Dash) -> impl View<Act<Dash>> {
    let peak = d.live.iter().cloned().fold(0.0, f32::max);
    let cell = |s: &'static str, w| text(s).caption().no_wrap().w(w);
    col![
        row![
            heading("Requests").fill_w(),
            choose(
                [("12 hours", Range::Hours), ("12 days", Range::Days)],
                d.range
            )
            .act(|d: &mut Dash, r| {
                d.range = r;
                d.live = if r == Range::Hours { HOURS } else { DAYS }.to_vec();
            }),
        ]
        .align_center()
        .fill_w(),
        caption(format!("Peak {:.0}k per interval", peak * 100.0)).dim(),
        bars(d.live.clone()),
        text("Recent jobs").sub(),
        col(JOBS.map(|(name, state, time)| {
            row![
                cell(name, 170.0),
                cell(state, 90.0).danger_if(state == "failed"),
                cell(time, 80.0)
            ]
        })),
    ]
    .gap(16.0)
    .pad(24.0)
    .fill()
}

fn main() {
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
        let last = d.live.last().copied().unwrap_or(0.5);
        d.live.remove(0);
        d.live.push((last + step * 0.25).clamp(0.1, 0.95));
    })
    .run_sized("Dashboard", 520.0, 460.0)
}
