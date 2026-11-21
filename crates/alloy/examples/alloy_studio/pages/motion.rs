//! Motion: reveal, eased values, springs, gradients, elevation and toasts.
use crate::ui::*;
use crate::Studio;
use alloy_ui::i18n;
use alloy_ui::prelude::*;

pub fn view(s: &Studio) -> N {
    let p = palette();
    let tint = |a: f32| p.accent.with_alpha(a);
    page(col![
        header(
            i18n::translate("mo.title", &[]),
            i18n::translate("mo.sub", &[]),
            text("")
        ),
        row![
            col![
                col![
                    row![
                        col![heading("web-frontend"), caption("main · 4 minutes ago").dim()].gap(2.0),
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
                            row![badge("Live").success().dot(), badge("v2.4.0").accent().outline()].gap(8.0),
                        ]
                        .gap(6.0)
                        .pad_xy(0.0, 10.0),
                    ),
                ]
                .card()
                .elevated()
                .pad(16.0),
                caption("Reveal: fade, a 6 px rise and a height collapse").dim(),
            ]
            .gap(8.0)
            .fill_w(),
            col![
                col![
                    toggle("Wide layout", at!(s.wide)),
                    col![caption("")]
                        .gradient(tint(1.0), tint(0.55), 90.0)
                        .round(6.0)
                        .pad_xy(animated(if s.wide { 300.0 } else { 120.0 }) / 2.0, 6.0),
                ]
                .gap(10.0)
                .card()
                .pad(16.0),
                caption("An eased value follows the toggle; a tonal gradient fills the bar").dim(),
            ]
            .gap(8.0)
            .fill_w(),
        ]
        .gap(16.0),
        row![
            col![
                row![
                    button("Sync").primary().toggle(at!(s.synced)),
                    fade(s.synced, caption("Synced just now")),
                ]
                .gap(12.0)
                .align_center()
                .card()
                .pad(16.0),
                caption("Fade in and out").dim(),
            ]
            .gap(8.0)
            .fill_w(),
            col![
                row![
                    slider(0.0..=100.0, at!(s.spring_v)).step(1.0).w(260.0),
                ]
                .card()
                .pad(16.0),
                caption("The thumb swells on a critically damped spring").dim(),
            ]
            .gap(8.0)
            .fill_w(),
        ]
        .gap(16.0),
        panel(
            "Toasts",
            row![
                button("Saved").act(|_| {
                    toast("Saved");
                }),
                button("Deleted, with undo").act(|s: &mut Studio| {
                    if !s.work.tasks.is_empty() {
                        s.work.tasks.remove(0);
                    }
                    toast("First task deleted").undo();
                }),
                button("Something failed").act(|_| {
                    toast("Could not reach the server").error().secs(5.0);
                }),
            ]
            .gap(8.0)
        ),
        panel(
            "Elevation",
            row![
                col![caption("Flat"), text("Hairline border")].gap(4.0).card().pad(16.0).fill_w(),
                col![caption("Raised"), text("Medium shadow")].gap(4.0).card().elevated().pad(16.0).fill_w(),
                col![caption("Tonal"), text("Gradient fill")]
                    .gap(4.0)
                    .gradient(tint(0.16), tint(0.04), 90.0)
                    .round(6.0)
                    .pad(16.0)
                    .fill_w(),
            ]
            .gap(14.0)
        ),
        caption("All motion is short, eased out and settles. OS reduced-motion makes every transition jump to its end.").dim(),
    ])
}
