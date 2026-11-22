//! `App::on_close` and `App::on_focus`, driven through the headless harness.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Clone)]
enum Msg {
    Ask,
    Focus(bool),
    Discard,
}

#[derive(Default)]
struct Doc {
    dirty: bool,
    asking: bool,
    focused: bool,
}

impl App for Doc {
    type Msg = Msg;
    fn update(&mut self, m: Msg) {
        match m {
            Msg::Ask => self.asking = true,
            Msg::Focus(f) => self.focused = f,
            Msg::Discard => self.dirty = false,
        }
    }
    fn on_close(&self) -> Option<Msg> {
        self.dirty.then_some(Msg::Ask)
    }
    fn on_focus(&self, focused: bool) -> Option<Msg> {
        Some(Msg::Focus(focused))
    }
    fn view(&self) -> impl View<Msg> {
        col![
            text(if self.asking {
                "Save changes?"
            } else {
                "editing"
            }),
            text(if self.focused { "focused" } else { "blurred" }),
            button("Discard").on(Msg::Discard),
        ]
    }
}

#[test]
fn a_clean_document_closes_and_a_dirty_one_asks_first() {
    let mut ui = Harness::new(Doc::default(), 300.0, 160.0);
    assert!(ui.close(), "nothing to save: the window may close");
    let mut ui = Harness::new(
        Doc {
            dirty: true,
            ..Doc::default()
        },
        300.0,
        160.0,
    );
    assert!(!ui.close(), "unsaved work vetoes the close");
    ui.expect("Save changes?").click("Discard");
    assert!(ui.close(), "closing works once the work is discarded");
}

#[test]
fn focus_changes_reach_the_app() {
    let mut ui = Harness::new(Doc::default(), 300.0, 160.0);
    ui.expect("blurred")
        .focus(true)
        .expect("focused")
        .focus(false)
        .expect("blurred");
}
