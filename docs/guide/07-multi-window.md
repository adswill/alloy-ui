# Multi-window

Windows share one state and one `App`. The first window shows `view`; every other window has a numeric id, opens with `Cmd::open_window(id, title, width, height)` and is drawn by `view_window(id)`.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default)]
struct Notes {
    text: String,
    palette_open: bool,
}

#[derive(Clone)]
enum Msg {
    Edit(String),
    TogglePalette,
}

impl App for Notes {
    type Msg = Msg;

    fn update_cmd(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Edit(t) => {
                self.text = t;
                Cmd::none()
            }
            Msg::TogglePalette => {
                self.palette_open = !self.palette_open;
                if self.palette_open {
                    Cmd::open_window(1, "Palette", 220.0, 120.0)
                } else {
                    Cmd::close_window(1)
                }
            }
        }
    }

    fn view(&self) -> impl View<Msg> {
        col![
            heading("Notes"),
            input(&self.text).on(Msg::Edit),
            button("Toggle palette").primary().on(Msg::TogglePalette),
        ]
        .gap(12.0)
        .pad(24.0)
    }

    fn view_window(&self, id: u64) -> impl View<Msg> {
        if id == 0 {
            return self.view().boxed();
        }
        col![
            heading("Palette"),
            text(format!("{} characters", self.text.chars().count())).dim()
        ]
        .gap(12.0)
        .pad(24.0)
        .boxed()
    }
}

fn main() {
    Notes::default().run_sized("Notes", 360.0, 200.0)
}
```

Each window has its own focus, scroll position and caret, and a message from any window updates the single state, after which every window is redrawn. Closing the last window exits the app. Window id `0` is always the first window.

`.boxed()` erases the view type so `view_window` can return different views from different branches.

The `sketch` API is single-window; use `App` when you need more than one.
