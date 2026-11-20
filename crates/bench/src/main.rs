//! Alloy benchmark harness.
//!
//! Measures only what exists today and labels it honestly:
//! * frame build: `update` + `view` + layout + draw-list build through the real runtime,
//! * CPU raster: the software reference renderer at 2x (not the GPU path),
//! * idle: runtime cost per no-op input event (a bound on idle work, not process CPU%),
//! * startup: wall time from process spawn to a written first frame (headless),
//! * binary size: release `counter` binary (headless build, includes the renderer).
//!
//! FPS, real idle CPU% and windowed startup need the platform shell and are
//! reported as "not measured" until it exists.

use std::process::Command;
use std::time::Instant;

use alloy_ui_core::{Application, Event, Point};
use alloy_ui_examples::apps::{counter, dashboard, settings, text_editor, todo};
use alloy_ui_examples::run::{apply, click_text, render, runtime, Demo};
use alloy_ui_render::{GpuHeadlessRenderer, HeadlessRenderer};

const FRAMES: usize = 300;

struct Row {
    name: &'static str,
    frame_us: f64,
    raster_ms: f64,
    idle_ns: f64,
    gpu_ms: Option<f64>,
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn measure<A: Application>(demo: Demo<A>) -> Row {
    let name = demo.name;
    let size = demo.size;
    let mut rt = runtime(demo.app, size);
    for s in &demo.steps {
        apply(&mut rt, s);
    }
    // Frame build: re-dispatch the last state change repeatedly via a resize toggle,
    // which forces view rebuild + layout + draw every time.
    let mut times = Vec::new();
    for i in 0..FRAMES {
        let w = size.0 + (i % 2) as f32;
        let t = Instant::now();
        rt.handle_event(&Event::Resized(alloy_ui_core::Size::new(w, size.1)));
        let _ = rt.draw();
        times.push(t.elapsed().as_secs_f64() * 1e6);
    }
    let frame_us = median(times);
    // Raster: median of 5 after one warm-up (font cache).
    let _ = render(&rt, size);
    let raster = (0..5)
        .map(|_| {
            let t = Instant::now();
            let _ = render(&rt, size);
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let raster_ms = median(raster);
    // GPU path (offscreen wgpu, includes read-back); None when no adapter exists.
    let (w, h) = (size.0 as u32 * 2, size.1 as u32 * 2);
    let gpu_ms = GpuHeadlessRenderer::try_new(w, h).ok().map(|mut g| {
        let list = rt.draw();
        let _ = g.render_to_image(&list, w, h, 2.0);
        median(
            (0..20)
                .map(|_| {
                    let t = Instant::now();
                    let _ = g.render_to_image(&list, w, h, 2.0);
                    t.elapsed().as_secs_f64() * 1e3
                })
                .collect(),
        )
    });
    // Idle: no-op events that change nothing.
    let n = 20_000;
    let t = Instant::now();
    for i in 0..n {
        rt.handle_event(&Event::CursorMoved(Point::new(0.0, (i % 2) as f32)));
    }
    let idle_ns = t.elapsed().as_secs_f64() * 1e9 / n as f64;
    Row {
        name,
        frame_us,
        raster_ms,
        idle_ns,
        gpu_ms,
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
        measure(counter::demo()),
        measure(todo::demo()),
        measure(text_editor::demo()),
        measure(settings::demo()),
        measure(dashboard::demo()),
    ];
    let _ = click_text::<counter::Counter>; // keep API exercised by the harness build

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
            .arg(&tmp)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok, "counter failed to run");
        starts.push(t.elapsed().as_secs_f64() * 1e3);
    }
    let startup_ms = median(starts);

    println!("### {} / {}", std::env::consts::OS, std::env::consts::ARCH);
    println!();
    let adapter = GpuHeadlessRenderer::try_new(64, 64)
        .map(|g| g.adapter_name())
        .unwrap_or_else(|_| "none".into());
    println!("Offscreen wgpu adapter: {adapter}");
    println!();
    println!("| Example | Frame build | CPU raster 2x | wgpu offscreen 2x (with read-back) | Frames/s possible (wgpu) | Per no-op event |");
    println!("|---|---|---|---|---|---|");
    for r in &rows {
        let (gpu, fps) = match r.gpu_ms {
            Some(g) => (
                format!("{g:.1} ms"),
                format!("{:.0}", 1000.0 / (g + r.frame_us / 1e3)),
            ),
            None => ("n/a".into(), "n/a".into()),
        };
        println!(
            "| {} | {:.0} us | {:.1} ms | {gpu} | {fps} | {:.0} ns |",
            r.name, r.frame_us, r.raster_ms, r.idle_ns
        );
    }
    println!();
    println!("| Metric | Value | Target |");
    println!("|---|---|---|");
    println!("| counter release binary (windowed build, also runs headless) | {size_mb:.2} MB | < 10 MB |");
    println!("| counter spawn to first frame written (--headless, median of 7) | {startup_ms:.0} ms | < 200 ms |");
    println!(
        "| fps, ProMotion 120 fps | not measured (no frame counter in the shell yet) | 60 fps |"
    );
}
