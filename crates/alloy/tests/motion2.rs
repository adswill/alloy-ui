//! Behaviour of the second-wave motion helpers through the real runtime.
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::DrawCommand;

fn texts_with_alpha(h: &Harness<impl App>) -> Vec<(String, f32)> {
    h.draw_list()
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Text { content, color, .. } => Some((content.clone(), color.a)),
            _ => None,
        })
        .collect()
}

#[derive(Default)]
struct P {
    page: usize,
}

fn pages() -> Harness<impl App> {
    Harness::new(
        sketch(P::default(), |s| {
            let page: Node<Act<P>> = if s.page == 0 {
                text("First page").into()
            } else {
                text("Second page").into()
            };
            col![
                button("Next").act(|s: &mut P| s.page = 1).w(80.0),
                route(s.page, page)
            ]
            .page()
        }),
        400.0,
        200.0,
    )
}

#[test]
fn route_cross_fades_then_settles_and_goes_idle() {
    let mut h = pages();
    h.advance(16);
    assert!(texts_with_alpha(&h)
        .iter()
        .any(|(t, a)| t == "First page" && *a > 0.9));
    h.click("Next");
    h.advance(16).advance(60);
    let mid = texts_with_alpha(&h);
    let old = mid.iter().find(|(t, _)| t == "First page").map(|x| x.1);
    let new = mid.iter().find(|(t, _)| t == "Second page").map(|x| x.1);
    assert!(
        old.is_some_and(|a| a > 0.0 && a < 0.95),
        "outgoing page still drawn, fading: {old:?}"
    );
    assert!(
        new.is_some_and(|a| a < 0.9),
        "incoming page fading in: {new:?}"
    );
    h.advance(600);
    let end = texts_with_alpha(&h);
    assert!(
        !end.iter().any(|(t, _)| t == "First page"),
        "old page is gone"
    );
    assert!(end.iter().any(|(t, a)| t == "Second page" && *a > 0.99));
}
