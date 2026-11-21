//! Animation / images: image opacity, keyed scope, appear, exit, vlist rows.
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::DrawCommand;

fn alpha_of(h: &Harness<impl App>, what: &str) -> Option<f32> {
    h.draw_list().0.iter().find_map(|c| match c {
        DrawCommand::Text { content, color, .. } if content == what => Some(color.a),
        _ => None,
    })
}

#[test]
fn vlist_rows_with_enter_become_visible() {
    let mut h = Harness::new(
        sketch((), |_: &()| {
            let l: Node<Act<()>> = vlist(100, |i| stagger([text(format!("Row {i}"))]))
                .row_height(24.0)
                .into();
            l
        }),
        300.0,
        200.0,
    );
    for _ in 0..40 {
        h.advance(16);
    }
    let a = alpha_of(&h, "Row 0").expect("row 0 drawn");
    assert!(a > 0.99, "row fully visible, alpha {a}");
    assert!(!h.animating(), "settled");
    h.scroll("Row 0", 96.0);
    for _ in 0..40 {
        h.advance(16);
    }
    let a = alpha_of(&h, "Row 6").expect("row 6 drawn");
    assert!(a > 0.99, "scrolled-in row visible, alpha {a}");
}

fn image_alpha(h: &Harness<impl App>) -> Option<f32> {
    h.draw_list().0.iter().find_map(|c| match c {
        DrawCommand::ImageAlpha { alpha, .. } => Some(*alpha),
        DrawCommand::Image { .. } => Some(1.0),
        _ => None,
    })
}

#[derive(Default)]
struct Show(bool);

#[test]
fn image_opacity_and_fade_apply_to_pictures() {
    let mut h = Harness::new(
        sketch(Show(false), |s: &Show| {
            col![
                image(7, 20.0, 20.0).opacity(0.4),
                fade(s.0, image(8, 20.0, 20.0)),
                button("go").act(|s: &mut Show| s.0 = true),
            ]
        }),
        200.0,
        200.0,
    );
    h.advance(16);
    let alphas: Vec<(u64, f32)> = h
        .draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::ImageAlpha { id, alpha, .. } => Some((*id, *alpha)),
            DrawCommand::Image { id, .. } => Some((*id, 1.0)),
            _ => None,
        })
        .collect();
    assert!(alphas.contains(&(7, 0.4)), "{alphas:?}");
    assert!(
        !alphas.iter().any(|(id, _)| *id == 8),
        "hidden image not drawn"
    );
    h.click("go");
    h.advance(60);
    let mid = h
        .draw_list()
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::ImageAlpha { id: 8, alpha, .. } => Some(*alpha),
            _ => None,
        })
        .expect("image 8 fading in");
    assert!(mid > 0.0 && mid < 1.0, "mid-fade alpha {mid}");
    for _ in 0..30 {
        h.advance(16);
    }
    assert!(h
        .draw_list()
        .0
        .iter()
        .any(|c| matches!(c, DrawCommand::Image { id: 8, .. })));
    assert!(!h.animating());
    let _ = image_alpha(&h);
}

#[derive(Default)]
struct Rows {
    ids: Vec<u32>,
}

#[test]
fn keyed_state_follows_the_item_not_the_position() {
    let mut h = Harness::new(
        sketch(Rows { ids: vec![1, 2, 3] }, |s: &Rows| {
            let mut rows: Vec<Node<Act<Rows>>> = s
                .ids
                .iter()
                .map(|&id| {
                    keyed(id, || {
                        // Each item remembers its own tag the first time it is built.
                        let tag = local(format!("tag{id}"));
                        text(tag.get()).into()
                    })
                })
                .collect();
            rows.push(button("reverse").act(|s: &mut Rows| s.ids.reverse()).into());
            col(rows)
        }),
        200.0,
        200.0,
    );
    h.advance(16);
    assert!(h.has_text("tag1") && h.has_text("tag3"));
    h.click("reverse");
    h.advance(16);
    // Same texts, in reversed order: state travelled with the ids.
    let y = |t: &str| h.position(t).unwrap().y;
    assert!(y("tag3") < y("tag2") && y("tag2") < y("tag1"));
    assert!(h.has_text("tag1") && h.has_text("tag2") && h.has_text("tag3"));
}

#[test]
fn appear_fades_in_on_first_build() {
    let mut h = Harness::new(
        sketch((), |_: &()| fade_appear(true, text("hello"))),
        200.0,
        100.0,
    );
    h.advance(16);
    h.advance(16);
    let a = alpha_of(&h, "hello").expect("drawn while appearing");
    assert!(a < 0.99, "starts translucent, alpha {a}");
    for _ in 0..40 {
        h.advance(16);
    }
    assert!(alpha_of(&h, "hello").unwrap() > 0.99);
    assert!(!h.animating());
}

#[test]
fn plain_fade_with_show_true_does_not_animate() {
    let mut h = Harness::new(sketch((), |_: &()| fade(true, text("hi"))), 200.0, 100.0);
    h.advance(16);
    assert!(alpha_of(&h, "hi").unwrap() > 0.99);
}

#[derive(Default)]
struct Todos(Vec<(u32, String)>);

fn todos() -> Harness<impl App> {
    Harness::new(
        sketch(
            Todos(vec![
                (1, "alpha".into()),
                (2, "beta".into()),
                (3, "gamma".into()),
            ]),
            |s: &Todos| {
                col![
                    presence_list(s.0.clone(), |t| t.0, |t| text(t.1.clone())),
                    button("drop").act(|s: &mut Todos| {
                        s.0.remove(1);
                    }),
                    button("add").act(|s: &mut Todos| s.0.push((9, "delta".into()))),
                ]
            },
        ),
        300.0,
        300.0,
    )
}

#[test]
fn removed_rows_exit_then_unmount() {
    let mut h = todos();
    h.advance(16);
    assert_eq!(h.count("beta"), 1);
    assert!(
        alpha_of(&h, "beta").unwrap() > 0.99,
        "first build is not animated"
    );
    h.click("drop");
    h.advance(16);
    h.advance(60);
    let a = alpha_of(&h, "beta").expect("still mounted while it exits");
    assert!(a < 0.99, "fading out, alpha {a}");
    assert!(h.animating());
    for _ in 0..60 {
        h.advance(16);
    }
    assert_eq!(h.count("beta"), 0, "gone once the exit finished");
    assert!(h.has_text("alpha") && h.has_text("gamma"));
    assert!(!h.animating(), "idle, nothing stuck");
}

#[test]
fn added_rows_ease_in() {
    let mut h = todos();
    h.advance(16);
    h.click("add");
    h.advance(32);
    let a = alpha_of(&h, "delta").expect("drawn");
    assert!(a < 0.99, "entering, alpha {a}");
    for _ in 0..40 {
        h.advance(16);
    }
    assert!(alpha_of(&h, "delta").unwrap() > 0.99);
}

#[test]
fn removed_rows_vanish_at_once_under_reduced_motion() {
    let mut h = todos();
    h.reduce_motion(true);
    h.advance(16);
    h.click("drop");
    h.advance(16);
    assert_eq!(h.count("beta"), 0);
    assert!(!h.animating());
}
