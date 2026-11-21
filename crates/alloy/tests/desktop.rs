//! Menus, tray, global hotkeys: what the app asks the OS for, and what happens when the OS calls back.
use alloy_ui::{prelude::*, testing::Harness};

fn demo() -> impl App<Msg = Act<Vec<String>>> {
    sketch(
        Vec::<String>::new(),
        |log: &Vec<String>| -> Node<Act<Vec<String>>> {
            col![log.join(","), button("Noop").act(|_: &mut Vec<String>| {})].into()
        },
    )
    .menu("File", |m| {
        m.item("Open", "mod+o", |l| l.push("open".into()))
            .separator()
            .item("About", "", |l| l.push("about".into()))
            .sub("Export", |s| s.item("CSV", "", |l| l.push("csv".into())))
    })
    .tray("Demo", |t| {
        t.click(|l| l.push("tray-click".into()))
            .item("Show", |l| l.push("show".into()))
    })
    .global_hotkey("ctrl+alt+space", |l| l.push("hotkey".into()))
}

#[test]
fn spec_lists_menus_tray_and_hotkeys() {
    let h = Harness::new(demo(), 400.0, 300.0);
    let spec = h.desktop_spec();
    assert_eq!(spec.menus.len(), 1);
    assert_eq!(spec.menus[0].title, "File");
    assert_eq!(spec.menus[0].find("mod+o"), Some("Open"));
    assert_eq!(spec.menus[0].find("cmd:About"), Some("About"));
    assert_eq!(spec.menus[0].find("cmd:CSV"), Some("CSV"));
    assert_eq!(spec.hotkeys, ["ctrl+alt+space"]);
    let tray = spec.tray.expect("tray");
    assert_eq!(tray.tooltip, "Demo");
    assert!(tray.icon_valid());
    assert_eq!(
        tray.menu.expect("tray menu").find("tray:Show"),
        Some("Show")
    );
}

#[test]
fn the_os_calling_back_runs_the_action() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    h.desktop("cmd:About");
    assert!(h.has_text("about"));
    h.desktop("cmd:CSV")
        .desktop("tray:Show")
        .desktop("tray:click");
    h.desktop("global:ctrl+alt+space");
    assert!(h.has_text("about,csv,show,tray-click,hotkey"));
}

#[test]
fn menu_shortcut_works_in_the_window_too() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    h.chord("mod+o");
    assert!(h.has_text("open"));
    // And the OS reporting the same item by its shortcut id.
    h.desktop("mod+o");
    assert!(h.has_text("open,open"));
}

#[test]
fn menu_items_are_in_the_command_palette() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    h.chord("mod+k").type_text("About");
    assert!(h.has_text("About"));
}

#[test]
fn unknown_ids_do_nothing() {
    let mut h = Harness::new(demo(), 400.0, 300.0);
    h.desktop("cmd:Nope").desktop("tray:Nope");
    assert!(!h.has_text("about"));
}
