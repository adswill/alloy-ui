//! Settings panel: section list on the left, grouped controls on the right.
use alloy_ui::prelude::*;

alloy_ui::choices! { enum Page { Appearance, General } }

struct S {
    page: Page,
    dark: bool,
    text_px: f32,
    login: bool,
    updates: bool,
}

fn view(s: &S) -> impl View<Act<S>> {
    let page = match s.page {
        Page::Appearance => col![
            heading("Appearance"),
            section(
                "Theme",
                choose([("Light", false), ("Dark", true)], s.dark).act(|s: &mut S, d| {
                    s.dark = d;
                    set_dark(d);
                })
            ),
            section(
                "Text size",
                col![
                    slider(12.0..=20.0, s.text_px)
                        .step(1.0)
                        .bind(|s: &mut S| &mut s.text_px),
                    "The quick brown fox jumps over the lazy dog.",
                    caption(format!("{:.0} px", s.text_px)).dim(),
                ]
            ),
        ],
        Page::General => col![
            heading("General"),
            section(
                "Startup",
                col![
                    check("Launch at login", s.login).bind(|s: &mut S| &mut s.login),
                    check("Check for updates automatically", s.updates)
                        .bind(|s: &mut S| &mut s.updates),
                ]
            ),
        ],
    };
    row![
        nav(&s.page)
            .bind(|s: &mut S| &mut s.page)
            .surface()
            .w(170.0)
            .fill_h(),
        page.gap(22.0).pad(28.0).fill()
    ]
    .gap(0.0)
    .fill()
}

fn main() {
    let state = S {
        page: Page::Appearance,
        dark: false,
        text_px: 14.0,
        login: false,
        updates: true,
    };
    sketch(state, view).run_sized("Settings", 560.0, 300.0)
}
