//! Second-wave effects: surfaces, transitions, loading indicators, counters.
//! Goldens (CPU reference, text stripped) cover the look; the rest pin behaviour,
//! including that nothing keeps ticking once it has settled.
//! Regenerate goldens with `UPDATE_GOLDEN=1 cargo test -p alloy-ui-widgets --test fx`.

use alloy_ui_core::*;
use alloy_ui_layout::Length;
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::path::Path;

type M = ();

fn block(w: f32, h: f32, c: Color) -> Element<M> {
    Container::new(Space::fixed(w, h).into_element())
        .background(c)
        .into_element()
}

fn stripes(ui: &Ui) -> Element<M> {
    let a = ui.theme().palette.accent;
    let cols = [0.95, 0.25, 0.7, 0.15, 0.85, 0.3, 0.6, 0.2];
    Row::new(
        cols.iter()
            .enumerate()
            .map(|(i, al)| block(18.0 + (i % 3) as f32 * 10.0, 90.0, a.with_alpha(*al)))
            .collect(),
    )
    .spacing(0.0)
    .into_element()
}

fn surfaces(ui: &Ui) -> Element<M> {
    let t = ui.theme();
    let panel = |layer: Layer, w: f32| {
        Fx::new(
            ui,
            1 + w as u64,
            8.0,
            Container::new(Space::fixed(w, 40.0).into_element())
                .background(t.palette.surface)
                .radius(8.0)
                .into_element(),
        )
        .layer(layer)
        .into_element()
    };
    let glass = Fx::new(
        ui,
        99,
        10.0,
        Container::new(Space::fixed(110.0, 40.0).into_element())
            .radius(10.0)
            .into_element(),
    )
    .layer(Layer::Frosted(8.0, None))
    .into_element();
    Column::new(vec![
        Stack::new(vec![
            (stripes(ui), Place::default()),
            (
                glass,
                Place {
                    x: style::Align::Center,
                    y: style::Align::Center,
                    ..Place::default()
                },
            ),
        ])
        .into_element(),
        Row::new(vec![
            panel(Layer::Inset, 60.0),
            panel(Layer::Grain(0.05), 61.0),
            Skeleton::new(ui, 40.0)
                .width(Length::Fixed(70.0))
                .into_element(),
            Spinner::new(ui, SpinnerKind::Ring)
                .size(32.0)
                .into_element(),
        ])
        .spacing(12.0)
        .into_element(),
    ])
    .spacing(14.0)
    .into_element()
}

fn render(theme: Theme, ui_setup: impl Fn(&Ui)) -> alloy_ui_render::Image {
    let bg = theme.palette.background;
    let ui = Ui::new(theme);
    ui_setup(&ui);
    let root = Root::new(
        &ui,
        Container::new(surfaces(&ui))
            .padding(alloy_ui_core::style::Padding::all(16.0))
            .into_element(),
    )
    .into_element();
    let layout = alloy_ui_widgets::layout(&root, Size::new(300.0, 220.0));
    let mut list = DrawList::default();
    root.0.draw(&layout, &mut list);
    list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
    let mut r = CpuRenderer::new(600, 440);
    r.set_background(bg);
    r.render_to_image(&list, 600, 440, 2.0)
}

fn check(name: &str, img: &alloy_ui_render::Image) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    assert_golden(&dir, name, img, Tolerance::CPU);
}

#[test]
fn golden_surfaces_light() {
    // Reduced motion pins the skeleton and spinner to their still poses.
    check(
        "fx_surfaces_light",
        &render(Theme::light(), |ui| ui.set_reduced_motion(true)),
    );
}

#[test]
fn golden_surfaces_dark() {
    check(
        "fx_surfaces_dark",
        &render(Theme::dark(), |ui| ui.set_reduced_motion(true)),
    );
}

// ---------------------------------------------------------------- behaviour

fn root(ui: &Ui, child: Element<M>) -> (Element<M>, Layout) {
    let r = Root::new(ui, child).into_element();
    let l =
        r.0.arrange(Rectangle::new(Point::default(), Size::new(320.0, 240.0)));
    (r, l)
}

fn tick(r: &mut Element<M>, l: &Layout, ms: u64) {
    r.0.on_event(&Event::Tick(ms), l, Point::default(), &mut Vec::new());
}

fn draw(r: &Element<M>, l: &Layout) -> Vec<DrawCommand> {
    let mut list = DrawList::default();
    r.0.draw(l, &mut list);
    list.0
}

fn text_alpha(cmds: &[DrawCommand], s: &str) -> Option<f32> {
    cmds.iter().find_map(|c| match c {
        DrawCommand::Text { content, color, .. } if content == s => Some(color.a),
        _ => None,
    })
}

#[test]
fn clocks_start_on_the_first_tick_after_creation_not_at_a_stale_time() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, Space::fixed(1.0, 1.0).into_element());
    tick(&mut r, &l, 5_000); // the app was idle; the last tick is old
    let (el, running) = ui.clock(1, 300);
    assert_eq!((el, running), (0, true));
    assert!(ui.animating(), "an unstarted clock asks for ticks");
    tick(&mut r, &l, 9_000);
    assert_eq!(ui.clock(1, 300).0, 0, "starts at the tick that follows it");
    tick(&mut r, &l, 9_150);
    assert_eq!(ui.clock(1, 300).0, 150);
    tick(&mut r, &l, 9_400);
    assert_eq!(ui.clock(1, 300), (300, false));
    assert!(!ui.animating());
}

#[test]
fn timeline_replays_when_its_trigger_changes() {
    let ui = Ui::new(Theme::light());
    let (mut r, l) = root(&ui, Space::fixed(1.0, 1.0).into_element());
    tick(&mut r, &l, 0);
    ui.timeline(7, 1, 400);
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 216);
    let t = ui.timeline(7, 1, 400);
    assert!((t.span(0, 400) - 0.5).abs() < 0.02, "{t:?}");
    tick(&mut r, &l, 600);
    assert!(ui.timeline(7, 1, 400).done());
    assert!(!ui.timeline(7, 2, 400).done(), "new trigger starts over");
    assert!(ui.animating());
}

#[test]
fn ripple_appears_on_press_and_is_gone_afterwards() {
    let ui = Ui::new(Theme::light());
    let fx = Fx::new(&ui, 5, 6.0, block(100.0, 40.0, Color::hex(0xFFFFFF)))
        .layer(Layer::Ripple(None))
        .into_element();
    let (mut r, l) = root(&ui, fx);
    tick(&mut r, &l, 0);
    let radial = |c: &[DrawCommand]| {
        c.iter()
            .filter(|c| matches!(c, DrawCommand::RadialGradient { .. }))
            .count()
    };
    assert_eq!(radial(&draw(&r, &l)), 0, "nothing before a press");
    let at = Point::new(30.0, 20.0);
    r.0.on_event(&Event::CursorMoved(at), &l, at, &mut Vec::new());
    r.0.on_event(
        &Event::MousePressed(MouseButton::Left),
        &l,
        at,
        &mut Vec::new(),
    );
    r.0.on_event(
        &Event::MouseReleased(MouseButton::Left),
        &l,
        at,
        &mut Vec::new(),
    );
    draw(&r, &l);
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 150);
    let mid = draw(&r, &l);
    assert_eq!(radial(&mid), 1);
    assert!(ui.animating());
    tick(&mut r, &l, 1_000);
    assert_eq!(radial(&draw(&r, &l)), 0, "faded out");
    assert!(!ui.animating(), "idle again");
}

#[test]
fn reduced_motion_drops_the_ripple_and_loops() {
    let ui = Ui::new(Theme::light());
    ui.set_reduced_motion(true);
    let fx = Fx::new(&ui, 5, 6.0, block(100.0, 40.0, Color::hex(0xFFFFFF)))
        .layer(Layer::Ripple(None))
        .into_element();
    let (mut r, l) = root(&ui, fx);
    tick(&mut r, &l, 0);
    let at = Point::new(30.0, 20.0);
    r.0.on_event(
        &Event::MousePressed(MouseButton::Left),
        &l,
        at,
        &mut Vec::new(),
    );
    tick(&mut r, &l, 16);
    assert!(!draw(&r, &l)
        .iter()
        .any(|c| matches!(c, DrawCommand::RadialGradient { .. })));
    for s in [
        Skeleton::new(&ui, 10.0).into_element::<M>(),
        Spinner::new(&ui, SpinnerKind::Dots).into_element::<M>(),
    ] {
        assert!(!s.0.animating(), "stands still under reduced motion");
    }
    assert!(!ui.animating());
}

#[test]
fn loops_run_only_while_the_indicator_exists() {
    let ui = Ui::new(Theme::light());
    let s: Element<M> = Spinner::new(&ui, SpinnerKind::Ring).into_element();
    assert!(s.0.animating());
    let sk: Element<M> = Skeleton::new(&ui, 10.0).into_element();
    assert!(sk.0.animating());
    // A determinate ring settles and stops asking for ticks.
    let ring: Element<M> = ProgressRing::new(&ui, 3, 0.5).into_element();
    let (mut r, l) = root(&ui, ring);
    tick(&mut r, &l, 0);
    draw(&r, &l);
    tick(&mut r, &l, 5_000);
    assert!(!r.0.animating() || !ui.animating());
}

#[test]
fn enter_fades_up_from_nothing_then_replays_after_absence() {
    let ui = Ui::new(Theme::light());
    let mk = |ui: &Ui| Enter::new(ui, 11, 100, Text::new(ui, "Row").into_element()).into_element();
    let (mut r, l) = root(&ui, mk(&ui));
    tick(&mut r, &l, 0);
    assert!(
        text_alpha(&draw(&r, &l), "Row").unwrap() < 1e-3,
        "waits out its delay"
    );
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 116);
    assert!(
        text_alpha(&draw(&r, &l), "Row").unwrap() < 1e-3,
        "delay not over at 100 ms"
    );
    tick(&mut r, &l, 250);
    let mid = text_alpha(&draw(&r, &l), "Row").unwrap();
    assert!(mid > 0.0 && mid < 1.0, "{mid}");
    tick(&mut r, &l, 1_000);
    assert_eq!(text_alpha(&draw(&r, &l), "Row"), Some(1.0));
    assert!(!ui.animating());
    // Two frames drawn without it, then it is back: it plays again.
    let (gone, gl) = root(&ui, Space::fixed(1.0, 1.0).into_element());
    draw(&gone, &gl);
    draw(&gone, &gl);
    let (mut r2, l2) = root(&ui, mk(&ui));
    tick(&mut r2, &l2, 1_100);
    assert!(text_alpha(&draw(&r2, &l2), "Row").unwrap() < 1e-3);
}

#[test]
fn glide_eases_its_size_and_settles() {
    let ui = Ui::new(Theme::light());
    let mk = |ui: &Ui, h: f32| {
        Glide::new(ui, 21, block(50.0, h, Color::hex(0xCCCCCC)))
            .size_only()
            .into_element()
    };
    let (mut r, l) = root(&ui, mk(&ui, 40.0));
    tick(&mut r, &l, 0);
    assert_eq!(r.0.style().height, Length::Fixed(40.0));
    let (mut r, l) = root(&ui, mk(&ui, 100.0));
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 100);
    match r.0.style().height {
        Length::Fixed(h) => assert!(h > 41.0 && h < 99.0, "mid-flight height {h}"),
        other => panic!("{other:?}"),
    }
    assert!(ui.animating());
    tick(&mut r, &l, 2_000);
    assert_eq!(r.0.style().height, Length::Fixed(100.0));
    assert!(!ui.animating());
}

#[test]
fn route_keeps_the_outgoing_page_until_the_transition_ends() {
    let ui = Ui::new(Theme::light());
    let mk = |ui: &Ui, page: u64, label: &str| {
        Route::new(ui, 31, page, Text::new(ui, label).into_element()).into_element()
    };
    let (mut r, l) = root(&ui, mk(&ui, 0, "One"));
    tick(&mut r, &l, 0);
    draw(&r, &l);
    let (mut r, l) = root(&ui, mk(&ui, 1, "Two"));
    draw(&r, &l);
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 90);
    let mid = draw(&r, &l);
    assert!(text_alpha(&mid, "One").is_some_and(|a| a > 0.0 && a < 1.0));
    assert!(text_alpha(&mid, "Two").is_some_and(|a| a < 1.0));
    tick(&mut r, &l, 2_000);
    let end = draw(&r, &l);
    assert!(text_alpha(&end, "One").is_none());
    assert_eq!(text_alpha(&end, "Two"), Some(1.0));
    assert!(!ui.animating());
}

#[test]
fn counter_rests_on_whole_digits_and_rolls_between() {
    let ui = Ui::new(Theme::light());
    let mk = |ui: &Ui, v: f64| Counter::new(ui, 41, v).into_element::<M>();
    let digits = |c: &[DrawCommand]| -> String {
        c.iter()
            .filter_map(|c| match c {
                DrawCommand::Text { content, color, .. } if color.a > 0.99 => Some(content.clone()),
                _ => None,
            })
            .collect()
    };
    let (mut r, l) = root(&ui, mk(&ui, 1284.0));
    tick(&mut r, &l, 0);
    assert_eq!(digits(&draw(&r, &l)), "1,284");
    let (mut r, l) = root(&ui, mk(&ui, 1421.0));
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 200);
    let mid = draw(&r, &l);
    let faded = mid
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { color, .. } if color.a < 0.99 && color.a > 0.0))
        .count();
    assert!(faded > 0, "some wheel is mid-turn");
    tick(&mut r, &l, 3_000);
    assert_eq!(digits(&draw(&r, &l)), "1,421");
    assert!(!ui.animating());
}

#[test]
fn counter_carries_only_when_the_lower_digits_pass_nine() {
    let ui = Ui::new(Theme::light());
    // 19.5: the units wheel is half way from 9 to 0, so the tens wheel has begun to turn.
    let c = Counter::new(&ui, 51, 19.0).into_element::<M>();
    let (mut r, l) = root(&ui, c);
    tick(&mut r, &l, 0);
    draw(&r, &l);
    let c = Counter::new(&ui, 51, 20.0).into_element::<M>();
    let (mut r, l) = root(&ui, c);
    tick(&mut r, &l, 16);
    tick(&mut r, &l, 16 + 60);
    let cmds = draw(&r, &l);
    let tens_rolling = cmds.iter().any(|c| matches!(c, DrawCommand::Text { content, color, .. } if content == "2" && color.a > 0.0 && color.a < 1.0));
    let ones_rolling = cmds.iter().any(|c| matches!(c, DrawCommand::Text { content, color, .. } if content == "0" && color.a > 0.0 && color.a < 1.0));
    assert!(
        ones_rolling && tens_rolling,
        "carry rolls both wheels together"
    );
}

#[test]
fn stack_overlays_layers_and_anchors_them() {
    let ui = Ui::new(Theme::light());
    let base = block(200.0, 100.0, Color::hex(0x336699));
    let badge = block(20.0, 10.0, Color::hex(0xFF0000));
    let stack = Stack::new(vec![
        (base, Place::default()),
        (
            badge,
            Place {
                x: style::Align::End,
                y: style::Align::End,
                offset: Point::new(-4.0, -4.0),
            },
        ),
    ])
    .into_element();
    let (r, l) = root(&ui, stack);
    let cmds = draw(&r, &l);
    let rects: Vec<Rectangle> = cmds
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Quad { bounds, .. } => Some(*bounds),
            _ => None,
        })
        .collect();
    assert_eq!(rects.len(), 2);
    assert_eq!(
        rects[0].size,
        Size::new(200.0, 100.0),
        "the stack is as big as its largest layer"
    );
    let b = rects[1];
    assert_eq!(
        (b.origin.x + b.size.width, b.origin.y + b.size.height),
        (196.0, 96.0)
    );
}

#[test]
fn frosted_glass_draws_a_blur_before_its_content_and_a_hairline_after() {
    let ui = Ui::new(Theme::light());
    let fx = Fx::new(&ui, 61, 8.0, block(80.0, 30.0, Color::TRANSPARENT))
        .layer(Layer::Frosted(12.0, None))
        .into_element();
    let (r, l) = root(&ui, fx);
    let cmds = draw(&r, &l);
    let blur = cmds
        .iter()
        .position(|c| matches!(c, DrawCommand::BackdropBlur { .. }));
    assert!(blur.is_some());
    assert!(matches!(
        cmds.last(),
        Some(DrawCommand::Quad {
            border: Some(_),
            ..
        })
    ));
}
