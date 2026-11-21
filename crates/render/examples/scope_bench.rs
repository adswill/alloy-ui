//! Cost of one spectrum + waterfall frame on this machine's GPU (or its software adapter):
//! `cargo run --release -p alloy-ui-render --example scope_bench`.
//! Each iteration writes one waterfall row, then renders the frame to an image. The read-back
//! is part of the time, so this is an upper bound for a window, where nothing is read back.
use alloy_ui_core::{Color, DrawCommand, DrawList, Point, Rectangle, Size};
use alloy_ui_render::{GpuHeadlessRenderer, HeadlessRenderer, StreamTexture};
use std::time::Instant;

fn main() {
    let (w, h) = (1280u32, 760u32);
    let mut r = match GpuHeadlessRenderer::try_new(w, h) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("BENCH no GPU adapter: {e}");
            return;
        }
    };
    eprintln!("BENCH adapter {}", r.adapter_name());
    let tex = StreamTexture::new(8192, 1024);
    let row: Vec<u8> = (0..8192 * 4).map(|i| (i * 7 % 251) as u8).collect();
    let rect = |x, y, w, h| Rectangle::new(Point::new(x, y), Size::new(w, h));
    let trace: Vec<Point> = (0..2560)
        .map(|i| Point::new(i as f32 / 2.0, 120.0 + ((i * 37) % 61) as f32 - 30.0))
        .collect();
    let mut head = 0u32;
    let mut times = Vec::new();
    for frame in 0..320 {
        tex.write(0, head, 8192, 1, &row);
        head = (head + 1) % 1024;
        let split = head as f32 / 1024.0;
        let top = 420.0 * split;
        let mut list = DrawList::default();
        list.push(DrawCommand::Polyline {
            points: trace.clone(),
            width: 1.5,
            color: Color::hex(0x0E7C6B),
        });
        list.push(DrawCommand::StreamImage {
            bounds: rect(0.0, 240.0, 1280.0, top),
            id: tex.id(),
            uv: [0.0, split, 1.0, 0.0],
        });
        list.push(DrawCommand::StreamImage {
            bounds: rect(0.0, 240.0 + top, 1280.0, 420.0 - top),
            id: tex.id(),
            uv: [0.0, 1.0, 1.0, split],
        });
        let t = Instant::now();
        let _ = r.render_to_image(&list, w, h, 1.0);
        if frame >= 20 {
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = times.len();
    let mean = times.iter().sum::<f64>() / n as f64;
    eprintln!(
        "BENCH frames={n} mean_ms={mean:.2} p50_ms={:.2} p95_ms={:.2} max_ms={:.2} fps_equiv={:.0}",
        times[n / 2],
        times[n * 95 / 100],
        times[n - 1],
        1000.0 / mean
    );
}
