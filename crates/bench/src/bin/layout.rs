//! Layout engine cost: `compute` over a deep, wide tree of nested rows and
//! columns with a mix of Fill / Shrink / Fixed children. Prints the median of
//! 25 runs. `layout [depth] [fanout]`.
use std::time::Instant;

use alloy_ui_layout::{compute, Axis, Length, Node};

fn tree(depth: u32, fanout: u32, axis: Axis, seed: &mut u32) -> Node {
    if depth == 0 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let w = 10.0 + (*seed >> 24) as f32;
        return Node::leaf(w, 14.0);
    }
    let flip = match axis {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    };
    let kids = (0..fanout)
        .map(|i| {
            let n = tree(depth - 1, fanout, flip, seed);
            match i % 3 {
                0 => n.width(Length::Fill(1)),
                1 => n.height(Length::Fill(2)),
                _ => n,
            }
        })
        .collect();
    Node::container(axis, kids).spacing(4.0)
}

fn count(n: &Node) -> usize {
    1 + n.children.iter().map(count).sum::<usize>()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let depth: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(8);
    let fanout: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(3);
    let mut seed = 7;
    let root = tree(depth, fanout, Axis::Vertical, &mut seed)
        .width(Length::Fill(1))
        .height(Length::Fill(1));
    let avail = alloy_ui_core::Size::new(1200.0, 800.0);
    let mut sum = 0.0f32;
    let mut times: Vec<f64> = (0..25)
        .map(|_| {
            let s = Instant::now();
            let l = compute(&root, alloy_ui_core::Point::new(0.0, 0.0), avail);
            let e = s.elapsed().as_secs_f64() * 1e6;
            sum += l.bounds.size.width;
            e
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "nodes {} depth {depth}: compute {:.0} us (checksum {sum})",
        count(&root),
        times[times.len() / 2]
    );
}
