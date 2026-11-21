//! Menu bar, tray icon, global hotkey, notifications and file dialogs, one line each.
use alloy_ui::prelude::*;

#[derive(Default)]
struct State {
    count: i64,
    picked: String,
}

fn main() {
    sketch(State::default(), |s| {
        col![
            heading("Desktop"),
            text(format!("{} so far", s.count)).dim(),
            text(if s.picked.is_empty() {
                "No file chosen"
            } else {
                &s.picked
            })
            .dim(),
            row![
                button("Add").primary().act(|s: &mut State| s.count += 1),
                button("Notify")
                    .act(|s: &mut State| notify("Counter", &format!("It is {}", s.count))),
                button("Open...").act(|s: &mut State| {
                    if let Some(p) = files::open()
                        .title("Pick a picture")
                        .filter("Images", &["png", "jpg"])
                        .pick()
                    {
                        s.picked = p.display().to_string();
                    }
                }),
            ]
            .gap(10.0),
        ]
        .gap(14.0)
        .pad(24.0)
        .fill()
    })
    .menu("File", |m| {
        m.item("Reset", "mod+r", |s: &mut State| s.count = 0)
            .separator()
            .item("Quit", "mod+q", |_| quit())
    })
    .tray("Desktop demo", |t| {
        t.click(|s: &mut State| s.count += 1)
            .item("Quit", |_| quit())
    })
    .global_hotkey("ctrl+alt+c", |s: &mut State| {
        s.count += 1;
        println!("count={}", s.count);
    })
    .run_sized("Desktop", 420.0, 220.0)
}
