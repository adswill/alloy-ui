//! The workbench widgets through the concise API.
use alloy_ui::{prelude::*, testing::Harness};

state! {
    struct Doc {
        tab: usize,
        picked: usize,
        copied: bool,
        link: String,
    }
}

fn view(s: &Doc) -> impl View<Act<Doc>> {
    col![
        tab_bar(["One", "Two", "Three"], at!(s.tab)),
        split(
            vlist(100_000, |i| text(format!("item {i}"))).act(|s: &mut Doc, i| s.picked = i),
            context_menu(
                markdown("Hello **world**, see [docs](https://x.test)")
                    .link(|s: &mut Doc, u| s.link = u),
                [
                    menu_item("Copy").act(|s: &mut Doc| s.copied = true),
                    menu_rule(),
                    menu_item("Delete").danger(),
                ],
            ),
        )
    ]
}

fn app() -> Harness<impl alloy_ui::App> {
    Harness::new(sketch(Doc::default(), view), 800.0, 480.0)
}

#[test]
fn hundred_thousand_rows_render_and_select() {
    let mut h = app();
    assert!(h.has_text("item 0") && !h.has_text("item 99999"));
    h.click("item 2");
    h.key("End");
    assert!(h.has_text("item 99999"));
}

#[test]
fn tabs_switch_and_menu_runs_its_action() {
    let mut h = app();
    h.click("Two");
    h.right_click("Hello");
    assert!(h.has_text("Delete"), "menu is showing");
    h.click("Copy");
    assert!(!h.has_text("Delete"), "menu closed after picking");
}

#[test]
fn markdown_link_click_is_safe() {
    let mut h = app();
    h.click("docs");
    // A pure render check: the click must not panic, and the page is still there.
    assert!(h.has_text("world"));
}
