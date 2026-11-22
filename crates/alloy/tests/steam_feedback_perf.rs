//! Steam Oxide feedback, area P: layout/text-measure allocations and density.
use alloy_ui::{prelude::*, testing::Harness};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Counting = Counting;

#[derive(Default)]
struct Page {
    n: u32,
}

/// 60 rows of text, a caption, a spacer and a button: a Library-like page.
fn page(s: &Page) -> Node<Act<Page>> {
    let rows = (0..60).map(|i| {
        row![
            text(format!("Game number {i} with a longer title to wrap")),
            space(),
            caption(format!("{} hours", i * 3 + s.n as usize)),
            button("Play"),
        ]
        .gap(8.0)
    });
    scroll(col(rows).gap(4.0).pad(12.0)).into()
}

/// Allocations of one idle frame (a `Tick`: view, layout, draw), averaged over 10.
fn frame_allocs(h: &mut Harness<impl alloy_ui::App>) -> usize {
    h.advance(16);
    let before = ALLOCS.load(Relaxed);
    for _ in 0..10 {
        h.advance(16);
    }
    (ALLOCS.load(Relaxed) - before) / 10
}

fn draw_hash(h: &Harness<impl alloy_ui::App>) -> u64 {
    let mut s = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", h.draw_list()).hash(&mut s);
    s.finish()
}

#[test]
fn a_big_page_allocates_little_per_frame_and_draws_the_same() {
    let mut h = Harness::new(sketch(Page::default(), page), 800.0, 600.0);
    let per_frame = frame_allocs(&mut h);
    eprintln!("allocations per frame: {per_frame}");
    // Before the layout memo and the allocation-free text path this page took 3299 per frame
    // (1393 in layout, 1906 in draw, ~650 of it the view itself, which stays).
    assert!(per_frame < 1300, "{per_frame} allocations per frame");
    // Output is byte-identical to the pre-cache implementation (hash taken before the change).
    assert_eq!(draw_hash(&h), 0x4d3a5b29f88f08d1);
}

#[test]
fn a_cached_layout_equals_a_fresh_one() {
    let mut a = Harness::new(sketch(Page::default(), page), 800.0, 600.0);
    a.advance(16).advance(16);
    let warm = draw_hash(&a);
    // A different size misses the cache, going back hits it again: same pixels each time.
    a.resize(500.0, 400.0);
    let small = draw_hash(&a);
    assert_ne!(warm, small);
    a.resize(800.0, 600.0);
    assert_eq!(draw_hash(&a), warm);
    let b = Harness::new(sketch(Page::default(), page), 800.0, 600.0);
    assert_eq!(draw_hash(&b), warm);
}

#[test]
fn wrapped_text_measures_and_draws_as_before() {
    let long = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi";
    let mut h = Harness::new(
        sketch((), move |_: &()| {
            col![text(long).fill_w(), text("x  y")].w(120.0)
        }),
        300.0,
        200.0,
    );
    h.advance(16);
    let lines: Vec<String> = h
        .draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            alloy_ui_core::DrawCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();
    assert!(lines.len() >= 4, "wrapped into several lines: {lines:?}");
    assert_eq!(lines.join(" ").split_whitespace().count(), 16);
    assert!(lines.iter().all(|l| l == l.trim_end()));
}

#[derive(Default)]
struct Dense {
    seen: Rc<Cell<Option<alloy_ui::theme::Density>>>,
}

fn dense_app(
    d: Option<alloy_ui::theme::Density>,
    seen: Rc<Cell<Option<alloy_ui::theme::Density>>>,
) -> impl alloy_ui::App {
    let s = sketch(Dense { seen }, |s: &Dense| {
        s.seen.set(Some(density()));
        col![button("Go"), text("hello")].pad(density().px(20.0))
    });
    match d {
        Some(d) => s.density(d),
        None => s,
    }
}

fn go_size(d: Option<alloy_ui::theme::Density>) -> (f32, alloy_ui::theme::Density) {
    use alloy_ui::theme::Density;
    let seen = Rc::new(Cell::new(None));
    let h = Harness::new(dense_app(d, seen.clone()), 400.0, 300.0);
    let p = h.position("Go").expect("button shown");
    (p.y, seen.get().unwrap_or(Density::Comfortable))
}

#[test]
fn density_scales_spacing_from_one_switch_and_can_be_queried() {
    use alloy_ui::theme::Density::*;
    let (default_y, d0) = go_size(None);
    let (comfortable_y, d1) = go_size(Some(Comfortable));
    let (compact_y, d2) = go_size(Some(Compact));
    let (spacious_y, d3) = go_size(Some(Spacious));
    assert_eq!(
        (d0, d1, d2, d3),
        (Comfortable, Comfortable, Compact, Spacious)
    );
    assert_eq!(default_y, comfortable_y);
    assert!(compact_y < comfortable_y, "{compact_y} < {comfortable_y}");
    assert!(spacious_y > comfortable_y, "{spacious_y} > {comfortable_y}");
}

#[test]
fn sketch_density_wins_over_a_look_and_scales_tree_rows() {
    use alloy_ui::theme::Density::*;
    let tree_h = |d| {
        let app = sketch((), |_: &()| {
            set_look(Look::dark().density(Spacious));
            tree(vec![TreeItem::new(1, "one"), TreeItem::new(2, "two")])
        })
        .density(d);
        let h = Harness::new(app, 300.0, 300.0);
        h.position("two").unwrap().y - h.position("one").unwrap().y
    };
    let (c, n) = (tree_h(Compact), tree_h(Comfortable));
    assert!(c < n, "compact rows {c} < comfortable rows {n}");
}
