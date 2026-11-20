//! The concise API driven through the real runtime, headless.
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;

#[test]
fn closure_style_counter() {
    let app = sketch(0i64, |n| {
        row![
            text(n.to_string()),
            button("Add").act(|n| *n += 1),
            button("Reset").act(|n| *n = 0)
        ]
    });
    let mut ui = Harness::new(app, 300.0, 100.0);
    assert!(ui.has_text("0"));
    ui.click("Add").click("Add").click("Add");
    assert!(ui.has_text("3"));
    ui.click("Reset");
    assert!(ui.has_text("0"));
}

#[derive(Default)]
struct Greeter {
    name: String,
    loud: bool,
}

#[derive(Clone)]
enum Msg {
    Name(String),
    Loud(bool),
}

impl App for Greeter {
    type Msg = Msg;
    fn update(&mut self, m: Msg) {
        match m {
            Msg::Name(n) => self.name = n,
            Msg::Loud(l) => self.loud = l,
        }
    }
    fn view(&self) -> impl View<Msg> {
        let hello = format!("Hello {}", self.name);
        col![
            input(&self.name).hint("Your name").on(Msg::Name),
            check("Loud", self.loud).on(Msg::Loud),
            if self.loud {
                hello.to_uppercase()
            } else {
                hello
            },
        ]
    }
}

#[test]
fn elm_style_input_keeps_focus_across_rebuilds() {
    let mut ui = Harness::new(Greeter::default(), 300.0, 200.0);
    ui.click("Your name").type_text("Ada").type_text(" L");
    assert!(
        ui.has_text("Hello Ada L"),
        "automatic ids keep the field focused"
    );
    ui.click("Loud");
    assert!(ui.has_text("HELLO ADA L"));
}

#[test]
fn bind_writes_fields_and_none_is_empty() {
    struct S {
        on: bool,
        px: f32,
    }
    let app = sketch(
        S {
            on: false,
            px: 12.0,
        },
        |s: &S| {
            col![
                check("Enabled", s.on).bind(|s: &mut S| &mut s.on),
                s.on.then(|| text("now on")),
                text(format!("{}", s.px)),
            ]
        },
    );
    let mut ui = Harness::new(app, 300.0, 120.0);
    assert!(!ui.has_text("now on"));
    ui.click("Enabled");
    assert!(ui.has_text("now on"));
}

#[test]
fn png_is_produced() {
    let ui = Harness::new(sketch((), |_| "hi"), 100.0, 50.0);
    assert!(ui.png().starts_with(&[0x89, b'P', b'N', b'G']));
}
