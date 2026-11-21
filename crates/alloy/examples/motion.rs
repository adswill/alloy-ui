//! Motion showcase: enter/exit, eased and spring values, gradient and elevation.
use alloy_ui::prelude::*;

#[derive(Default)]
struct S {
    details: bool,
    wide: bool,
    synced: bool,
}

fn main() {
    sketch(S::default(), |s| {
        let p = palette();
        let tint = |a: f32| p.accent.with_alpha(a);
        col![
            heading("Deploy"),
            text("Every transition below is eased out and settles; nothing loops.").dim(),
            col![
                row![
                    col![heading("web-frontend"), caption("main · 4 minutes ago")].gap(2.0),
                    space(),
                    toggle("Details", at!(s.details)),
                ]
                .align_center(),
                reveal(
                    s.details,
                    col![
                        rule(),
                        caption("Build 2m 14s · 38 tests passed · 0 warnings"),
                        caption("Rolled out to 3 of 3 regions"),
                    ]
                    .gap(6.0)
                    .pad_xy(0.0, 10.0),
                ),
            ]
            .card()
            .elevated(),
            // A value that eases: the bar's width follows the toggle.
            col![
                toggle("Wide layout", at!(s.wide)),
                col![caption("")]
                    .gradient(tint(1.0), tint(0.55), 90.0)
                    .round(6.0)
                    .pad_xy(animated(if s.wide { 300.0 } else { 120.0 }) / 2.0, 6.0),
            ]
            .gap(10.0)
            .card(),
            row![
                button("Sync").primary().toggle(at!(s.synced)),
                fade(s.synced, caption("Synced just now")),
            ]
            .gap(12.0)
            .align_center(),
        ]
        .page()
    })
    .run_sized("Motion", 480.0, 560.0)
}
