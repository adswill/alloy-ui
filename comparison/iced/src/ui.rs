//! Shared look for the three apps: palette, style functions and a couple of
//! small composite widgets. Everything is plain iced; styling goes through the
//! widgets' `style` closures and a custom `Theme`.
#![allow(dead_code)]

use iced::border::Radius;
use iced::widget::{
    button, container, pick_list, row, slider, text, text_editor, text_input, toggler,
};
use iced::widget::{overlay::menu, scrollable};
use iced::{font, Background, Border, Color, Element, Font, Length, Shadow, Theme};

pub const INDIGO: Color = Color::from_rgb(
    0x4f as f32 / 255.0,
    0x6b as f32 / 255.0,
    0xed as f32 / 255.0,
);
pub const RADIUS: f32 = 8.0;

pub const SEMIBOLD: Font = Font {
    weight: font::Weight::Semibold,
    ..Font::DEFAULT
};

fn hex(rgb: u32) -> Color {
    Color::from_rgb8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub fn mix(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// Builds the app theme from a light/dark choice and an accent colour.
pub fn theme(dark: bool, accent: Color) -> Theme {
    let palette = if dark {
        iced::theme::Palette {
            background: hex(0x1d1c1a),
            text: hex(0xecebe6),
            primary: accent,
            success: hex(0x4cb782),
            warning: hex(0xe0a53a),
            danger: hex(0xe5675f),
        }
    } else {
        iced::theme::Palette {
            background: hex(0xf8f6f2),
            text: hex(0x24221e),
            primary: accent,
            success: hex(0x1f8a5b),
            warning: hex(0xb7791f),
            danger: hex(0xc9423a),
        }
    };
    Theme::custom(if dark { "Alloy Dark" } else { "Alloy Light" }, palette)
}

/// Colours derived from the theme's palette; the neutrals follow light/dark.
#[derive(Clone, Copy)]
pub struct Skin {
    pub dark: bool,
    pub page: Color,
    pub sidebar: Color,
    pub card: Color,
    pub hover: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
}

impl Skin {
    pub fn of(theme: &Theme) -> Self {
        let p = theme.palette();
        let dark = p.background.r < 0.5;
        let (sidebar, card, hover, border, muted) = if dark {
            (
                hex(0x151412),
                hex(0x262522),
                hex(0x2e2c28),
                hex(0x38362f),
                hex(0x9a968c),
            )
        } else {
            (
                hex(0xeeebe4),
                hex(0xfffefc),
                hex(0xece9e2),
                hex(0xe2ded5),
                hex(0x7a756b),
            )
        };
        Skin {
            dark,
            page: p.background,
            sidebar,
            card,
            hover,
            border,
            text: p.text,
            muted,
            accent: p.primary,
            accent_soft: mix(card, p.primary, if dark { 0.22 } else { 0.12 }),
            success: p.success,
            warning: p.warning,
            danger: p.danger,
        }
    }
}

fn border(color: Color) -> Border {
    Border {
        color,
        width: 1.0,
        radius: Radius::from(RADIUS),
    }
}

// ---- containers ---------------------------------------------------------

pub fn page(theme: &Theme) -> container::Style {
    let s = Skin::of(theme);
    container::Style::default().color(s.text).background(s.page)
}

pub fn sidebar(theme: &Theme) -> container::Style {
    let s = Skin::of(theme);
    container::Style::default()
        .color(s.text)
        .background(s.sidebar)
        .border(Border {
            color: s.border,
            width: 0.0,
            radius: Radius::from(0.0),
        })
}

pub fn card(theme: &Theme) -> container::Style {
    let s = Skin::of(theme);
    container::Style::default()
        .color(s.text)
        .background(s.card)
        .border(border(s.border))
}

/// A vertical hairline, drawn as a 1 px wide container.
pub fn hairline(theme: &Theme) -> container::Style {
    container::Style::default().background(Skin::of(theme).border)
}

pub fn pill(color: fn(&Skin) -> Color) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let c = color(&Skin::of(theme));
        container::Style::default()
            .color(c)
            .background(with_alpha(c, 0.12))
            .border(Border {
                radius: Radius::from(99.0),
                ..Border::default()
            })
    }
}

pub fn bar(strength: f32) -> impl Fn(&Theme) -> container::Style {
    move |theme| {
        let s = Skin::of(theme);
        container::Style::default()
            .background(mix(s.card, s.accent, strength))
            .border(Border {
                radius: Radius::from(4.0).bottom(0.0),
                ..Border::default()
            })
    }
}

pub fn swatch(color: Color, selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, _| {
        let s = Skin::of(theme);
        button::Style {
            background: Some(Background::Color(color)),
            text_color: Color::WHITE,
            border: Border {
                color: if selected { s.text } else { s.border },
                width: if selected { 2.0 } else { 1.0 },
                radius: Radius::from(99.0),
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

// ---- text ---------------------------------------------------------------

pub fn muted(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(Skin::of(theme).muted),
    }
}

pub fn accent(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(Skin::of(theme).accent),
    }
}

pub fn danger(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(Skin::of(theme).danger),
    }
}

pub fn success(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(Skin::of(theme).success),
    }
}

pub fn heading<'a>(label: impl text::IntoFragment<'a>) -> iced::widget::Text<'a, Theme> {
    text(label).size(20).font(SEMIBOLD)
}

pub fn caption<'a>(label: impl text::IntoFragment<'a>) -> iced::widget::Text<'a, Theme> {
    text(label).size(12).style(muted)
}

// ---- buttons ------------------------------------------------------------

fn base_button(bg: Option<Color>, fg: Color, line: Color) -> button::Style {
    button::Style {
        background: bg.map(Background::Color),
        text_color: fg,
        border: border(line),
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn primary(theme: &Theme, status: button::Status) -> button::Style {
    let s = Skin::of(theme);
    let bg = match status {
        button::Status::Hovered => mix(s.accent, Color::BLACK, 0.1),
        button::Status::Pressed => mix(s.accent, Color::BLACK, 0.2),
        button::Status::Disabled => with_alpha(s.accent, 0.4),
        button::Status::Active => s.accent,
    };
    base_button(Some(bg), Color::WHITE, bg)
}

pub fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let s = Skin::of(theme);
    let bg = match status {
        button::Status::Hovered => s.hover,
        button::Status::Pressed => mix(s.hover, s.border, 0.5),
        _ => s.card,
    };
    base_button(Some(bg), s.text, s.border)
}

pub fn destructive(theme: &Theme, status: button::Status) -> button::Style {
    let s = Skin::of(theme);
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => with_alpha(s.danger, 0.1),
        _ => s.card,
    };
    base_button(Some(bg), s.danger, mix(s.border, s.danger, 0.45))
}

/// Sidebar navigation entry.
pub fn nav(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let s = Skin::of(theme);
        let (bg, fg) = if selected {
            (Some(s.accent_soft), s.accent)
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            (Some(mix(s.sidebar, s.border, 0.6)), s.text)
        } else {
            (None, s.text)
        };
        button::Style {
            border: Border {
                width: 0.0,
                ..border(Color::TRANSPARENT)
            },
            ..base_button(bg, fg, Color::TRANSPARENT)
        }
    }
}

/// A row in a list (notes list): card-like when selected.
pub fn list_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let s = Skin::of(theme);
        if selected {
            base_button(Some(s.card), s.text, s.border)
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            base_button(
                Some(mix(s.sidebar, s.border, 0.5)),
                s.text,
                Color::TRANSPARENT,
            )
        } else {
            base_button(None, s.text, Color::TRANSPARENT)
        }
    }
}

fn segment(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let s = Skin::of(theme);
        let (bg, fg) = if selected {
            (Some(s.card), s.text)
        } else if matches!(status, button::Status::Hovered) {
            (Some(with_alpha(s.card, 0.5)), s.text)
        } else {
            (None, s.muted)
        };
        let line = if selected {
            s.border
        } else {
            Color::TRANSPARENT
        };
        base_button(bg, fg, line)
    }
}

/// Segmented control: a recessed track holding one button per option.
pub fn segmented<'a, T, M>(
    options: &[(&'a str, T)],
    selected: T,
    on_select: impl Fn(T) -> M,
) -> Element<'a, M>
where
    T: Copy + PartialEq + 'a,
    M: Clone + 'a,
{
    let buttons = options.iter().map(|&(label, value)| {
        button(text(label).size(13).center())
            .padding([5, 14])
            .style(segment(selected == value))
            .on_press(on_select(value))
            .into()
    });
    container(row(buttons).spacing(2))
        .padding(2)
        .style(|theme: &Theme| {
            let s = Skin::of(theme);
            container::Style::default()
                .background(mix(s.page, s.border, 0.55))
                .border(Border {
                    radius: Radius::from(RADIUS),
                    ..Border::default()
                })
        })
        .width(Length::Shrink)
        .into()
}

// ---- inputs -------------------------------------------------------------

pub fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let s = Skin::of(theme);
    let line = match status {
        text_input::Status::Focused { .. } => s.accent,
        text_input::Status::Hovered => mix(s.border, s.muted, 0.4),
        _ => s.border,
    };
    text_input::Style {
        background: Background::Color(s.card),
        border: border(line),
        icon: s.muted,
        placeholder: with_alpha(s.muted, 0.8),
        value: s.text,
        selection: with_alpha(s.accent, 0.3),
    }
}

/// Title field: no chrome until focused.
pub fn input_bare(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = input(theme, status);
    style.background = Background::Color(Color::TRANSPARENT);
    style.border = Border {
        color: Color::TRANSPARENT,
        width: 0.0,
        radius: Radius::from(RADIUS),
    };
    style
}

pub fn editor(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let s = Skin::of(theme);
    let line = match status {
        text_editor::Status::Focused { .. } => s.accent,
        _ => s.border,
    };
    text_editor::Style {
        background: Background::Color(s.card),
        border: border(line),
        placeholder: with_alpha(s.muted, 0.8),
        value: s.text,
        selection: with_alpha(s.accent, 0.3),
    }
}

pub fn range(theme: &Theme, status: slider::Status) -> slider::Style {
    let s = Skin::of(theme);
    let handle = match status {
        slider::Status::Active => s.accent,
        slider::Status::Hovered => mix(s.accent, Color::BLACK, 0.1),
        slider::Status::Dragged => mix(s.accent, Color::BLACK, 0.2),
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (Background::Color(s.accent), Background::Color(s.border)),
            width: 4.0,
            border: Border {
                radius: Radius::from(2.0),
                ..Border::default()
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle { radius: 8.0 },
            background: Background::Color(handle),
            border_width: 3.0,
            border_color: s.card,
        },
    }
}

pub fn switch(theme: &Theme, status: toggler::Status) -> toggler::Style {
    let s = Skin::of(theme);
    let on = match status {
        toggler::Status::Active { is_toggled } | toggler::Status::Hovered { is_toggled } => {
            is_toggled
        }
        toggler::Status::Disabled { .. } => false,
    };
    let track = if on {
        s.accent
    } else {
        mix(s.border, s.muted, 0.35)
    };
    toggler::Style {
        background: Background::Color(if matches!(status, toggler::Status::Hovered { .. }) {
            mix(track, s.text, 0.08)
        } else {
            track
        }),
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        foreground: Background::Color(Color::WHITE),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        text_color: Some(s.text),
        border_radius: None,
        padding_ratio: 0.15,
    }
}

pub fn dropdown(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let s = Skin::of(theme);
    let line = match status {
        pick_list::Status::Active => s.border,
        pick_list::Status::Hovered => mix(s.border, s.muted, 0.4),
        pick_list::Status::Opened { .. } => s.accent,
    };
    pick_list::Style {
        text_color: s.text,
        placeholder_color: s.muted,
        handle_color: s.muted,
        background: Background::Color(s.card),
        border: border(line),
    }
}

pub fn dropdown_menu(theme: &Theme) -> menu::Style {
    let s = Skin::of(theme);
    menu::Style {
        background: Background::Color(s.card),
        border: border(s.border),
        text_color: s.text,
        selected_text_color: s.accent,
        selected_background: Background::Color(s.accent_soft),
        shadow: Shadow {
            color: with_alpha(Color::BLACK, 0.12),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
    }
}

/// Thin, quiet scrollbars.
pub fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let s = Skin::of(theme);
    let mut style = scrollable::default(theme, status);
    let rail = scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(with_alpha(s.muted, 0.35)),
            border: Border {
                radius: Radius::from(3.0),
                ..Border::default()
            },
        },
    };
    style.vertical_rail = rail;
    style.horizontal_rail = rail;
    style.container = container::Style::default();
    style
}
