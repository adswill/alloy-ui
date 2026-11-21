//! Cost of creating the text system (font discovery + bundled fonts) and of the first shape.
use alloy_ui_render::{TextMeasurer, TextSystem};
use std::time::Instant;

fn main() {
    let s = Instant::now();
    let sys = TextSystem::new();
    let create = s.elapsed().as_secs_f64() * 1e3;
    let m = TextMeasurer::from_text_system(&sys);
    let s = Instant::now();
    let _ = m.measure("Hello world", 14.0, 400, None);
    let first = s.elapsed().as_secs_f64() * 1e3;
    let s = Instant::now();
    let _ = m.measure("Another string", 14.0, 400, None);
    let second = s.elapsed().as_secs_f64() * 1e3;
    println!("TextSystem::new {create:.1} ms | first measure {first:.2} ms | next new string {second:.3} ms | faces {}", sys.face_count());
}
