//! Two windows from one state: the main window opens and closes a palette window.
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
        // The first window keeps `view`; the palette shows a live character count.
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
