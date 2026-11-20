//! Per-phase timing of one frame: draw-list build, CPU raster, GPU offscreen.
//! Prints microseconds. `phases [text_editor|dashboard]`.
use std::time::Instant;

use alloy_ui::testing::Harness;
use alloy_ui::App;
use alloy_ui_examples::{dashboard, text_editor};
use alloy_ui_render::{CpuRenderer, GpuHeadlessRenderer, HeadlessRenderer};

fn med(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("dashboard") => run(dashboard::app(), dashboard::SIZE),
        _ => run(text_editor::app(), text_editor::SIZE),
    }
}

fn run(app: impl App, size: (f32, f32)) {
    let mut h = Harness::new(app, size.0, size.1);
    h.settle();
    let t = |f: &mut dyn FnMut()| {
        med((0..15)
            .map(|_| {
                let s = Instant::now();
                f();
                s.elapsed().as_secs_f64() * 1e6
            })
            .collect())
    };
    let list = h.draw_list();
    println!("commands: {}", list.0.len());
    let draw = t(&mut || {
        let _ = h.draw_list();
    });
    let (w, ht) = (size.0 as u32 * 2, size.1 as u32 * 2);
    // One renderer for all frames, like a window: fonts and caches stay warm.
    let mut cpur = CpuRenderer::new(w, ht);
    let _ = cpur.render_to_image(&list, w, ht, 2.0);
    let cpu = t(&mut || {
        let _ = cpur.render_to_image(&list, w, ht, 2.0);
    });
    let gpu = GpuHeadlessRenderer::try_new(w, ht).ok().map(|mut g| {
        println!("adapter: {}", g.adapter_name());
        let _ = g.render_to_image(&list, w, ht, 2.0);
        t(&mut || {
            let _ = g.render_to_image(&list, w, ht, 2.0);
        })
    });
    println!("draw {draw:.0} us | cpu raster {cpu:.0} us | gpu {gpu:?} us");
}
