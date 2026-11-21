//! The theme gallery: six original presets, each with a light and a dark face.
//!
//! A preset is a pure function `(Preset, Appearance) -> Theme`, so it composes with the other
//! variants: `Theme::preset(Preset::Graphite, Appearance::Dark).density(Density::Compact)`.
//!
//! Every preset follows the rules in `DESIGN.md`: one accent, neutrals that carry a faint hint
//! of the accent's family (never beige or cream), small radii, no gradients, and WCAG AA text
//! on every surface (the tests below run all ten themes through the same contrast checks as the
//! stock theme). They differ in more than hue: each sets its own neutral temperature, radii and
//! depth, so they read as distinct products and not as recoloured copies.

use crate::{contrast_ratio, mix, Appearance, Radii, Theme};
use alloy_ui_core::Color;

/// A named look for the whole UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Preset {
    /// The stock deep-teal theme.
    #[default]
    Alloy,
    /// Cool steel greys with a clear cobalt accent. Soft radii; calm and corporate-clean.
    Graphite,
    /// Neutral charcoal with a signal-orange accent. Crisp, tight radii; for tools.
    Ember,
    /// Green-tinged greys with an acid-green accent. Near-square corners; terminal-adjacent.
    Signal,
    /// Cool-magenta greys with a hot-pink accent. The roundest preset; for creative apps.
    Flux,
    /// Icy blue-greys with a cyan accent. Light and airy.
    Glacier,
    /// Indigo-cast greys with an electric violet accent. Deep and a little futuristic.
    Voltage,
}

impl Preset {
    /// The designed presets, in gallery order (the stock theme excluded).
    pub const PREMIUM: [Preset; 6] = [
        Preset::Graphite,
        Preset::Ember,
        Preset::Signal,
        Preset::Flux,
        Preset::Glacier,
        Preset::Voltage,
    ];

    /// Every preset including the stock one.
    pub const ALL: [Preset; 7] = [
        Preset::Alloy,
        Preset::Graphite,
        Preset::Ember,
        Preset::Signal,
        Preset::Flux,
        Preset::Glacier,
        Preset::Voltage,
    ];

    /// Display name.
    pub const fn name(self) -> &'static str {
        match self {
            Preset::Alloy => "Alloy",
            Preset::Graphite => "Graphite",
            Preset::Ember => "Ember",
            Preset::Signal => "Signal",
            Preset::Flux => "Flux",
            Preset::Glacier => "Glacier",
            Preset::Voltage => "Voltage",
        }
    }

    /// One line on what the preset is for.
    pub const fn tagline(self) -> &'static str {
        match self {
            Preset::Alloy => "Calm deep teal. The default.",
            Preset::Graphite => "Cool steel, cobalt accent.",
            Preset::Ember => "Charcoal, signal orange.",
            Preset::Signal => "Green-tinged greys, acid green.",
            Preset::Flux => "Magenta-cast greys, hot pink.",
            Preset::Glacier => "Icy blue-greys, cyan.",
            Preset::Voltage => "Indigo-cast greys, electric violet.",
        }
    }

    /// The brand colour (before it is nudged to pass AA) for an appearance.
    pub const fn brand(self, appearance: Appearance) -> Color {
        let dark = matches!(appearance, Appearance::Dark);
        Color::hex(match (self, dark) {
            (Preset::Alloy, false) => 0x0B6B5F,
            (Preset::Alloy, true) => 0x4DBFAE,
            (Preset::Graphite, false) => 0x1F5FD6,
            (Preset::Graphite, true) => 0x6FA5FF,
            (Preset::Ember, false) => 0xC2410C,
            (Preset::Ember, true) => 0xFF8A4C,
            (Preset::Signal, false) => 0x3A7D0A,
            (Preset::Signal, true) => 0xA3E635,
            (Preset::Flux, false) => 0xC01575,
            (Preset::Flux, true) => 0xFF6FB5,
            (Preset::Glacier, false) => 0x0B78A8,
            (Preset::Glacier, true) => 0x4CC9F0,
            (Preset::Voltage, false) => 0x5B3FE0,
            (Preset::Voltage, true) => 0xA48BFF,
        })
    }

    /// Neutral hue (degrees) and saturation (0..1) the greys are tinted with.
    const fn neutral(self) -> (f32, f32) {
        match self {
            Preset::Alloy => (180.0, 0.0), // unused: the stock neutrals are kept
            Preset::Graphite => (215.0, 0.12),
            Preset::Ember => (24.0, 0.035),
            Preset::Signal => (140.0, 0.07),
            Preset::Flux => (318.0, 0.08),
            Preset::Glacier => (198.0, 0.12),
            Preset::Voltage => (252.0, 0.14),
        }
    }

    const fn radii(self) -> Radii {
        match self {
            Preset::Alloy => Radii::standard(),
            Preset::Graphite => Radii {
                small: 3.0,
                medium: 6.0,
                large: 8.0,
            },
            Preset::Ember => Radii {
                small: 2.0,
                medium: 3.0,
                large: 4.0,
            },
            Preset::Signal => Radii {
                small: 1.0,
                medium: 2.0,
                large: 3.0,
            },
            Preset::Flux => Radii {
                small: 4.0,
                medium: 8.0,
                large: 12.0,
            },
            Preset::Glacier => Radii {
                small: 3.0,
                medium: 5.0,
                large: 7.0,
            },
            Preset::Voltage => Radii {
                small: 3.0,
                medium: 7.0,
                large: 10.0,
            },
        }
    }
}

fn to_hsl(c: Color) -> (f32, f32, f32) {
    let (mx, mn) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let l = (mx + mn) / 2.0;
    if (mx - mn).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if mx == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if mx == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    (h * 60.0, s, l)
}

fn from_hsl(h: f32, s: f32, l: f32, a: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    Color {
        r: r + m,
        g: g + m,
        b: b + m,
        a,
    }
}

/// `c` keeping its lightness and alpha, with the preset's hue and saturation.
fn tint(c: Color, hue: f32, sat: f32) -> Color {
    let (_, _, l) = to_hsl(c);
    from_hsl(hue, sat, l, c.a)
}

/// Push `fg` away from the surfaces until it reaches `min` contrast on all of them.
fn ensure(fg: Color, grounds: [Color; 3], dark: bool, min: f32) -> Color {
    let toward = if dark {
        Color::hex(0xFFFFFF)
    } else {
        Color::hex(0x000000)
    };
    for i in 0..=20 {
        let c = mix(fg, toward, i as f32 * 0.05);
        if grounds.iter().all(|g| contrast_ratio(c, *g) >= min) {
            return c;
        }
    }
    toward
}

impl Theme {
    /// The theme for a preset and appearance.
    pub fn preset(preset: Preset, appearance: Appearance) -> Theme {
        let mut t = Theme::for_appearance(appearance);
        let dark = appearance == Appearance::Dark;
        if preset != Preset::Alloy {
            let (h, s) = preset.neutral();
            // Dark surfaces take a little more of the tint than light ones: a tint is
            // hard to see at 8% lightness.
            let s = if dark { s * 1.25 } else { s };
            let p = &mut t.palette;
            p.background = tint(p.background, h, s);
            p.surface = tint(p.surface, h, s);
            p.surface_raised = tint(p.surface_raised, h, s * 0.5);
            p.border = tint(p.border, h, s);
            p.border_control = tint(p.border_control, h, s);
            p.border_strong = tint(p.border_strong, h, s);
            p.text = tint(p.text, h, s);
            p.text_secondary = tint(p.text_secondary, h, s);
            p.text_disabled = tint(p.text_disabled, h, s);
            let g = [p.background, p.surface, p.surface_raised];
            p.text = ensure(p.text, g, dark, 7.0);
            p.text_secondary = ensure(p.text_secondary, g, dark, 4.5);
            p.border_strong = ensure(p.border_strong, g, dark, 3.0);
            p.text_disabled = ensure(p.text_disabled, g, dark, 3.0);
            let ov = if dark { 0.93 } else { 0.09 };
            let ink = from_hsl(h, s, ov, 1.0);
            let (hv, pr) = if dark { (0.07, 0.13) } else { (0.06, 0.12) };
            p.hover = ink.with_alpha(hv);
            p.pressed = ink.with_alpha(pr);
            t.radii = preset.radii();
        }
        t.with_accent(preset.brand(appearance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Vec<(Preset, Theme)> {
        let mut v = vec![];
        for p in Preset::ALL {
            for a in [Appearance::Light, Appearance::Dark] {
                v.push((p, Theme::preset(p, a)));
            }
        }
        v
    }

    #[test]
    fn every_preset_passes_aa_in_both_appearances() {
        for (preset, t) in all() {
            let p = &t.palette;
            for g in [p.background, p.surface, p.surface_raised] {
                for (name, fg, min) in [
                    ("text", p.text, 7.0),
                    ("secondary", p.text_secondary, 4.5),
                    ("accent", p.accent, 4.5),
                    ("danger", p.danger, 4.5),
                    ("success", p.success, 4.5),
                    ("warning", p.warning, 4.5),
                    ("disabled", p.text_disabled, 3.0),
                    ("border_strong", p.border_strong, 3.0),
                    ("focus", p.focus_ring, 3.0),
                ] {
                    let r = contrast_ratio(fg, g);
                    assert!(r >= min, "{preset:?} {:?} {name} {r:.2}", t.appearance);
                }
            }
            for bg in [p.accent, p.accent_hover, p.accent_pressed] {
                assert!(
                    contrast_ratio(p.on_accent, bg) >= 4.5,
                    "{preset:?} on_accent"
                );
            }
            assert!(
                contrast_ratio(p.text, p.selection) >= 4.5,
                "{preset:?} selection"
            );
        }
    }

    #[test]
    fn presets_are_distinct_and_keep_their_accent_family() {
        let themes = all();
        for (i, (a, ta)) in themes.iter().enumerate() {
            for (b, tb) in themes.iter().skip(i + 1) {
                if a != b && ta.appearance == tb.appearance {
                    assert_ne!(ta.palette.accent, tb.palette.accent, "{a:?} vs {b:?}");
                    assert_ne!(
                        ta.palette.background, tb.palette.background,
                        "{a:?} vs {b:?}"
                    );
                }
            }
        }
        let g = Theme::preset(Preset::Graphite, Appearance::Light)
            .palette
            .accent;
        assert!(g.b > g.r && g.b > g.g, "graphite stays blue");
        let e = Theme::preset(Preset::Ember, Appearance::Dark)
            .palette
            .accent;
        assert!(e.r > e.b, "ember stays orange");
    }

    #[test]
    fn neutrals_are_never_beige() {
        // Beige and cream read as warm, yellow-ish and light. No light background may be
        // both warm (r > b by a visible margin) and yellow (g > b).
        for (preset, t) in all() {
            let b = t.palette.background;
            if t.appearance == Appearance::Light {
                assert!(
                    !(b.r - b.b > 0.04 && b.g > b.b + 0.02),
                    "{preset:?} background is beige: {b:?}"
                );
            }
        }
    }

    #[test]
    fn stock_preset_is_the_stock_theme() {
        assert_eq!(
            Theme::preset(Preset::Alloy, Appearance::Dark),
            Theme::dark().with_accent(Preset::Alloy.brand(Appearance::Dark))
        );
    }

    #[test]
    fn radii_are_small_and_ordered() {
        for (preset, t) in all() {
            let r = t.radii;
            assert!(r.small <= r.medium && r.medium <= r.large, "{preset:?}");
            assert!(r.large <= 12.0);
        }
    }
}
