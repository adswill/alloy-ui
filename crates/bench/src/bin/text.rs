//! Text-path micro benchmark: measuring and laying out strings repeatedly,
//! as a no-op event (re-layout) and every frame do.
use std::time::Instant;

use alloy_ui_core::FontFamily;
use alloy_ui_render::{TextMeasurer, TextSystem};

fn main() {
    let mut sys = TextSystem::new();
    let m: TextMeasurer = sys.measurer();
    let strings: Vec<String> = (0..60)
        .map(|i| format!("Row {i}: some label text"))
        .collect();
    // Warm fonts.
    for s in &strings {
        m.measure(s, 14.0, 400, None);
    }
    let n = 2000;
    let t = Instant::now();
    for _ in 0..n {
        for s in &strings {
            std::hint::black_box(m.measure(s, 14.0, 400, None));
        }
    }
    let per = t.elapsed().as_secs_f64() * 1e9 / (n * strings.len()) as f64;
    println!("measure: {per:.0} ns per string");
    let t = Instant::now();
    for _ in 0..n {
        for (i, s) in strings.iter().enumerate() {
            std::hint::black_box(sys.layout(
                s,
                28.0,
                400,
                FontFamily::Sans,
                None,
                None,
                (10.0, i as f32 * 30.0),
            ));
        }
    }
    let per = t.elapsed().as_secs_f64() * 1e9 / (n * strings.len()) as f64;
    println!("layout: {per:.0} ns per string");
}
