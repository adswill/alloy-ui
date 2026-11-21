//! Frame cost of a large view: a scrolling checklist with N rows (default 10000).
//! Prints microseconds per phase. `large [rows]`.
use std::time::Instant;

use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;

struct Big {
    rows: Vec<(String, bool)>,
}

fn view(s: &Big) -> impl View<Act<Big>> {
    col![
        heading("Big list"),
        scroll(
            col(s
                .rows
                .iter()
                .enumerate()
                .map(|(i, t)| check(&t.0, t.1).act(move |s: &mut Big, v| s.rows[i].1 = v)))
            .gap(10.0)
        ),
    ]
    .gap(12.0)
    .pad(24.0)
    .fill()
}

fn med(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(10_000);
    let rows = (0..n)
        .map(|i| (format!("Task number {i}: write the thing"), i % 3 == 0))
        .collect();
    let s = Instant::now();
    let mut h = Harness::new(sketch(Big { rows }, view), 440.0, 470.0);
    h.settle();
    let first = s.elapsed().as_secs_f64() * 1e3;
    let t = |f: &mut dyn FnMut()| {
        med((0..9)
            .map(|_| {
                let s = Instant::now();
                f();
                s.elapsed().as_secs_f64() * 1e6
            })
            .collect())
    };
    let draw = t(&mut || {
        let _ = h.draw_list();
    });
    let tick = t(&mut || {
        h.advance(16);
    });
    println!(
        "rows {n}: build+settle {first:.1} ms | draw {draw:.0} us | tick(event+view+layout) {tick:.0} us | commands {}",
        h.draw_list().0.len()
    );
}
