//! Alloy theming: palette, type scale, spacing, radii, elevation and motion.
//!
//! A [`Theme`] bundles every visual token widgets need. Two designed themes
//! ship: [`Theme::light`] and [`Theme::dark`]; [`Theme::for_appearance`]
//! picks one, and [`ThemeMode::resolve`] turns a user preference (including
//! "follow the system") into a concrete [`Appearance`].
//!
//! # Design reasoning
//!
//! The rules in `DESIGN.md` drive every choice here.
//!
//! * **One accent, deep teal-green** (`#0B6B5F` light, `#4DBFAE` dark). Alloy
//!   is a tool framework, so the accent should read as "calm, active" and not
//!   as a brand shout. Teal-green sits well away from the purple-blue
//!   gradient/neon territory and from the default system blue that every
//!   other toolkit uses, yet it is conventional enough to read as "primary
//!   action". It is used sparingly: primary buttons, focus, selection, links.
//! * **Neutrals are slightly cool greys**, not cream or beige, and carry a hint
//!   of the accent hue so accent and neutrals feel related.
//! * **Hierarchy by size and weight** with a single family stack; no second
//!   font is needed. Body is 14px on a 1.5 line height.
//! * **Spacing is non-uniform** (tight within groups, generous between).
//! * **Radii are small**: 2/4/6px, never pill-shaped by default.
//! * **Shadows only express elevation**; in dark mode they are deeper and
//!   surfaces get lighter instead.
//! * **Motion is short and eased out**; no overshoot, no bounce.
//! * **Contrast**: every text/background pair is tested for WCAG AA
//!   (4.5:1 body, 3:1 large text and UI components) in the unit tests.
#![warn(missing_docs)]

use alloy_ui_core::{Color, Point, Shadow};

/// Light or dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Appearance {
    /// Light.
    Light,
    /// Dark.
    Dark,
}

/// A user's appearance preference, which may defer to the operating system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ThemeMode {
    /// Always light.
    Light,
    /// Always dark.
    Dark,
    /// Follow the OS setting (the default).
    #[default]
    System,
}

impl ThemeMode {
    /// Resolve to a concrete appearance given what the OS currently reports.
    pub const fn resolve(self, system: Appearance) -> Appearance {
        match self {
            ThemeMode::Light => Appearance::Light,
            ThemeMode::Dark => Appearance::Dark,
            ThemeMode::System => system,
        }
    }
}

/// Resolved colour roles used by widgets.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Window background.
    pub background: Color,
    /// Default surface for panels and content areas.
    pub surface: Color,
    /// Raised surface: menus, popovers, dialogs, hovered rows.
    pub surface_raised: Color,
    /// Subtle divider / decorative border (not required to reach 3:1).
    pub border: Color,
    /// Border of interactive controls (inputs, checkboxes); reaches 3:1.
    pub border_strong: Color,
    /// Primary text.
    pub text: Color,
    /// Secondary text (captions, hints).
    pub text_secondary: Color,
    /// Disabled text; exempt from AA but kept above 3:1 for legibility.
    pub text_disabled: Color,
    /// Accent.
    pub accent: Color,
    /// Accent while hovered.
    pub accent_hover: Color,
    /// Accent while pressed.
    pub accent_pressed: Color,
    /// Text or icon drawn on top of an accent fill.
    pub on_accent: Color,
    /// Destructive / error colour (usable as text on the surfaces).
    pub danger: Color,
    /// Success colour (usable as text on the surfaces).
    pub success: Color,
    /// Keyboard focus ring.
    pub focus_ring: Color,
    /// Selection background (text selection, selected rows); primary text
    /// stays readable on it.
    pub selection: Color,
}

impl Palette {
    /// The designed light palette.
    pub const fn light() -> Self {
        Self {
            background: Color::hex(0xF2F4F4),
            surface: Color::hex(0xFAFBFB),
            surface_raised: Color::hex(0xFFFFFF),
            border: Color::hex(0xD5D9D9),
            border_strong: Color::hex(0x737A7A),
            text: Color::hex(0x161A1A),
            text_secondary: Color::hex(0x474E4E),
            text_disabled: Color::hex(0x717878),
            accent: Color::hex(0x0B6B5F),
            accent_hover: Color::hex(0x095A50),
            accent_pressed: Color::hex(0x074A41),
            on_accent: Color::hex(0xFFFFFF),
            danger: Color::hex(0xB3261E),
            success: Color::hex(0x1B7A3A),
            focus_ring: Color::hex(0x0B6B5F),
            selection: Color::hex(0xCDE6E1),
        }
    }

    /// The designed dark palette.
    pub const fn dark() -> Self {
        Self {
            background: Color::hex(0x121515),
            surface: Color::hex(0x1A1E1E),
            surface_raised: Color::hex(0x242929),
            border: Color::hex(0x343A3A),
            border_strong: Color::hex(0x7E8686),
            text: Color::hex(0xEDF0F0),
            text_secondary: Color::hex(0xA9B1B1),
            text_disabled: Color::hex(0x7B8383),
            accent: Color::hex(0x4DBFAE),
            accent_hover: Color::hex(0x6BD0C0),
            accent_pressed: Color::hex(0x3AA897),
            on_accent: Color::hex(0x06201B),
            danger: Color::hex(0xF28078),
            success: Color::hex(0x5CC47E),
            focus_ring: Color::hex(0x6BD0C0),
            selection: Color::hex(0x1F4D46),
        }
    }

    /// Palette for an appearance.
    pub const fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self::light(),
            Appearance::Dark => Self::dark(),
        }
    }
}

/// Font weight on the usual 100..=900 scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontWeight(pub u16);

impl FontWeight {
    /// Regular (400).
    pub const REGULAR: Self = Self(400);
    /// Medium (500).
    pub const MEDIUM: Self = Self(500);
    /// Semibold (600).
    pub const SEMIBOLD: Self = Self(600);
    /// Bold (700).
    pub const BOLD: Self = Self(700);
}

/// One step of the type scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    /// Font size in logical pixels.
    pub size: f32,
    /// Weight.
    pub weight: FontWeight,
    /// Line height in logical pixels.
    pub line_height: f32,
    /// Letter spacing in em (negative tightens large headings).
    pub letter_spacing: f32,
}

impl TextStyle {
    const fn new(size: f32, weight: FontWeight, line_height: f32, letter_spacing: f32) -> Self {
        Self {
            size,
            weight,
            line_height,
            letter_spacing,
        }
    }

    /// Whether WCAG treats this as large text (>= 18.66px bold or >= 24px).
    pub fn is_large(&self) -> bool {
        self.size >= 24.0 || (self.size >= 18.66 && self.weight.0 >= 700)
    }
}

/// Type scale: hierarchy comes from size and weight in one family.
#[derive(Debug, Clone, PartialEq)]
pub struct Typography {
    /// Preferred UI font family stack (system UI first).
    pub family: &'static [&'static str],
    /// Monospace family stack.
    pub mono_family: &'static [&'static str],
    /// Page / window title.
    pub display: TextStyle,
    /// Section heading.
    pub heading: TextStyle,
    /// Sub-heading.
    pub subheading: TextStyle,
    /// Default body text (14px, 1.5 line height).
    pub body: TextStyle,
    /// Emphasised body text (labels, buttons).
    pub label: TextStyle,
    /// Captions and hints.
    pub caption: TextStyle,
    /// Monospace code text.
    pub code: TextStyle,
}

impl Typography {
    /// The default scale (shared by light and dark).
    pub const fn standard() -> Self {
        Self {
            family: &[
                "system-ui",
                "Segoe UI",
                "SF Pro Text",
                "Cantarell",
                "Noto Sans",
                "sans-serif",
            ],
            mono_family: &[
                "ui-monospace",
                "Cascadia Mono",
                "Menlo",
                "DejaVu Sans Mono",
                "monospace",
            ],
            display: TextStyle::new(28.0, FontWeight::SEMIBOLD, 34.0, -0.01),
            heading: TextStyle::new(20.0, FontWeight::SEMIBOLD, 28.0, -0.005),
            subheading: TextStyle::new(16.0, FontWeight::SEMIBOLD, 24.0, 0.0),
            body: TextStyle::new(14.0, FontWeight::REGULAR, 21.0, 0.0),
            label: TextStyle::new(14.0, FontWeight::MEDIUM, 20.0, 0.0),
            caption: TextStyle::new(12.0, FontWeight::REGULAR, 18.0, 0.01),
            code: TextStyle::new(13.0, FontWeight::REGULAR, 20.0, 0.0),
        }
    }
}

/// Spacing scale in logical pixels. Deliberately non-uniform: steps grow
/// faster than linearly so groups can be tight and sections generous.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Hairline gap, e.g. icon to label.
    pub xxs: f32,
    /// Tight, within a group.
    pub xs: f32,
    /// Related controls.
    pub sm: f32,
    /// Default padding.
    pub md: f32,
    /// Between groups.
    pub lg: f32,
    /// Between sections.
    pub xl: f32,
    /// Page margins and major breaks.
    pub xxl: f32,
}

impl Spacing {
    /// The default rhythm: 2, 4, 8, 14, 24, 40, 64.
    pub const fn standard() -> Self {
        Self {
            xxs: 2.0,
            xs: 4.0,
            sm: 8.0,
            md: 14.0,
            lg: 24.0,
            xl: 40.0,
            xxl: 64.0,
        }
    }
}

/// Corner radii in logical pixels; small and restrained.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radii {
    /// Checkboxes, tags, small chips.
    pub small: f32,
    /// Buttons, inputs.
    pub medium: f32,
    /// Panels, popovers, dialogs.
    pub large: f32,
}

impl Radii {
    /// The default radii: 2, 4, 6.
    pub const fn standard() -> Self {
        Self {
            small: 2.0,
            medium: 4.0,
            large: 6.0,
        }
    }
}

/// Shadows for real elevation only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elevation {
    /// Flat on the surface; no shadow.
    pub none: Option<Shadow>,
    /// Slightly lifted: hovered cards, sticky bars.
    pub low: Shadow,
    /// Floating: menus, popovers, tooltips.
    pub medium: Shadow,
    /// Modal: dialogs.
    pub high: Shadow,
}

impl Elevation {
    const fn shadow(alpha: f32, dy: f32, blur: f32) -> Shadow {
        Shadow {
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: alpha,
            },
            offset: Point { x: 0.0, y: dy },
            blur,
        }
    }

    /// Shadows tuned for light surfaces.
    pub const fn light() -> Self {
        Self {
            none: None,
            low: Self::shadow(0.08, 1.0, 3.0),
            medium: Self::shadow(0.12, 4.0, 12.0),
            high: Self::shadow(0.18, 10.0, 28.0),
        }
    }

    /// Shadows tuned for dark surfaces (stronger, as they are harder to see).
    pub const fn dark() -> Self {
        Self {
            none: None,
            low: Self::shadow(0.30, 1.0, 3.0),
            medium: Self::shadow(0.40, 4.0, 14.0),
            high: Self::shadow(0.55, 10.0, 32.0),
        }
    }
}

/// A cubic-bezier easing curve, `(x1, y1, x2, y2)`, as in CSS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing(pub f32, pub f32, pub f32, pub f32);

impl Easing {
    /// Ease-out: fast start, gentle settle. Control points keep y within
    /// 0..=1 so there is never overshoot.
    pub const EASE_OUT: Self = Self(0.16, 1.0, 0.3, 1.0);
    /// Ease-in-out for symmetric movements; also never overshoots.
    pub const EASE_IN_OUT: Self = Self(0.4, 0.0, 0.2, 1.0);
    /// Linear, for progress indicators.
    pub const LINEAR: Self = Self(0.0, 0.0, 1.0, 1.0);

    /// Evaluate the curve at progress `t` in 0..=1 (clamped).
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let Self(x1, y1, x2, y2) = self;
        let bez = |a: f32, b: f32, s: f32| {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        };
        // Solve x(s) = t by bisection.
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = 0.5 * (lo + hi);
            if bez(x1, x2, mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        bez(y1, y2, 0.5 * (lo + hi))
    }
}

/// Motion tokens. Only for meaningful state change; short and eased out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    /// Hover / press feedback, in milliseconds.
    pub fast_ms: u32,
    /// Default state change (toggle, focus), in milliseconds.
    pub normal_ms: u32,
    /// Entering/leaving surfaces (menus, dialogs), in milliseconds.
    pub slow_ms: u32,
    /// Default easing for everything.
    pub easing: Easing,
}

impl Motion {
    /// The default motion: 90, 150, 240 ms, ease-out.
    pub const fn standard() -> Self {
        Self {
            fast_ms: 90,
            normal_ms: 150,
            slow_ms: 240,
            easing: Easing::EASE_OUT,
        }
    }
}

/// Every visual token a widget needs.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Which appearance this theme is.
    pub appearance: Appearance,
    /// Colour roles.
    pub palette: Palette,
    /// Type scale.
    pub typography: Typography,
    /// Spacing scale.
    pub spacing: Spacing,
    /// Corner radii.
    pub radii: Radii,
    /// Elevation shadows.
    pub elevation: Elevation,
    /// Motion tokens.
    pub motion: Motion,
}

impl Theme {
    /// The designed light theme.
    pub const fn light() -> Self {
        Self {
            appearance: Appearance::Light,
            palette: Palette::light(),
            typography: Typography::standard(),
            spacing: Spacing::standard(),
            radii: Radii::standard(),
            elevation: Elevation::light(),
            motion: Motion::standard(),
        }
    }

    /// The designed dark theme.
    pub const fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            palette: Palette::dark(),
            typography: Typography::standard(),
            spacing: Spacing::standard(),
            radii: Radii::standard(),
            elevation: Elevation::dark(),
            motion: Motion::standard(),
        }
    }

    /// The theme for an appearance.
    pub const fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self::light(),
            Appearance::Dark => Self::dark(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

/// WCAG 2.x relative luminance of an opaque sRGB colour.
pub fn relative_luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG contrast ratio (1.0..=21.0) between two opaque colours.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn themes() -> [Theme; 2] {
        [Theme::light(), Theme::dark()]
    }

    #[test]
    fn body_text_meets_aa() {
        for t in themes() {
            let p = &t.palette;
            let grounds = [p.background, p.surface, p.surface_raised];
            for g in grounds {
                for (name, fg) in [
                    ("text", p.text),
                    ("text_secondary", p.text_secondary),
                    ("accent", p.accent),
                    ("danger", p.danger),
                    ("success", p.success),
                ] {
                    let r = contrast_ratio(fg, g);
                    assert!(r >= 4.5, "{:?} {name} on {g:?}: {r:.2}", t.appearance);
                }
            }
            for (name, bg) in [
                ("accent", p.accent),
                ("accent_hover", p.accent_hover),
                ("accent_pressed", p.accent_pressed),
            ] {
                let r = contrast_ratio(p.on_accent, bg);
                assert!(r >= 4.5, "{:?} on_accent on {name}: {r:.2}", t.appearance);
            }
            let r = contrast_ratio(p.text, p.selection);
            assert!(r >= 4.5, "{:?} text on selection: {r:.2}", t.appearance);
        }
    }

    #[test]
    fn ui_elements_meet_3_to_1() {
        for t in themes() {
            let p = &t.palette;
            for g in [p.background, p.surface, p.surface_raised] {
                for (name, fg) in [
                    ("border_strong", p.border_strong),
                    ("focus_ring", p.focus_ring),
                    ("text_disabled", p.text_disabled),
                    ("accent_hover", p.accent_hover),
                    ("accent_pressed", p.accent_pressed),
                ] {
                    // Hover/pressed accents are only drawn as fills/text on
                    // surfaces in the matching state; 3:1 is the UI floor.
                    let r = contrast_ratio(fg, g);
                    assert!(r >= 3.0, "{:?} {name} on {g:?}: {r:.2}", t.appearance);
                }
            }
        }
    }

    #[test]
    fn contrast_math_is_correct() {
        let black = Color::hex(0x000000);
        let white = Color::hex(0xFFFFFF);
        assert!((contrast_ratio(black, white) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn type_scale_is_a_hierarchy() {
        let ty = Typography::standard();
        assert!(ty.display.size > ty.heading.size);
        assert!(ty.heading.size > ty.subheading.size);
        assert!(ty.subheading.size > ty.body.size);
        assert!(ty.body.size > ty.caption.size);
        assert!((14.0..=16.0).contains(&ty.body.size));
        let ratio = ty.body.line_height / ty.body.size;
        assert!((1.4..=1.6).contains(&ratio));
        for s in [
            ty.display,
            ty.heading,
            ty.subheading,
            ty.body,
            ty.label,
            ty.caption,
            ty.code,
        ] {
            assert!(s.line_height >= s.size * 1.2);
        }
        assert!(ty.label.weight > ty.body.weight);
    }

    #[test]
    fn spacing_is_non_uniform_and_increasing() {
        let s = Spacing::standard();
        let v = [s.xxs, s.xs, s.sm, s.md, s.lg, s.xl, s.xxl];
        assert!(v.windows(2).all(|w| w[0] < w[1]));
        let diffs: Vec<f32> = v.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(diffs.windows(2).any(|d| (d[0] - d[1]).abs() > 1.0));
    }

    #[test]
    fn radii_are_small() {
        let r = Radii::standard();
        assert!(r.small < r.medium && r.medium < r.large && r.large <= 8.0);
    }

    #[test]
    fn elevation_grows() {
        for e in [Elevation::light(), Elevation::dark()] {
            assert!(e.none.is_none());
            assert!(e.low.blur < e.medium.blur && e.medium.blur < e.high.blur);
            assert!(e.low.offset.y < e.high.offset.y);
            assert!(e.low.color.a < e.high.color.a);
        }
    }

    #[test]
    fn easing_never_overshoots_and_hits_ends() {
        for e in [Easing::EASE_OUT, Easing::EASE_IN_OUT, Easing::LINEAR] {
            assert!(e.at(0.0).abs() < 1e-3);
            assert!((e.at(1.0) - 1.0).abs() < 1e-3);
            let mut prev = 0.0;
            for i in 0..=100 {
                let y = e.at(i as f32 / 100.0);
                assert!((-1e-3..=1.0 + 1e-3).contains(&y));
                assert!(y + 1e-3 >= prev);
                prev = y;
            }
        }
        // Ease-out is front-loaded.
        assert!(Easing::EASE_OUT.at(0.3) > 0.5);
    }

    #[test]
    fn motion_is_short() {
        let m = Motion::standard();
        assert!(m.fast_ms < m.normal_ms && m.normal_ms < m.slow_ms && m.slow_ms <= 300);
    }

    #[test]
    fn mode_resolution() {
        assert_eq!(
            ThemeMode::System.resolve(Appearance::Dark),
            Appearance::Dark
        );
        assert_eq!(
            ThemeMode::Light.resolve(Appearance::Dark),
            Appearance::Light
        );
        assert_eq!(ThemeMode::Dark.resolve(Appearance::Light), Appearance::Dark);
        assert_eq!(
            Theme::for_appearance(Appearance::Dark).appearance,
            Appearance::Dark
        );
    }

    #[test]
    fn palettes_are_opaque_and_themes_differ() {
        for t in themes() {
            let p = &t.palette;
            for c in [p.background, p.surface, p.surface_raised, p.text, p.accent] {
                assert_eq!(c.a, 1.0);
            }
        }
        assert!(
            relative_luminance(Palette::light().background)
                > relative_luminance(Palette::dark().background)
        );
    }
}
