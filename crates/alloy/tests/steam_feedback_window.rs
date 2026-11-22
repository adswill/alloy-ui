//! Steam feedback, area W: hide/show the window, a minimum size, the application menu.
use alloy_ui::platform::menu::MenuItem;
use alloy_ui::{prelude::*, testing::Harness, window_hidden, DesktopSpec};

#[derive(Default)]
struct S {
    n: i64,
}

fn find<'a>(items: &'a [MenuItem], label: &str) -> Option<&'a MenuItem> {
    items.iter().find_map(|i| match i {
        MenuItem::Action { label: l, .. } if l == label => Some(i),
        MenuItem::Submenu(m) => find(&m.items, label),
        _ => None,
    })
}

#[test]
fn close_to_tray_hides_the_window_and_the_tray_brings_it_back() {
    show_window();
    let app = sketch(S::default(), |_: &S| text("running"))
        .tray("x", |t| t.item("Show", |_| show_window()))
        .on_close_requested(|cx| cx.hide());
    let mut h = Harness::new(app, 200.0, 100.0);
    assert!(!h.close(), "the window stays open (hidden)");
    assert!(window_hidden(), "the close button hid it");
    h.desktop("tray:Show");
    assert!(!window_hidden(), "the tray item showed it again");
}

#[test]
fn hide_window_works_from_any_handler() {
    show_window();
    let mut h = Harness::new(
        sketch(S::default(), |_: &S| {
            button("Hide").act(|_: &mut S| hide_window())
        }),
        200.0,
        100.0,
    );
    h.click("Hide");
    assert!(window_hidden());
    show_window();
    assert!(!window_hidden());
}

#[test]
fn harness_resize_clamps_to_the_minimum_size() {
    let app = sketch(S::default(), |_: &S| text("x")).min_window_size(400.0, 300.0);
    let mut h = Harness::new(app, 200.0, 100.0);
    assert_eq!(h.size(), (400.0, 300.0), "the first size is clamped too");
    h.resize(100.0, 900.0);
    assert_eq!(h.size(), (400.0, 900.0));
}

#[test]
fn item_enabled_greys_an_item_and_drops_its_shortcut() {
    let app = sketch(S::default(), |s: &S| text(format!("n={}", s.n))).menu("File", |m| {
        m.item("Open", "mod+o", |s| s.n += 1)
            .item_enabled("Save", "mod+s", false, |s| s.n += 10)
    });
    let mut h = Harness::new(app, 200.0, 100.0);
    let spec: DesktopSpec = h.desktop_spec();
    let file = &spec.menus[0];
    assert!(matches!(
        find(&file.items, "Open"),
        Some(MenuItem::Action { enabled: true, .. })
    ));
    assert!(matches!(
        find(&file.items, "Save"),
        Some(MenuItem::Action { enabled: false, .. })
    ));
    h.chord("mod+s");
    h.chord("mod+o");
    h.expect("n=1");
}

#[test]
fn app_menu_and_standard_edit_items() {
    let app = sketch(S::default(), |s: &S| text(format!("n={}", s.n)))
        .app_menu(|m| {
            m.about()
                .item("Settings...", "mod+,", |s| s.n = 7)
                .separator()
                .hide()
                .hide_others()
                .show_all()
                .separator()
                .quit()
        })
        .menu("Edit", |m| m.cut().copy().paste().select_all())
        .menu("View", |m| m.item("Find", "mod+ctrl+f", |s| s.n = 9));
    let mut h = Harness::new(app, 200.0, 100.0);
    let spec = h.desktop_spec();
    let app_menu = spec.app_menu.expect("an application menu");
    for l in [
        "About",
        "Settings...",
        "Hide",
        "Hide Others",
        "Show All",
        "Quit",
    ] {
        assert!(find(&app_menu.items, l).is_some(), "{l} is in the app menu");
    }
    for (l, id) in [
        ("Cut", "std:cut"),
        ("Copy", "std:copy"),
        ("Paste", "std:paste"),
        ("Select All", "std:select_all"),
    ] {
        assert!(
            matches!(find(&spec.menus[0].items, l), Some(MenuItem::Action { id: i, .. }) if i == id)
        );
    }
    // A full chord survives to the shell, which hands it to AppKit with every modifier.
    let find_item = find(&spec.menus[1].items, "Find").unwrap();
    assert!(matches!(find_item, MenuItem::Action { shortcut: Some(s), .. } if s == "mod+ctrl+f"));
    let c = alloy_ui::platform::menu::chord("mod+ctrl+f").unwrap();
    assert!(c.ctrl && c.key == "f");
    h.chord("mod+,");
    h.expect("n=7");
    h.chord("mod+ctrl+f");
    h.expect("n=9");
}
