//! Spectrum and waterfall at display rate: the RF display the OnAir port needs.
//!
//! `cargo run --release -p alloy-ui --example scope`
//! Set `ALLOY_SCOPE_SECS=10` to run for that long, print frame statistics to stderr and exit
//! (used by CI to measure frame rate per platform).
use alloy_ui::prelude::*;
use alloy_ui::{spectrum, Waterfall};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const BINS: usize = 8192;
const ROWS: u32 = 1024;

struct Scope {
    wf: Waterfall,
    row: Vec<f32>,
    seed: u32,
    t: f32,
}

impl Scope {
    fn new() -> Self {
        let wf = Waterfall::new(BINS as u32, ROWS);
        wf.range(-110.0, -40.0);
        Self {
            wf,
            row: vec![-100.0; BINS],
            seed: 0x1234_5678,
            t: 0.0,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 17;
        self.seed ^= self.seed << 5;
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }

    /// One new line: noise floor, a few drifting carriers and a wide DVB-T-like block.
    fn step(&mut self) {
        self.t += 0.016;
        for i in 0..BINS {
            let x = i as f32 / BINS as f32;
            let mut v = -98.0 + (self.rand() - 0.5) * 8.0;
            if (0.55..0.75).contains(&x) {
                v = v.max(-72.0 + (self.rand() - 0.5) * 5.0);
            }
            self.row[i] = v;
        }
        for k in 0..6 {
            let c = (0.08 + 0.13 * k as f32 + 0.02 * (self.t * (0.4 + k as f32 * 0.1)).sin())
                * BINS as f32;
            for d in -3i32..=3 {
                let i = (c as i32 + d).clamp(0, BINS as i32 - 1) as usize;
                self.row[i] = self.row[i].max(-50.0 - (d.abs() as f32) * 9.0 - k as f32 * 3.0);
            }
        }
        self.wf.push_row(&self.row);
    }
}

static START: Mutex<Option<Instant>> = Mutex::new(None);
static FRAMES: Mutex<Vec<f64>> = Mutex::new(Vec::new());

/// Counts paints; with `ALLOY_SCOPE_SECS` set, reports and exits after that long.
fn frame_probe() {
    let now = Instant::now();
    let mut start = START.lock().unwrap();
    let s = *start.get_or_insert(now);
    let t = now.duration_since(s).as_secs_f64();
    let mut frames = FRAMES.lock().unwrap();
    frames.push(t);
    let Some(limit) = std::env::var("ALLOY_SCOPE_SECS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    else {
        return;
    };
    if t < limit {
        return;
    }
    // Skip the first second (window and shader start-up).
    let steady: Vec<f64> = frames.iter().copied().filter(|x| *x > 1.0).collect();
    let mut dt: Vec<f64> = steady.windows(2).map(|w| (w[1] - w[0]) * 1000.0).collect();
    dt.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = dt.len().max(1);
    let mean = dt.iter().sum::<f64>() / n as f64;
    let pct = |p: f64| {
        dt.get(((n as f64 * p) as usize).min(n - 1))
            .copied()
            .unwrap_or(0.0)
    };
    eprintln!(
        "SCOPE frames={} fps={:.1} mean_ms={:.2} p50_ms={:.2} p95_ms={:.2} p99_ms={:.2} max_ms={:.2}",
        steady.len(),
        1000.0 / mean.max(1e-6),
        mean,
        pct(0.5),
        pct(0.95),
        pct(0.99),
        dt.last().copied().unwrap_or(0.0)
    );
    std::process::exit(0);
}

fn main() {
    sketch(Scope::new(), |s| {
        col![
            row![
                heading("Scope"),
                text("8192 bins, 1024 rows of history").dim()
            ]
            .gap(12.0),
            spectrum(&s.row, -110.0, -30.0, 220.0),
            s.wf.view(420.0),
            canvas(1.0, 1.0, |_| frame_probe()).live(),
        ]
        .gap(8.0)
        .pad(12.0)
        .fill()
    })
    .tick(Duration::from_millis(8), |s| s.step())
    .run_sized("Scope", 1280.0, 760.0)
}
