//! Appearance: ten themes from six presets and two appearances, accent, density, high contrast,
//! icon packs, window effects and language, with a live preview and a contrast report.
use crate::ui::*;
use crate::{Studio, EFFECTS};
use alloy_ui::i18n;
use alloy_ui::prelude::*;
use alloy_ui::theme::{contrast_ratio, Appearance};
use alloy_ui::Icon;

fn hex(c: Color) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    b(c.r) << 16 | b(c.g) << 8 | b(c.b)
}

fn code(s: &Studio) -> String {
    let mut out = format!(
        "set_look(\n    Look::{}()\n    .preset(Preset::{:?})",
        if s.dark { "dark" } else { "light" },
        Preset::ALL[s.preset]
    );
    if let Some(c) = s.accent {
        out += &format!("\n    .accent(Color::hex(0x{:06X}))", hex(c));
    }
    if s.contrast {
        out += "\n    .high_contrast(true)";
    }
    if s.density != 1 {
        out += &format!("\n    .density(Density::{:?})", crate::DENSITIES[s.density]);
    }
    out + ",\n);"
}

fn ratio(label: &str, fg: Color, bg: Color, min: f32) -> N {
    let r = contrast_ratio(fg, bg);
    row![
        text(label).fill_w(),
        caption(format!("{r:.1}:1")).dim(),
        if r >= min {
            badge("AA").success()
        } else {
            badge("Fail").danger()
        },
    ]
    .gap(8.0)
    .align_center()
    .into()
}

fn controls(s: &Studio) -> N {
    let appearance = if s.dark {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    col![
        section(
            "Preset",
            col![
                row(Preset::ALL.iter().enumerate().map(|(i, pr)| {
                    swatch(pr.brand(appearance))
                        .named(pr.name())
                        .selected(s.preset == i)
                        .act(move |s: &mut Studio| {
                            s.preset = i;
                            s.accent = None;
                        })
                }))
                .gap(10.0),
                caption(format!(
                    "{}. {}",
                    Preset::ALL[s.preset].name(),
                    Preset::ALL[s.preset].tagline()
                ))
                .dim(),
            ]
            .gap(8.0)
        ),
        section(
            "Appearance",
            row![
                choose([("Light", false), ("Dark", true)], at!(s.dark)),
                check("High contrast", at!(s.contrast)),
            ]
            .gap(16.0)
            .align_center()
        ),
        section(
            "Accent",
            col![
                color_picker(at!(s.picker)).act(|s: &mut Studio, c| {
                    s.picker = c;
                    s.accent = Some(c);
                }),
                button("Use the preset's accent")
                    .ghost()
                    .act(|s: &mut Studio| s.accent = None),
            ]
            .gap(8.0)
        ),
        section(
            "Density, icons and window",
            col![
                choose(
                    [("Compact", 0usize), ("Comfortable", 1), ("Spacious", 2)],
                    at!(s.density)
                ),
                choose([("Lucide", false), ("Phosphor", true)], at!(s.phosphor)),
                choose(
                    EFFECTS.iter().enumerate().map(|(i, (l, _))| (*l, i)),
                    at!(s.effect)
                ),
                caption("Window effects need OS support (Mica on Windows 11, vibrancy on macOS, a compositor on Linux); otherwise the window stays opaque.").dim(),
            ]
            .gap(10.0)
        ),
        section(
            "Language",
            i18n::language_picker::<Studio>(&[("en", "English"), ("fr", "Français"), ("ar", "العربية")])
        ),
    ]
    .gap(18.0)
    .pad(24.0)
    .into()
}

fn preview(s: &Studio) -> N {
    let p = palette();
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let deploys = vec![3.0, 5.0, 4.0, 8.0, 6.0, 7.0, 9.0];
    let errors = vec![2.0, 1.0, 3.0, 2.0, 1.0, 1.0, 0.0];
    col![
        row![
            icon(Icon::Package).size(20.0).accent(),
            heading("Deployments").fill_w(),
            input(at!(s.customer_filter))
                .hint("Search releases")
                .w(190.0),
            button("New release").primary().act(|_| {}),
        ]
        .gap(10.0)
        .align_center(),
        rule(),
        row![
            col![
                caption("Succeeded this week").dim(),
                heading("42"),
                text("Two rollbacks, both under a minute.").dim().italic(),
            ]
            .gap(4.0),
            sparkline(deploys.clone()).w(150.0),
        ]
        .gap(28.0)
        .align_center(),
        line_chart([("Deploys", deploys), ("Failures", errors)])
            .labels(days)
            .height(170.0),
        row![
            badge("Live").success().dot(),
            badge("Canary").warning().dot(),
            badge("Rolled back").danger().dot(),
            badge("v2.4.0").accent().outline(),
            badge("Stable"),
        ]
        .gap(8.0)
        .align_center(),
        row![
            button("Deploy").primary().act(|_| {}),
            button("Roll back").act(|_| {}),
            button("Skip").ghost().act(|_| {}),
            row![
                icon(Icon::Info).size(14.0).dim(),
                caption("Last deploy 14 minutes ago by Mira.").dim(),
            ]
            .gap(6.0)
            .align_center(),
        ]
        .gap(8.0)
        .align_center(),
        rule(),
        row![
            section(
                "Contrast (WCAG AA)",
                col![
                    ratio("Text on page", p.text, p.background, 4.5),
                    ratio("Secondary text", p.text_secondary, p.surface, 4.5),
                    ratio("Accent on page", p.accent, p.background, 4.5),
                    ratio("Label on accent", p.on_accent, p.accent, 4.5),
                    ratio("Focus ring", p.focus_ring, p.surface, 3.0),
                ]
                .gap(6.0)
            ),
            section(
                "The code for this look",
                markdown(format!("```rust\n{}\n```", code(s))).width(300.0)
            )
            .w(330.0),
        ]
        .gap(28.0),
    ]
    .gap(16.0)
    .pad(24.0)
    .into()
}

pub fn view(s: &Studio) -> N {
    col![
        col![header(
            i18n::translate("ap.title", &[]),
            i18n::translate("ap.sub", &[]),
            text("")
        )]
        .pad_xy(24.0, 18.0),
        row![
            scroll(controls(s)).w(390.0).fill_h(),
            scroll(preview(s)).fill()
        ]
        .gap(0.0)
        .fill(),
    ]
    .gap(0.0)
    .fill()
    .into()
}
