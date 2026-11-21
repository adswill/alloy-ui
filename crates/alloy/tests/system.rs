//! Drag and drop, dropped files, deep links, rich clipboard and system settings, driven
//! through the real runtime.
use alloy_ui::{prelude::*, testing::Harness};
use std::path::PathBuf;

#[derive(Default, Clone, PartialEq, Debug)]
struct Board {
    todo: Vec<u32>,
    done: Vec<u32>,
    files: Vec<PathBuf>,
    opened: Vec<String>,
    hovering_seen: Vec<bool>,
}

fn card(id: u32) -> Node<Act<Board>> {
    text(format!("Card {id}")).pad(8.0).card().drag(id)
}

fn board() -> impl App<Msg = Act<Board>> {
    sketch(
        Board {
            todo: vec![1, 2],
            done: vec![9],
            ..Board::default()
        },
        |b: &Board| -> Node<Act<Board>> {
            let todo: Vec<Node<Act<Board>>> = b.todo.iter().map(|&i| card(i)).collect();
            let done: Vec<Node<Act<Board>>> = b.done.iter().map(|&i| card(i)).collect();
            let hint: Node<Act<Board>> = text(if files_hovering() { "DROP NOW" } else { "idle" }).into();
            row![
                col![heading("To do"), col(todo).pad(16.0).w(160.0).h(240.0)]
                    .drop_act(|b: &mut Board, id: u32| {
                        b.done.retain(|&d| d != id);
                        if !b.todo.contains(&id) {
                            b.todo.push(id);
                        }
                    }),
                col![heading("Done"), col(done).pad(16.0).w(160.0).h(240.0)]
                    .drop_act(|b: &mut Board, id: u32| {
                        b.todo.retain(|&d| d != id);
                        if !b.done.contains(&id) {
                            b.done.push(id);
                        }
                    }),
                col![
                    hint,
                    text(format!("files:{}", b.files.len())),
                    text(format!("opened:{}", b.opened.join(","))),
                ],
            ]
            .gap(24.0)
            .into()
        },
    )
    .on_drop(|b, files| b.files = files)
    .on_open_url(|b, u| b.opened.push(u))
    .on_open_files(|b, f| b.files.extend(f))
}

#[test]
fn drag_card_between_columns() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    assert!(h.has_text("Card 1"));
    // Drop on the other column's heading: inside its drop zone.
    h.drag("Card 1", "Done");
    h.advance(20).advance(400);
    // The card is now listed after the 9 in the Done column: its position moved right.
    let p1 = h.position("Card 1").unwrap();
    let p9 = h.position("Card 9").unwrap();
    assert!(p1.x > 100.0 && (p1.x - p9.x).abs() < 1.0, "Card 1 joined the Done column: {p1:?} vs {p9:?}");
    // And back again.
    h.drag("Card 1", "To do");
    h.advance(20).advance(400);
    let p1 = h.position("Card 1").unwrap();
    let p2 = h.position("Card 2").unwrap();
    assert!((p1.x - p2.x).abs() < 1.0, "Card 1 is back with Card 2");
}

#[test]
fn drop_outside_a_target_changes_nothing() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    let before = h.position("Card 2").unwrap();
    // The hint text is in no drop zone.
    h.drag("Card 2", "idle");
    h.advance(20).advance(400);
    assert_eq!(h.position("Card 2").unwrap(), before);
}

#[test]
fn escape_cancels_a_drag() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    let before = h.position("Card 1").unwrap();
    h.drag_hold("Card 1", "Done");
    h.key("Escape");
    h.release();
    h.advance(20).advance(400);
    assert_eq!(h.position("Card 1").unwrap(), before);
}

#[test]
fn ghost_follows_pointer() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    let n_idle = h.draw_list().0.len();
    h.drag_hold("Card 1", "Done");
    let list = h.draw_list();
    // The ghost adds a second "Card 1" label, drawn near the Done column.
    let labels: Vec<_> = list
        .0
        .iter()
        .filter_map(|c| match c {
            alloy_ui::DrawCommand::Text { content, origin, .. } if content == "Card 1" => Some(*origin),
            _ => None,
        })
        .collect();
    assert_eq!(labels.len(), 2, "original (dimmed) plus ghost");
    assert!(labels[1].x > labels[0].x + 100.0, "ghost sits over the Done column");
    assert!(list.0.len() > n_idle);
    h.advance(16).advance(300);
    if std::env::var_os("SYSTEM_PNG").is_some() {
        h.save_png("/tmp/dnd_drag.png");
    }
    h.release();
}

#[test]
fn files_dropped_and_hover_hint() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    assert!(h.has_text("idle"));
    h.hover_files(true);
    assert!(h.has_text("DROP NOW"));
    h.drop_files(["/tmp/a.txt", "/tmp/b.png"]);
    assert!(h.has_text("idle"), "hint clears on drop");
    assert!(h.has_text("files:2"));
}

#[test]
fn deep_links_and_open_files() {
    let mut h = Harness::new(board(), 640.0, 360.0);
    h.open_url("myapp://invite/42");
    assert!(h.has_text("opened:myapp://invite/42"));
    h.open_files(["/tmp/x.md"]);
    assert!(h.has_text("files:1"));
}

#[test]
fn host_wishes_come_from_the_sketch() {
    use alloy_ui::WindowEffect;
    let plain = board();
    assert_eq!(App::window_effect(&plain), WindowEffect::None);
    assert!(!App::system_accent(&plain));
    assert_eq!(App::single_instance(&plain), None);
    let a = sketch(0, |_: &i32| -> Node<Act<i32>> { col!["x"].into() })
        .window_effect(WindowEffect::Mica)
        .system_accent()
        .single_instance("dev.example.x")
        .url_scheme("x");
    // The builder methods share their names with the `App` hooks the shell reads.
    assert_eq!(App::window_effect(&a), WindowEffect::Mica);
    assert!(App::system_accent(&a));
    assert_eq!(App::single_instance(&a).as_deref(), Some("dev.example.x"));
    assert_eq!(App::url_schemes(&a), vec!["x".to_string()]);
}
