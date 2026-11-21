//! The spectrum and waterfall widgets render through the real runtime.
use alloy_ui::{prelude::*, spectrum, testing::Harness, Waterfall};

struct S {
    wf: Waterfall,
    row: Vec<f32>,
}

fn line(t: usize) -> Vec<f32> {
    (0..2048)
        .map(|i| {
            let x = i as f32 / 2048.0;
            let peak = (-((x - 0.3 - 0.0005 * t as f32) * 60.0).powi(2)).exp() * 50.0;
            -95.0 + peak + ((i * 7919 + t * 31) % 13) as f32 * 0.3
        })
        .collect()
}

#[test]
fn spectrum_and_waterfall_draw() {
    let wf = Waterfall::new(512, 64);
    wf.range(-100.0, -40.0);
    for t in 0..100 {
        wf.push_row(&line(t));
    }
    let state = S {
        wf: wf.clone(),
        row: line(100),
    };
    let app = sketch(state, |s: &S| {
        col![spectrum(&s.row, -100.0, -30.0, 120.0), s.wf.view(200.0)].pad(8.0)
    });
    let mut ui = Harness::new(app, 640.0, 380.0);
    ui.advance(16);
    let path = std::env::temp_dir().join("alloy-scope.png");
    ui.save_png(&path);
    assert!(path.exists());
    eprintln!("wrote {}", path.display());
}
