//! Motion III: gestures and shared-element routes.
//!
//! * Gestures: drag the list (it stretches past the ends and coasts when let go), and drag the
//!   sheet's handle (it follows you, resists past the ends and settles on a snap point).
//! * Routes: open a card; its cover flies to the detail screen as the pages slide.
//! * Weight: a label whose weight animates.
//!
//! `--headless x.png --click Routes --click "Aurora" --advance 140` captures mid-flight.
use alloy_ui::prelude::*;

#[derive(Default)]
struct S {
    page: usize,
    bold: usize,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Grid,
    Card(usize),
}

type V = Node<Act<S>>;

const CARDS: [(&str, u32, u32); 4] = [
    ("Aurora", 0x5B3FE0, 0x2F9BFF),
    ("Ember", 0xC2410C, 0xFF8A4C),
    ("Signal", 0x2F7A1F, 0xA3E635),
    ("Glacier", 0x0B78A8, 0x4CC9F0),
];

fn main() {
    sketch(S::default(), |s| {
        let page: V = match s.page {
            0 => gestures(),
            1 => routes(),
            _ => weight(s),
        };
        col![
            row![
                heading("Motion III"),
                space(),
                tabs(["Gestures", "Routes", "Weight"], at!(s.page)),
            ]
            .align_center(),
            page,
        ]
        .gap(16.0)
        .page()
    })
    .run_sized("Motion III", 720.0, 640.0)
}

fn gestures() -> V {
    let rows: Vec<V> = (1..=24)
        .map(|i| {
            row![
                text(format!("{i:02}")).dim(),
                col![text(format!("Contact {i}")), caption("Last seen just now")].gap(2.0),
            ]
            .gap(14.0)
            .pad(10.0)
            .into()
        })
        .collect();
    let page: V = col![
        text("Drag the list: it stretches past the ends and keeps coasting.").dim(),
        scroll_touch(col(rows).gap(4.0)).h(420.0),
    ]
    .gap(8.0)
    .fill()
    .into();
    let panel: V = col![
        heading("Quick actions"),
        caption("Drag the handle, or flick it up and down."),
        row![
            button("Share").primary().act(|_: &mut S| {}),
            button("Archive").act(|_: &mut S| {}),
        ]
        .gap(8.0),
    ]
    .gap(8.0)
    .into();
    stack([
        Layer::from(page),
        Layer::from(sheet(&[0.17, 0.42, 0.82], panel)),
    ])
}

fn cover(i: usize, w: f32, h: f32) -> V {
    let (name, a, b) = CARDS[i];
    hero(
        ("cover", i),
        col![text(name).label()]
            .pad(12.0)
            .w(w)
            .h(h)
            .gradient(Color::hex(a), Color::hex(b), 135.0)
            .round(12.0),
    )
}

fn routes() -> V {
    let nav = router(Screen::Grid);
    nav.show_with(RouteKind::SlideFade, move |r| match r {
        Screen::Grid => col![
            text("Open a card; its cover flies to the next screen.").dim(),
            row((0..4).map(|i| -> V {
                col![
                    cover(i, 130.0, 90.0),
                    button("Open")
                        .ghost()
                        .act(move |_: &mut S| nav.push(Screen::Card(i))),
                ]
                .gap(6.0)
                .into()
            }))
            .gap(14.0),
        ]
        .gap(12.0)
        .into(),
        Screen::Card(i) => col![
            nav.back_button(),
            cover(i, 380.0, 230.0),
            heading(CARDS[i].0),
            caption("The same element, scaled and moved between screens."),
        ]
        .gap(12.0)
        .into(),
    })
}

fn weight(s: &S) -> V {
    let names = ["Overview", "Activity", "Billing", "Members"];
    col![
        text("Select one: the label firms up from regular to bold.").dim(),
        row(names.iter().enumerate().map(|(i, n)| -> V {
            let w = if i == s.bold { 700 } else { 400 };
            col![
                weighted(*n, w),
                button("Select").ghost().act(move |s: &mut S| s.bold = i),
            ]
            .gap(6.0)
            .pad_xy(14.0, 10.0)
            .surface()
            .round(8.0)
            .into()
        }))
        .gap(10.0),
    ]
    .gap(12.0)
    .into()
}
