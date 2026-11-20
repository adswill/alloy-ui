//! Settings panel: section list on the left, grouped controls on the right.
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Align, Length, Padding};
use alloy_ui_core::{Application, Command, Element};
use alloy_ui_theme::{Appearance, Theme, ThemeMode};
use alloy_ui_widgets::{
    Button, Checkbox, Column, Container, Root, Row, Slider, Text, TextRole, Ui, Variant,
};

/// Which section is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    /// Theme and text size.
    Appearance,
    /// Startup and updates.
    General,
}

/// Settings state.
pub struct Settings {
    /// Open section.
    pub section: Section,
    /// Chosen theme mode.
    pub mode: ThemeMode,
    /// Preview text size in px.
    pub text_px: f32,
    /// Launch at login.
    pub launch_at_login: bool,
    /// Check for updates.
    pub auto_update: bool,
    ui: Ui,
}

/// Settings messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Switch section.
    Section(Section),
    /// Pick theme mode.
    Mode(ThemeMode),
    /// Move text size.
    Size(f32),
    /// Set launch at login.
    Login(bool),
    /// Set updates.
    Update(bool),
}

impl Settings {
    /// Defaults.
    pub fn new(ui: &Ui) -> Self {
        Self {
            section: Section::Appearance,
            mode: ThemeMode::default(),
            text_px: 14.0,
            launch_at_login: false,
            auto_update: true,
            ui: ui.clone(),
        }
    }
}

impl Application for Settings {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Section(s) => self.section = s,
            Msg::Mode(m) => {
                self.mode = m;
                // With no windowed shell there is no system setting; "System" previews as Light.
                self.ui
                    .set_theme(Theme::for_appearance(m.resolve(Appearance::Light)));
            }
            Msg::Size(v) => self.text_px = v,
            Msg::Login(v) => self.launch_at_login = v,
            Msg::Update(v) => self.auto_update = v,
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let ui = &self.ui;
        let p = ui.theme().palette.clone();
        let nav = |text: &'static str, s: Section| -> Element<Msg> {
            let v = if self.section == s {
                Variant::Secondary
            } else {
                Variant::Ghost
            };
            Button::new(ui, text)
                .on_press(Msg::Section(s))
                .variant(v)
                .width(Length::Fill(1))
                .into_element()
        };
        let mode = |text: &'static str, m: ThemeMode| -> Element<Msg> {
            let v = if self.mode == m {
                Variant::Secondary
            } else {
                Variant::Ghost
            };
            Button::new(ui, text)
                .on_press(Msg::Mode(m))
                .variant(v)
                .into_element()
        };
        let group = |title: &str, rows: Vec<Element<Msg>>| -> Element<Msg> {
            Column::new(vec![
                Text::new(ui, title.to_string())
                    .role(TextRole::Label)
                    .secondary()
                    .into_element(),
                Column::new(rows).spacing(10.0).into_element(),
            ])
            .spacing(8.0)
            .into_element()
        };
        let body: Vec<Element<Msg>> = match self.section {
            Section::Appearance => vec![
                Text::new(ui, "Appearance")
                    .role(TextRole::Heading)
                    .into_element(),
                group(
                    "Theme",
                    vec![Row::new(vec![
                        mode("System", ThemeMode::System),
                        mode("Light", ThemeMode::Light),
                        mode("Dark", ThemeMode::Dark),
                    ])
                    .spacing(4.0)
                    .into_element()],
                ),
                group(
                    "Text size",
                    vec![
                        Slider::new(ui, 12.0, 20.0, self.text_px)
                            .step(1.0)
                            .on_change(Msg::Size)
                            .width(Length::Fill(1))
                            .into_element(),
                        Text::new(ui, "The quick brown fox jumps over the lazy dog.")
                            .color(p.text)
                            .into_element(),
                        Text::new(ui, format!("{:.0} px", self.text_px))
                            .role(TextRole::Caption)
                            .secondary()
                            .into_element(),
                    ],
                ),
            ],
            Section::General => vec![
                Text::new(ui, "General")
                    .role(TextRole::Heading)
                    .into_element(),
                group(
                    "Startup",
                    vec![
                        Checkbox::new(ui, "Launch at login", self.launch_at_login)
                            .on_toggle(Msg::Login)
                            .into_element(),
                        Checkbox::new(ui, "Check for updates automatically", self.auto_update)
                            .on_toggle(Msg::Update)
                            .into_element(),
                    ],
                ),
            ],
        };
        let side = Container::new(
            Column::new(vec![
                nav("Appearance", Section::Appearance),
                nav("General", Section::General),
            ])
            .spacing(2.0)
            .padding(Padding::all(12.0))
            .width(Length::Fill(1))
            .into_element(),
        )
        .background(p.surface)
        .width(Length::Fixed(170.0))
        .height(Length::Fill(1))
        .into_element();
        let main = Column::new(body)
            .spacing(22.0)
            .padding(Padding::all(28.0))
            .align(Align::Start)
            .width(Length::Fill(1))
            .height(Length::Fill(1))
            .into_element();
        Root::new(
            ui,
            Container::new(
                Row::new(vec![side, main])
                    .width(Length::Fill(1))
                    .height(Length::Fill(1))
                    .into_element(),
            )
            .background(p.background)
            .width(Length::Fill(1))
            .height(Length::Fill(1))
            .into_element(),
        )
        .into_element()
    }
}

/// Headless demo: dark mode, then General with launch at login ticked.
pub fn demo() -> Demo<Settings> {
    Demo {
        name: "settings",
        app: Settings::new(&crate::run::headless_ui(Theme::light())),
        size: (560.0, 300.0),
        steps: vec![
            Step::Click("Dark"),
            Step::Click("General"),
            Step::Click("Launch at login"),
        ],
    }
}
