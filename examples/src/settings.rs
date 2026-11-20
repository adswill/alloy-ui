//! Settings panel: section list on the left, grouped controls on the right.
use alloy_ui::prelude::*;

alloy_ui::choices! { enum Page { Appearance, General, Privacy } }

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (640.0, 420.0);

struct S {
    page: Page,
    dark: bool,
    text_px: f32,
    login: bool,
    updates: bool,
    crash_reports: bool,
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
                .gap(10.0)
            ),
        ],
        Page::Privacy => col![
            heading("Privacy"),
            section(
                "Diagnostics",
                col![
                    check("Send crash reports", s.crash_reports)
                        .bind(|s: &mut S| &mut s.crash_reports),
                    caption("Reports contain a stack trace and the OS version, nothing else.")
                        .dim(),
                ]
                .gap(10.0)
            ),
        ],
    };
    row![
        nav(&s.page)
            .bind(|s: &mut S| &mut s.page)
            .surface()
            .w(180.0)
            .fill_h(),
        page.gap(24.0).pad(28.0).fill()
    ]
    .gap(0.0)
    .fill()
}

/// The app.
pub fn app() -> impl App {
    sketch(
        S {
            page: Page::Appearance,
            dark: false,
            text_px: 14.0,
            login: false,
            updates: true,
            crash_reports: false,
        },
        view,
    )
}

/// Open the window (or render headless, see the crate docs).
pub fn run() {
    app().run_sized("Settings", SIZE.0, SIZE.1)
}
