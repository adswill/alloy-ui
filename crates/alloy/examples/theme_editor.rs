//! Theme editor: pick a preset, appearance, accent, density and icon pack, watch a live
//! preview repaint, read the contrast report, and copy the `set_look(..)` line that makes it.
use alloy_ui::prelude::*;
use alloy_ui::theme::{contrast_ratio, Appearance};

state! {
    struct Editor {
        preset: usize,
        dark: bool,
        /// A hand-picked accent; `None` keeps the preset's own.
        custom: Option<Color>,
        picker: Color = Color::hex(0x1F5FD6),
        density: usize = 1,
        contrast: bool,
        phosphor: bool,
        notify: bool = true,
        autosave: bool = true,
        volume: f32 = 62.0,
        query: String,
    }
}

const DENSITIES: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];

impl Editor {
    fn look(&self) -> Look {
        let mut look = Look::light()
            .is_dark(self.dark)
            .preset(Preset::ALL[self.preset])
            .high_contrast(self.contrast)
            .density(DENSITIES[self.density]);
        if let Some(c) = self.custom {
            look = look.accent(c);
        }
        look
    }

    /// The Rust that reproduces this look, ready to paste.
    fn code(&self) -> String {
        let mut s = format!(
            "set_look(\n    Look::{}()\n    .preset(Preset::{:?})",
            if self.dark { "dark" } else { "light" },
            Preset::ALL[self.preset]
        );
        if let Some(c) = self.custom {
            s += &format!("\n    .accent(Color::hex(0x{:06X}))", hex(c));
        }
        if self.contrast {
            s += "\n    .high_contrast(true)";
        }
        if self.density != 1 {
            s += &format!("\n    .density(Density::{:?})", DENSITIES[self.density]);
        }
        s + ",\n);"
    }

    /// The same look as a `theme` line for a live `.alloy` view (see `live!`): the preset is
    /// carried by its accent colour, since the markup has no preset names.
    fn live_line(&self) -> String {
        let appearance = if self.dark {
            Appearance::Dark
        } else {
            Appearance::Light
        };
        let accent = self
            .custom
            .unwrap_or_else(|| Preset::ALL[self.preset].brand(appearance));
        let mut s = format!(
            "theme {} accent=#{:06X}",
            if self.dark { "dark" } else { "light" },
            hex(accent)
        );
        match DENSITIES[self.density] {
            Density::Compact => s += " density=compact",
            Density::Spacious => s += " density=spacious",
            _ => {}
        }
        if self.contrast {
            s += " contrast=high";
        }
        s
    }
}

fn hex(c: Color) -> u32 {
    let b = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    b(c.r) << 16 | b(c.g) << 8 | b(c.b)
}

/// One row of the contrast report: a label, the ratio, and a pass/fail badge.
fn ratio(label: &str, fg: Color, bg: Color, min: f32) -> impl View<Act<Editor>> {
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
}

fn editor_pane(s: &Editor) -> impl View<Act<Editor>> {
    col![
        heading("Theme editor"),
        section(
            "Preset",
            col![
                row(Preset::ALL.iter().enumerate().map(|(i, pr)| {
                    swatch(pr.brand(if s.dark {
                        Appearance::Dark
                    } else {
                        Appearance::Light
                    }))
                    .named(pr.name())
                    .selected(s.preset == i)
                    .act(move |s: &mut Editor| {
                        s.preset = i;
                        s.custom = None;
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
                color_picker(at!(s.picker)).act(|s: &mut Editor, c| {
                    s.picker = c;
                    s.custom = Some(c);
                }),
                button("Use the preset's accent")
                    .ghost()
                    .on(Act::new(|s: &mut Editor| s.custom = None)),
            ]
            .gap(8.0)
        ),
        section(
            "Density and icons",
            col![
                choose(
                    [("Compact", 0usize), ("Comfortable", 1), ("Spacious", 2)],
                    at!(s.density)
                ),
                choose([("Lucide", false), ("Phosphor", true)], at!(s.phosphor)),
            ]
            .gap(10.0)
        ),
    ]
    .gap(18.0)
    .pad(24.0)
}

fn preview(s: &Editor) -> impl View<Act<Editor>> {
    let p = palette();
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let deploys = vec![3.0, 5.0, 4.0, 8.0, 6.0, 7.0, 9.0];
    let errors = vec![2.0, 1.0, 3.0, 2.0, 1.0, 1.0, 0.0];
    col![
        row![
            icon(Icon::Package).size(20.0).accent(),
            heading("Deployments").fill_w(),
            input(at!(s.query)).hint("Search releases").w(190.0),
            button("New release")
                .primary()
                .on(Act::new(|_: &mut Editor| {})),
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
        section(
            "Notifications",
            col![
                toggle("Email me when a deploy fails", at!(s.notify)),
                check("Save drafts automatically", at!(s.autosave)),
                row![
                    icon(Icon::Bell).dim(),
                    slider(0.0..=100.0, s.volume)
                        .step(1.0)
                        .bind(|s: &mut Editor| &mut s.volume)
                        .w(220.0),
                    caption(format!("{:.0}%", s.volume)).dim(),
                ]
                .gap(10.0)
                .align_center(),
            ]
            .gap(10.0)
        ),
        row![
            button("Deploy").primary().on(Act::new(|_: &mut Editor| {})),
            button("Roll back").on(Act::new(|_: &mut Editor| {})),
            button("Skip").ghost().on(Act::new(|_: &mut Editor| {})),
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
                "Code",
                col![
                    markdown(format!("```rust\n{}\n```", s.code())).width(300.0),
                    row![
                        button("Copy Rust").act(|s: &mut Editor| {
                            alloy_ui::clipboard::copy(s.code());
                        }),
                        button("Copy .alloy line").ghost().act(|s: &mut Editor| {
                            alloy_ui::clipboard::copy(s.live_line());
                        }),
                    ]
                    .gap(8.0),
                    caption(s.live_line()).dim(),
                ]
                .gap(8.0)
            )
            .w(330.0),
        ]
        .gap(28.0),
    ]
    .gap(16.0)
    .pad(24.0)
}

fn view(s: &Editor) -> impl View<Act<Editor>> {
    set_look(s.look());
    set_icon_pack(if s.phosphor {
        IconPack::Phosphor
    } else {
        IconPack::Lucide
    });
    row![
        scroll(editor_pane(s)).w(380.0).fill_h(),
        scroll(preview(s)).fill()
    ]
    .gap(0.0)
    .fill()
}

fn main() {
    let env = |k: &str| std::env::var(k).ok();
    let s = Editor {
        preset: env("EDITOR_PRESET")
            .and_then(|v| v.parse().ok())
            .unwrap_or(1),
        dark: env("EDITOR_DARK").is_some(),
        phosphor: env("EDITOR_PHOSPHOR").is_some(),
        ..Editor::default()
    };
    sketch(s, view).run_sized("Alloy theme editor", 1120.0, 820.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_line_follows_the_controls() {
        let mut e = Editor::default();
        assert!(e.live_line().starts_with("theme light accent=#"));
        e.dark = true;
        e.custom = Some(Color::hex(0x6D4AFF));
        e.density = 0;
        e.contrast = true;
        assert_eq!(
            e.live_line(),
            "theme dark accent=#6D4AFF density=compact contrast=high"
        );
    }
}
