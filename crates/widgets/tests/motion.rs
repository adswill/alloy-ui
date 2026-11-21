//! Animation behaviour: tweens, springs, presence, reduced motion, idle.
use alloy_ui_core::*;
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;

type M = ();

fn root(ui: &Ui, child: Element<M>) -> (Element<M>, Layout) {
    let r = Root::new(ui, child).into_element();
    let l =
        r.0.arrange(Rectangle::new(Point::default(), Size::new(300.0, 300.0)));
    (r, l)
}

fn tick(r: &mut Element<M>, l: &Layout, ms: u64) {
    r.0.on_event(&Event::Tick(ms), l, Point::default(), &mut Vec::new());
}

fn solid(ui: &Ui) -> Element<M> {
    Text::new(ui, "Saved").into_element()
}

#[test]
fn spring_settles_without_overshoot_and_goes_idle() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, solid(&ui));
    tick(&mut r, &l, 0);
    assert_eq!(ui.spring(1, 0.0, Spring::SNAPPY), 0.0);
    ui.spring(1, 1.0, Spring::SNAPPY);
    assert!(ui.animating());
    let mut peak = 0.0f32;
    for i in 1..=60 {
        tick(&mut r, &l, i * 16);
        peak = peak.max(ui.spring(1, 1.0, Spring::SNAPPY));
    }
    assert!(peak <= 1.003, "overshot to {peak}");
    assert!((ui.spring(1, 1.0, Spring::SNAPPY) - 1.0).abs() < 0.002);
    assert!(!ui.animating(), "no ticking once settled");
}

#[test]
fn retargeting_a_spring_keeps_velocity() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, solid(&ui));
    tick(&mut r, &l, 0);
    ui.spring(2, 0.0, Spring::SNAPPY);
    ui.spring(2, 1.0, Spring::SNAPPY);
    for i in 1..=4 {
        tick(&mut r, &l, i * 16);
    }
    let mid = ui.spring(2, 1.0, Spring::SNAPPY);
    assert!(mid > 0.05 && mid < 0.95);
    // Reverse: the carried velocity makes it fall back more slowly than a
    // spring that was at rest at the same spot.
    ui.spring(2, 0.0, Spring::SNAPPY);
    tick(&mut r, &l, 5 * 16 + 16);
    let carried = ui.spring(2, 0.0, Spring::SNAPPY);
    let (x, _) = Spring::SNAPPY.step(mid, 0.0, 0.0, 0.032);
    assert!(carried > x + 0.005, "{carried} vs rest {x}");
}

#[test]
fn animates_colours_and_points() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, solid(&ui));
    tick(&mut r, &l, 0);
    let a = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let b = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    ui.animate(3, a, Transition::ms(100));
    ui.animate(3, b, Transition::ms(100));
    tick(&mut r, &l, 16); // start
    tick(&mut r, &l, 36);
    let mid = ui.animate(3, b, Transition::ms(100));
    assert!(mid.r > 0.05 && mid.b > 0.05, "{mid:?}");
    tick(&mut r, &l, 200);
    assert_eq!(ui.animate(3, b, Transition::ms(100)), b);
    let p = ui.animate(4, Point::new(1.0, 2.0), Transition::spring());
    assert_eq!(p, Point::new(1.0, 2.0));
}

#[test]
fn presence_plays_exit_then_unmounts() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, solid(&ui));
    tick(&mut r, &l, 0);
    assert!(!ui.presence(5, false, 150).mounted, "starts hidden");
    ui.presence(5, true, 150);
    tick(&mut r, &l, 10);
    tick(&mut r, &l, 90);
    let mid = ui.presence(5, true, 150);
    assert!(mid.mounted && mid.progress > 0.2 && mid.progress < 1.0);
    tick(&mut r, &l, 400);
    assert_eq!(ui.presence(5, true, 150).progress, 1.0);
    ui.presence(5, false, 150);
    tick(&mut r, &l, 410);
    tick(&mut r, &l, 480);
    assert!(ui.presence(5, false, 150).mounted, "still fading out");
    tick(&mut r, &l, 900);
    assert!(!ui.presence(5, false, 150).mounted);
    assert!(!ui.animating());
}

#[test]
fn reduced_motion_jumps() {
    let ui = Ui::new(Theme::light());
    ui.set_reduced_motion(true);
    let (mut r, l) = root(&ui, solid(&ui));
    tick(&mut r, &l, 0);
    ui.ease(6, 0.0);
    assert_eq!(ui.ease(6, 1.0), 1.0);
    ui.spring(7, 0.0, Spring::SNAPPY);
    assert_eq!(ui.spring(7, 1.0, Spring::SNAPPY), 1.0);
    ui.presence(8, false, 150);
    assert_eq!(ui.presence(8, true, 150).progress, 1.0);
    assert!(!ui.animating());
}

fn reveal_cmds(ui: &Ui, show: bool, now: u64, r: &mut Option<(Element<M>, Layout)>) -> usize {
    let child = Reveal::new(ui.clone(), 9, show, solid(ui))
        .collapse(true)
        .slide(8.0)
        .into_element();
    let (mut el, l) = root(ui, child);
    tick(&mut el, &l, now);
    let mut list = DrawList::default();
    el.0.draw(&l, &mut list);
    let n = list.0.len();
    *r = Some((el, l));
    n
}

#[test]
fn reveal_collapses_height_and_stops_drawing_when_gone() {
    let ui = Ui::new(Theme::light());
    let mut keep = None;
    assert_eq!(
        reveal_cmds(&ui, false, 0, &mut keep),
        0,
        "hidden draws nothing"
    );
    let h0 = keep.as_ref().unwrap().1.children[0].bounds.size.height;
    assert_eq!(h0, 0.0);
    reveal_cmds(&ui, true, 16, &mut keep);
    reveal_cmds(&ui, true, 100, &mut keep);
    let hm = keep.as_ref().unwrap().1.children[0].bounds.size.height;
    assert!(hm > 0.0, "growing: {hm}");
    reveal_cmds(&ui, true, 600, &mut keep);
    let hf = keep.as_ref().unwrap().1.children[0].bounds.size.height;
    assert!(hf > hm, "full {hf} > {hm}");
    assert!(reveal_cmds(&ui, true, 700, &mut keep) > 0);
}
