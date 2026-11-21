//! Theme variants: brand accent, high contrast and density presets.
//!
//! Each is a pure function from a [`Theme`] to a [`Theme`], so they compose:
//! `Theme::dark().with_accent(brand).density(Density::Compact)`. Switching at
//! runtime is `ui.set_theme(..)`; nothing is cached against the old theme.

use crate::{contrast_ratio, Appearance, Spacing, Theme};
use alloy_ui_core::Color;

/// Linear mix of two opaque colours: `t = 0` is `a`, `t = 1` is `b`.
pub fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Color {
        r: l(a.r, b.r),
        g: l(a.g, b.g),
        b: l(a.b, b.b),
        a: 1.0,
    }
}

/// How much room controls and layouts take. Scales the spacing tokens, which
/// buttons, lists, tables and panels read for their padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Density {
    /// Tight rows for data-heavy tools (about 80% of the default).
    Compact,
    /// The designed rhythm.
    #[default]
    Comfortable,
    /// Roomy, for touch and presentation (about 125%).
    Spacious,
}

impl Density {
    /// The factor applied to every spacing step.
    pub const fn scale(self) -> f32 {
        match self {
            Self::Compact => 0.8,
            Self::Comfortable => 1.0,
            Self::Spacious => 1.25,
        }
    }
}

const BLACK: Color = Color::hex(0x000000);
const WHITE: Color = Color::hex(0xFFFFFF);

/// Push `c` towards black (light themes) or white (dark themes) until it has
/// at least `min` contrast against every colour in `grounds`.
fn legible(c: Color, grounds: &[Color], dark: bool, min: f32) -> Color {
    let toward = if dark { WHITE } else { BLACK };
    let mut out = c;
    for i in 0..=20 {
        out = mix(c, toward, i as f32 * 0.05);
        if grounds.iter().all(|g| contrast_ratio(out, *g) >= min) {
            break;
        }
    }
    out
}

fn best_on(bg: [Color; 3]) -> Color {
    let ok = |fg| bg.iter().all(|b| contrast_ratio(fg, *b) >= 4.5);
    let ink = Color::hex(0x0B0F0F);
    if ok(WHITE) {
        WHITE
    } else if ok(ink) {
        ink
    } else if bg.iter().map(|b| contrast_ratio(WHITE, *b)).sum::<f32>()
        > bg.iter().map(|b| contrast_ratio(BLACK, *b)).sum::<f32>()
    {
        WHITE
    } else {
        BLACK
    }
}

impl Theme {
    /// Re-derive every accent role from a brand colour. The brand colour is
    /// nudged (never recoloured) until it passes WCAG AA on the theme's
    /// surfaces, so any input yields a readable accent; `on_accent` is picked
    /// for contrast on the accent and both of its states.
    pub fn with_accent(mut self, brand: Color) -> Self {
        let dark = self.appearance == Appearance::Dark;
        let p = &mut self.palette;
        let grounds = [p.background, p.surface, p.surface_raised];
        let accent = legible(brand, &grounds, dark, 4.5);
        let toward = if dark { WHITE } else { BLACK };
        let hover = legible(mix(accent, toward, 0.12), &grounds, dark, 4.5);
        let pressed = legible(mix(accent, toward, 0.24), &grounds, dark, 4.5);
        // A dark theme's hover brightens; keep the three states ordered.
        let (hover, pressed) = if dark {
            (mix(accent, WHITE, 0.14), mix(accent, BLACK, 0.12))
        } else {
            (hover, pressed)
        };
        p.accent = accent;
        p.accent_hover = hover;
        p.accent_pressed = pressed;
        p.on_accent = best_on([accent, hover, pressed]);
        p.focus_ring = if dark { hover } else { accent };
        let tint = mix(p.surface_raised, accent, if dark { 0.30 } else { 0.18 });
        p.selection = tint;
        // Selected rows keep primary text readable: wash the tint out if not.
        let mut t = if dark { 0.30 } else { 0.18 };
        while contrast_ratio(p.text, p.selection) < 4.5 && t > 0.02 {
            t -= 0.02;
            p.selection = mix(p.surface_raised, accent, t);
        }
        self
    }

    /// A high-contrast variant of this theme's appearance: pure black on
    /// white (or the reverse), 7:1 text, solid borders, and a bold focus ring.
    pub fn high_contrast(mut self) -> Self {
        let dark = self.appearance == Appearance::Dark;
        let (bg, fg) = if dark { (BLACK, WHITE) } else { (WHITE, BLACK) };
        let accent = if dark {
            Color::hex(0x7FE8D6)
        } else {
            Color::hex(0x00453D)
        };
        let p = &mut self.palette;
        p.background = bg;
        p.surface = bg;
        p.surface_raised = bg;
        p.text = fg;
        p.text_secondary = fg;
        p.text_disabled = mix(fg, bg, 0.35);
        p.border = fg;
        p.border_strong = fg;
        p.border_control = fg;
        p.accent = accent;
        p.accent_hover = mix(accent, fg, 0.2);
        p.accent_pressed = mix(accent, fg, 0.4);
        p.on_accent = bg;
        p.focus_ring = if dark {
            Color::hex(0xFFE14D)
        } else {
            Color::hex(0x0033CC)
        };
        p.danger = if dark {
            Color::hex(0xFF9C95)
        } else {
            Color::hex(0x8A0F08)
        };
        p.success = if dark {
            Color::hex(0x8BE8A8)
        } else {
            Color::hex(0x0A5A24)
        };
        p.warning = if dark {
            Color::hex(0xFFD066)
        } else {
            Color::hex(0x6B3B00)
        };
        p.selection = if dark {
            Color::hex(0x1A4F9C)
        } else {
            Color::hex(0xFFE14D)
        };
        p.hover = fg.with_alpha(0.14);
        p.pressed = fg.with_alpha(0.26);
        self
    }

    /// Scale the spacing tokens for `density`.
    pub fn density(mut self, density: Density) -> Self {
        let k = density.scale();
        let s = Spacing::standard();
        let r = |v: f32| (v * k).round().max(1.0);
        self.spacing = Spacing {
            xxs: r(s.xxs),
            xs: r(s.xs),
            sm: r(s.sm),
            md: r(s.md),
            lg: r(s.lg),
            xl: r(s.xl),
            xxl: r(s.xxl),
        };
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aa(t: &Theme) {
        let p = &t.palette;
        for g in [p.background, p.surface, p.surface_raised] {
            for fg in [p.accent, p.text, p.warning, p.danger, p.success] {
                assert!(contrast_ratio(fg, g) >= 4.5, "{:?}", t.appearance);
            }
            assert!(contrast_ratio(p.focus_ring, g) >= 3.0);
        }
        for bg in [p.accent, p.accent_hover, p.accent_pressed] {
            assert!(contrast_ratio(p.on_accent, bg) >= 4.5);
        }
        assert!(contrast_ratio(p.text, p.selection) >= 4.5);
    }

    #[test]
    fn any_brand_colour_stays_readable() {
        for brand in [
            0xFFE600u32,
            0x00FFFF,
            0xFF00AA,
            0x3366FF,
            0x111111,
            0xFFFFFF,
        ] {
            for base in [Theme::light(), Theme::dark()] {
                aa(&base.with_accent(Color::hex(brand)));
            }
        }
    }

    #[test]
    fn accent_keeps_the_hue() {
        let t = Theme::light().with_accent(Color::hex(0x3366FF));
        assert!(t.palette.accent.b > t.palette.accent.r);
    }

    #[test]
    fn high_contrast_is_7_to_1() {
        for base in [Theme::light(), Theme::dark()] {
            let t = base.high_contrast();
            aa(&t);
            let p = &t.palette;
            assert!(contrast_ratio(p.text, p.background) >= 7.0);
            assert!(contrast_ratio(p.accent, p.background) >= 7.0);
            assert!(contrast_ratio(p.border, p.background) >= 7.0);
        }
    }

    #[test]
    fn density_scales_and_stays_ordered() {
        let c = Theme::light().density(Density::Compact).spacing;
        let n = Theme::light().density(Density::Comfortable).spacing;
        let s = Theme::light().density(Density::Spacious).spacing;
        assert_eq!(n, Spacing::standard());
        assert!(c.md < n.md && n.md < s.md);
        for sp in [c, s] {
            let v = [sp.xxs, sp.xs, sp.sm, sp.md, sp.lg, sp.xl, sp.xxl];
            assert!(v.windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn stock_warning_is_readable() {
        aa(&Theme::light());
        aa(&Theme::dark());
    }
}
