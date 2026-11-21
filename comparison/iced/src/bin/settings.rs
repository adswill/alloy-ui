//! Preferences: sidebar sections with appearance, notification and privacy settings.

#[path = "../ui.rs"]
mod ui;

use std::fmt;

use iced::widget::{
    button, column, container, pick_list, row, scrollable, slider, space, text, toggler,
};
use iced::{Color, Element, Length, Settings, Theme};

fn main() -> iced::Result {
    iced::application(Preferences::default, Preferences::update, Preferences::view)
        .title("Preferences")
        .theme(Preferences::theme)
        .settings(Settings {
            default_text_size: 14.into(),
            ..Settings::default()
        })
        .window_size((960.0, 640.0))
        .centered()
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Appearance,
    Notifications,
    Privacy,
    About,
}

impl Section {
    const ALL: [Section; 4] = [
        Section::Appearance,
        Section::Notifications,
        Section::Privacy,
        Section::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::Appearance => "Appearance",
            Section::Notifications => "Notifications",
            Section::Privacy => "Privacy",
            Section::About => "About",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivery {
    Instantly,
    Hourly,
    Daily,
}

impl Delivery {
    const ALL: [Delivery; 3] = [Delivery::Instantly, Delivery::Hourly, Delivery::Daily];
}

impl fmt::Display for Delivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Delivery::Instantly => "Instantly",
            Delivery::Hourly => "Hourly digest",
            Delivery::Daily => "Daily digest",
        })
    }
}

const ACCENTS: [(&str, Color); 4] = [
    ("Indigo", ui::INDIGO),
    ("Teal", Color::from_rgb(0.122, 0.616, 0.541)),
    ("Rose", Color::from_rgb(0.839, 0.271, 0.478)),
    ("Amber", Color::from_rgb(0.851, 0.541, 0.122)),
];

#[derive(Debug)]
struct Preferences {
    section: Section,
    dark: bool,
    accent: usize,
    text_size: f32,
    notifications: [bool; 3],
    volume: f32,
    delivery: Delivery,
    privacy: [bool; 2],
    confirming_clear: bool,
    history_cleared: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            section: Section::Appearance,
            dark: false,
            accent: 0,
            text_size: 15.0,
            notifications: [true, true, false],
            volume: 60.0,
            delivery: Delivery::Instantly,
            privacy: [false, true],
            confirming_clear: false,
            history_cleared: false,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    Section(Section),
    Dark(bool),
    Accent(usize),
    TextSize(f32),
    Notification(usize, bool),
    Volume(f32),
    Delivery(Delivery),
    Privacy(usize, bool),
    AskClear,
    ConfirmClear,
    CancelClear,
}

impl Preferences {
    fn theme(&self) -> Theme {
        ui::theme(self.dark, ACCENTS[self.accent].1)
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::Section(section) => self.section = section,
            Message::Dark(dark) => self.dark = dark,
            Message::Accent(accent) => self.accent = accent,
            Message::TextSize(size) => self.text_size = size,
            Message::Notification(i, on) => self.notifications[i] = on,
            Message::Volume(volume) => self.volume = volume,
            Message::Delivery(delivery) => self.delivery = delivery,
            Message::Privacy(i, on) => self.privacy[i] = on,
            Message::AskClear => {
                self.confirming_clear = true;
                self.history_cleared = false;
            }
            Message::ConfirmClear => {
                self.confirming_clear = false;
                self.history_cleared = true;
            }
            Message::CancelClear => self.confirming_clear = false,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let content = match self.section {
            Section::Appearance => self.appearance(),
            Section::Notifications => self.notifications(),
            Section::Privacy => self.privacy(),
            Section::About => about(),
        };

        let page = column![ui::heading(self.section.label()), content].spacing(18);

        let main = scrollable(
            container(page)
                .padding([24, 32])
                .max_width(640)
                .width(Length::Fill),
        )
        .style(ui::scroll)
        .width(Length::Fill)
        .height(Length::Fill);

        container(row![self.sidebar(), main]).style(ui::page).into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let nav = column(Section::ALL.map(|section| {
            button(text(section.label()))
                .width(Length::Fill)
                .padding([8, 12])
                .style(ui::nav(self.section == section))
                .on_press(Message::Section(section))
                .into()
        }))
        .spacing(2);

        container(
            column![
                container(text("Preferences").size(14).font(ui::SEMIBOLD)).padding([4, 12]),
                nav,
            ]
            .spacing(14),
        )
        .padding(14)
        .width(220)
        .height(Length::Fill)
        .style(ui::sidebar)
        .into()
    }

    fn appearance(&self) -> Element<'_, Message> {
        let theme_row = setting(
            "Theme",
            "Applies immediately",
            ui::segmented(
                &[("Light", false), ("Dark", true)],
                self.dark,
                Message::Dark,
            ),
        );

        let swatches = row(ACCENTS.iter().enumerate().map(|(i, &(_, color))| {
            button(space::horizontal())
                .width(24)
                .height(24)
                .style(ui::swatch(color, self.accent == i))
                .on_press(Message::Accent(i))
                .into()
        }))
        .spacing(10);
        let accent_row = setting("Accent colour", ACCENTS[self.accent].0, swatches.into());

        let size_row = setting(
            "Text size",
            format!("{} px", self.text_size),
            slider(12.0..=20.0, self.text_size, Message::TextSize)
                .step(1.0_f32)
                .width(200)
                .style(ui::range)
                .into(),
        );

        let preview = container(
            text("The quick brown fox jumps over the lazy dog.")
                .size(self.text_size)
                .width(Length::Fill),
        )
        .padding([14, 16])
        .width(Length::Fill)
        .style(ui::card);

        column![
            group(vec![theme_row, accent_row, size_row]),
            column![ui::caption("Preview"), preview].spacing(8),
        ]
        .spacing(18)
        .into()
    }

    fn notifications(&self) -> Element<'_, Message> {
        const ITEMS: [(&str, &str); 3] = [
            ("New messages", "Alert me when someone writes to me"),
            (
                "Mentions and replies",
                "When I am mentioned or a thread I follow moves",
            ),
            ("Weekly summary", "A recap of activity every Monday"),
        ];

        let mut rows: Vec<Element<'_, Message>> = ITEMS
            .iter()
            .enumerate()
            .map(|(i, &(title, hint))| {
                setting(
                    title,
                    hint,
                    toggle(self.notifications[i], move |on| {
                        Message::Notification(i, on)
                    }),
                )
            })
            .collect();

        rows.push(setting(
            "Volume",
            format!("{:.0}%", self.volume),
            slider(0.0..=100.0, self.volume, Message::Volume)
                .width(200)
                .style(ui::range)
                .into(),
        ));
        rows.push(setting(
            "Delivery",
            "How often notifications arrive",
            pick_list(Delivery::ALL, Some(self.delivery), Message::Delivery)
                .padding([6, 10])
                .width(160)
                .style(ui::dropdown)
                .menu_style(ui::dropdown_menu)
                .into(),
        ));
        group(rows)
    }

    fn privacy(&self) -> Element<'_, Message> {
        let toggles = group(vec![
            setting(
                "Share usage analytics",
                "Anonymous data that helps improve the app",
                toggle(self.privacy[0], |on| Message::Privacy(0, on)),
            ),
            setting(
                "Send crash reports",
                "Include a diagnostic report when something breaks",
                toggle(self.privacy[1], |on| Message::Privacy(1, on)),
            ),
        ]);

        let clear: Element<'_, Message> = if self.confirming_clear {
            row![
                text("Clear all history? This cannot be undone.").style(ui::danger),
                space::horizontal(),
                button(text("Cancel").size(13))
                    .padding([6, 14])
                    .style(ui::secondary)
                    .on_press(Message::CancelClear),
                button(text("Clear history").size(13))
                    .padding([6, 14])
                    .style(ui::primary)
                    .on_press(Message::ConfirmClear),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into()
        } else {
            let mut actions = row![button(text("Clear history").size(13))
                .padding([6, 14])
                .style(ui::destructive)
                .on_press(Message::AskClear)]
            .spacing(12)
            .align_y(iced::Center);
            if self.history_cleared {
                actions = actions.push(text("History cleared").size(13).style(ui::success));
            }
            actions.into()
        };

        column![
            toggles,
            container(clear)
                .padding([14, 16])
                .width(Length::Fill)
                .style(ui::card),
        ]
        .spacing(18)
        .into()
    }
}

fn about<'a>() -> Element<'a, Message> {
    container(
        column![
            text("Alloy Preferences").size(16).font(ui::SEMIBOLD),
            ui::caption("Version 1.4.0"),
            text("A small settings panel for trying out the same interface across toolkits.")
                .style(ui::muted),
        ]
        .spacing(6),
    )
    .padding([16, 18])
    .width(Length::Fill)
    .style(ui::card)
    .into()
}

fn toggle<'a>(on: bool, message: impl Fn(bool) -> Message + 'a) -> Element<'a, Message> {
    toggler(on)
        .on_toggle(message)
        .size(20)
        .style(ui::switch)
        .into()
}

/// A labelled row: title and hint on the left, the control on the right.
fn setting<'a>(
    title: &'a str,
    hint: impl text::IntoFragment<'a>,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    row![
        column![text(title), ui::caption(hint)].spacing(2),
        space::horizontal(),
        control,
    ]
    .align_y(iced::Center)
    .into()
}

/// Rows in one bordered card, separated by hairlines.
fn group<'a>(rows: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let last = rows.len().saturating_sub(1);
    let mut items = column![];
    for (i, content) in rows.into_iter().enumerate() {
        items = items.push(container(content).padding([13, 16]));
        if i != last {
            items = items.push(
                container(space::horizontal())
                    .width(Length::Fill)
                    .height(1)
                    .style(ui::hairline),
            );
        }
    }
    container(items).width(Length::Fill).style(ui::card).into()
}
