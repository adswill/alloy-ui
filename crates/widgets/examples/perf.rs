//! Rough timing of layout + draw for a large list. Run with
//! `cargo run --release -p alloy-ui-widgets --example perf`.
use alloy_ui_core::*;
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::time::Instant;

fn main() {
    let ui = Ui::new(Theme::light());
    for n in [100usize, 1000, 5000] {
        let items = (0..n)
            .map(|i| Text::new(&ui, format!("Row {i}")).into_element::<()>())
            .collect();
        let root = Root::new(
            &ui,
            List::new(&ui, 1, items, Some(3), |_| ()).into_element(),
        )
        .into_element();
        let t = Instant::now();
        let layout = alloy_ui_widgets::layout(&root, Size::new(600.0, 800.0));
        let lay = t.elapsed();
        let t = Instant::now();
        let mut list = DrawList::default();
        root.0.draw(&layout, &mut list);
        println!(
            "{n:>5} rows: layout {:>7.2?}  draw {:>7.2?}  ({} commands)",
            lay,
            t.elapsed(),
            list.0.len()
        );
    }
}
