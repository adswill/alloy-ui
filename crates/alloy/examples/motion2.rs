//! Motion II: surfaces, transitions and loading states. Everything animated
//! here is one call; nothing loops except the loading indicators, which stop
//! when hidden or when reduced motion is on.
//!
//! `--headless x.png --click Motion --advance 140` captures a frame mid-transition.
use alloy_ui::prelude::*;

#[derive(Default)]
struct S {
    page: usize,
    expand: bool,
    flip: bool,
    list: bool,
    visitors: i64,
    step: u32,
    ring: f32,
    go: bool,
}

type V = Node<Act<S>>;

fn main() {
    sketch(
        S {
            list: true,
            visitors: 1284,
            ring: 0.64,
            ..S::default()
        },
        |s| {
            let page: V = match s.page {
                0 => surfaces(),
                1 => motion(s),
                2 => loading(s),
                _ => easing(s),
            };
            col![
                row![
                    heading("Motion II"),
                    space(),
                    tabs(["Surfaces", "Motion", "Loading", "Easing"], at!(s.page)),
                ]
                .align_center(),
                route(s.page, page),
            ]
            .gap(16.0)
            .page()
        },
    )
    .run_sized("Motion II", 720.0, 640.0)
}

fn stripes() -> V {
    let p = palette();
    let bar = |a: f32, w: f32| -> V { text("").w(w).h(150.0).bg(p.accent.with_alpha(a)).into() };
    row![
        bar(0.95, 36.0),
        bar(0.25, 24.0),
        bar(0.7, 52.0),
        bar(0.15, 30.0),
        bar(0.85, 44.0),
        bar(0.35, 28.0),
        bar(0.6, 60.0),
        bar(0.2, 26.0),
        bar(0.9, 40.0),
    ]
    .gap(0.0)
    .into()
}

fn surfaces() -> V {
    let p = palette();
    col![
        row![
            col![heading("Lift"), caption("Hover to rise")]
                .gap(2.0)
                .card()
                .lift(),
            col![heading("Tilt"), caption("Hover for depth")]
                .gap(2.0)
                .card()
                .tilt(),
            col![heading("Inset"), caption("A recessed well")]
                .gap(2.0)
                .pad(16.0)
                .bg(p.background)
                .round(8.0)
                .inset(),
        ]
        .gap(14.0),
        row![
            button("Deploy")
                .primary()
                .act(|_: &mut S| {})
                .boxed()
                .ripple_color(Color::hex(0xFFFFFF))
                .round(6.0),
            button("Press and hold")
                .act(|_: &mut S| {})
                .glow()
                .round(6.0),
            col![heading("Grain"), caption("A 3.5% film of noise")]
                .gap(2.0)
                .card()
                .grain(0.035),
        ]
        .gap(14.0)
        .align_center(),
        text("Frosted glass over live content").label().dim(),
        stack([
            Layer::from(stripes()),
            Layer::from(
                col![
                    heading("Release 2.4"),
                    caption("Rolled out to 3 of 3 regions"),
                ]
                .gap(2.0)
                .pad(18.0)
                .frosted()
                .round(10.0)
            )
            .anchor(Anchor::Center),
        ]),
    ]
    .gap(14.0)
    .into()
}

fn chip(name: &'static str) -> V {
    shared(
        name,
        text(name).pad_xy(14.0, 8.0).surface().outline().round(16.0),
    )
}

fn motion(s: &S) -> V {
    let t = timeline(s.step, 1100);
    let bar = |from: u32, to: u32, w: f32| -> V {
        text("")
            .w(w * t.eased(from, to, Easing::QUART_OUT))
            .h(8.0)
            .bg(palette().accent)
            .round(4.0)
            .into()
    };
    let order: [&'static str; 3] = if s.flip {
        ["Gamma", "Beta", "Alpha"]
    } else {
        ["Alpha", "Beta", "Gamma"]
    };
    col![
        row![
            toggle("Expand", at!(s.expand)),
            button("Reorder").act(|s: &mut S| s.flip = !s.flip),
            button("Replay").act(|s: &mut S| s.step += 1),
            toggle("List", at!(s.list)),
        ]
        .gap(14.0)
        .align_center(),
        row![
            glide(
                col![
                    heading("Layout glide"),
                    caption("Size and position changes animate by themselves."),
                    text("Details appear and everything below follows.")
                        .dim()
                        .when(s.expand),
                ]
                .gap(4.0)
                .card()
                .w(300.0)
            ),
            col![
                row![chip(order[0]), chip(order[1]), chip(order[2])].gap(8.0),
                col![
                    caption("Timeline: three parts, overlapping"),
                    bar(0, 500, 260.0),
                    bar(200, 700, 200.0),
                    bar(450, 1100, 140.0),
                ]
                .gap(8.0),
            ]
            .gap(18.0),
        ]
        .gap(24.0),
        row![
            {
                let list: V = if s.list {
                    stagger(
                        [
                            "Build passed",
                            "Tests passed",
                            "Deployed to 3 regions",
                            "Smoke tests green",
                        ]
                        .map(|l| -> V {
                            text(l)
                                .pad_xy(14.0, 10.0)
                                .surface()
                                .outline()
                                .round(6.0)
                                .w(250.0)
                                .into()
                        }),
                    )
                    .gap(6.0)
                    .into()
                } else {
                    text("").into()
                };
                list
            },
            col![
                caption("Visitors today"),
                counter(s.visitors as f64).large(),
                row![
                    button("+1").act(|s: &mut S| s.visitors += 1),
                    button("+137").act(|s: &mut S| s.visitors += 137),
                    button("Reset").ghost().act(|s: &mut S| s.visitors = 1284),
                ]
                .gap(8.0),
            ]
            .gap(8.0),
        ]
        .gap(40.0),
    ]
    .gap(20.0)
    .into()
}

fn loading(s: &S) -> V {
    row![
        col![
            caption("Skeleton, while data loads"),
            row![
                skeleton_circle(40.0),
                col![skeleton(160.0, 12.0), skeleton(110.0, 10.0)].gap(8.0),
            ]
            .gap(12.0)
            .align_center(),
            skeleton_text(3).gap(8.0).w(260.0),
        ]
        .gap(14.0)
        .card()
        .w(300.0),
        col![
            caption("Indeterminate"),
            row![spinner(), dots(), spinner_sized(32.0)]
                .gap(24.0)
                .align_center(),
            loading_bar(220.0),
            caption("Determinate"),
            row![
                progress_ring(s.ring),
                slider(0.0..=1.0, at!(s.ring)).w(160.0)
            ]
            .gap(18.0)
            .align_center(),
        ]
        .gap(16.0)
        .card()
        .w(330.0),
    ]
    .gap(20.0)
    .into()
}

fn easing(s: &S) -> V {
    let rows: Vec<V> = Easing::PRESETS
        .iter()
        .map(|(name, e)| -> V {
            let x = animated_with(if s.go { 1.0f32 } else { 0.0 }, 900, *e);
            row![
                text(*name).caption().w(96.0),
                row![
                    text("").w(x * 300.0).h(1.0),
                    text("").w(10.0).h(10.0).bg(palette().accent).round(5.0),
                ]
                .gap(0.0)
                .w(320.0),
            ]
            .align_center()
            .into()
        })
        .collect();
    col![
        row![
            button("Play").primary().act(|s: &mut S| s.go = !s.go),
            caption("Every curve in `Easing::PRESETS`, 900 ms. None overshoot."),
        ]
        .gap(12.0)
        .align_center(),
        col(rows).gap(3.0),
    ]
    .gap(12.0)
    .into()
}
