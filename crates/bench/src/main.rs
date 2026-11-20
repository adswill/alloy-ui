//! Alloy benchmark harness.
//!
//! Measures only what exists today and labels it honestly:
//! * first and repeat frame: runtime + layout + draw + 2x CPU raster + PNG encode through the
//!   real runtime (the software reference renderer, not the GPU path),
//! * idle: runtime cost per no-op input event (a bound on idle work, not process CPU%),
//! * startup: wall time from process spawn to a written first frame (headless),
//! * binary size: release `counter` binary.
//!
//! Windowed numbers (first paint, fps, idle CPU) come from `windowed.py` next to this crate.

use std::process::Command;
use std::time::Instant;

use alloy_ui::testing::Harness;
use alloy_ui::{prelude::App, Key};
use alloy_ui_examples::{counter, dashboard, settings, text_editor, todo};

const FRAMES: usize = 15;

struct Row {
    name: &'static str,
    first_ms: f64,
    raster_ms: f64,
    idle_ns: f64,
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Cost of the first frame (build runtime, layout, draw, 2x CPU raster, PNG encode), the
/// steady-state repeat of the same, and a no-op event, through the real runtime.
fn measure(name: &'static str, app: impl App, size: (f32, f32)) -> Row {
    let t = Instant::now();
    let mut h = Harness::new(app, size.0, size.1);
    let _ = h.png();
    let first_ms = t.elapsed().as_secs_f64() * 1e3;
    let raster_ms = median(
        (0..FRAMES)
            .map(|_| {
                let t = Instant::now();
                let _ = h.png();
                t.elapsed().as_secs_f64() * 1e3
            })
            .collect(),
    );
    let n = 2_000;
    let t = Instant::now();
    for _ in 0..n {
        h.press(Key::Escape);
    }
    let idle_ns = t.elapsed().as_secs_f64() * 1e9 / n as f64;
    Row {
        name,
        first_ms,
        raster_ms,
        idle_ns,
    }
}

fn cargo_release() -> std::path::PathBuf {
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "--release", "-p", "alloy-ui-examples", "--bins"])
        .status()
        .expect("run cargo");
    assert!(status.success(), "release build failed");
    // The bench binary is run from a release build, so its directory is target/release.
    let dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    assert!(dir.ends_with("release"), "run alloy-bench with --release");
    dir
}

fn main() {
    let rows = vec![
        measure("counter", counter::app(), counter::SIZE),
        measure("todo", todo::app(), todo::SIZE),
        measure("text_editor", text_editor::app(), text_editor::SIZE),
        measure("settings", settings::app(), settings::SIZE),
        measure("dashboard", dashboard::app(), dashboard::SIZE),
    ];

    let rel = cargo_release();
    let ext = if cfg!(windows) { ".exe" } else { "" };
    let counter_bin = rel.join(format!("counter{ext}"));
    let size_mb = std::fs::metadata(&counter_bin)
        .map(|m| m.len() as f64 / 1_048_576.0)
        .unwrap_or(f64::NAN);
    let tmp = std::env::temp_dir();
    let mut starts = Vec::new();
    for _ in 0..7 {
        let t = Instant::now();
        let ok = Command::new(&counter_bin)
            .arg("--headless")
            .arg(tmp.join("counter.png"))
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok, "counter failed to run");
        starts.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let startup_ms = median(starts);

    println!("### {} / {}", std::env::consts::OS, std::env::consts::ARCH);
    println!();
    println!("| Example | First frame (runtime + 2x raster + PNG) | Repeat frame (same, median) | Per no-op key event |");
    println!("|---|---|---|---|");
    for r in &rows {
        println!(
            "| {} | {:.1} ms | {:.1} ms | {:.0} us |",
            r.name,
            r.first_ms,
            r.raster_ms,
            r.idle_ns / 1e3
        );
    }
    println!();
    println!("| Metric | Value | Target |");
    println!("|---|---|---|");
    println!("| counter release binary (windowed build, also runs headless) | {size_mb:.2} MB | < 10 MB |");
    println!("| counter spawn to first frame written (--headless, median of 7) | {startup_ms:.0} ms | < 200 ms |");
    println!("| windowed fps and first paint | see `windowed.py` | 60 fps |");
}
