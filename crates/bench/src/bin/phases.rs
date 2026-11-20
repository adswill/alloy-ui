//! Per-phase timing of one text_editor frame (pointer-move style redraw):
//! draw-list build, CPU raster, GPU offscreen. Prints microseconds.
use std::time::Instant;

use alloy_ui_core::{Event, Point};
use alloy_ui_examples::apps::{dashboard, text_editor};
use alloy_ui_examples::run::{apply, runtime};
use alloy_ui_render::{CpuRenderer, GpuHeadlessRenderer, HeadlessRenderer};

fn med(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("dashboard") => run(dashboard::demo()),
        _ => run(text_editor::demo()),
    }
}

fn run<A: alloy_ui_core::Application>(demo: alloy_ui_examples::run::Demo<A>) {
    let size = demo.size;
    let mut rt = runtime(demo.app, size);
    for s in &demo.steps {
        apply(&mut rt, s);
    }
    let t = |f: &mut dyn FnMut()| {
        med((0..15)
            .map(|_| {
                let s = Instant::now();
                f();
                s.elapsed().as_secs_f64() * 1e6
            })
            .collect())
    };
    let mut i = 0;
    let ev = t(&mut || {
        i += 1;
        rt.handle_event(&Event::CursorMoved(Point::new(
            100.0 + (i % 50) as f32,
            100.0,
        )));
    });
    let list = rt.draw();
    println!("commands: {}", list.0.len());
    let draw = t(&mut || {
        let _ = rt.draw();
    });
    let (w, h) = (size.0 as u32 * 2, size.1 as u32 * 2);
    // One renderer for all frames, like a window: fonts and caches stay warm.
    let mut cpur = CpuRenderer::new(w, h);
    let _ = cpur.render_to_image(&list, w, h, 2.0);
    let cpu = t(&mut || {
        let _ = cpur.render_to_image(&list, w, h, 2.0);
    });
    let gpu = GpuHeadlessRenderer::try_new(w, h).ok().map(|mut g| {
        println!("adapter: {}", g.adapter_name());
        let _ = g.render_to_image(&list, w, h, 2.0);
        t(&mut || {
            let _ = g.render_to_image(&list, w, h, 2.0);
        })
    });
    println!("event {ev:.0} us | draw {draw:.0} us | cpu raster {cpu:.0} us | gpu {gpu:?} us");
}
