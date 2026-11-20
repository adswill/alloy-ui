//! Settings panel: section list on the left, grouped controls on the right.
use crate::kit::{button, checkbox, column, label, row, slider, Emphasis};
use crate::run::{Demo, Step};
use alloy_ui_core::style::{Length, Padding};
use alloy_ui_core::{Application, Command, Element};
use alloy_ui_theme::{Appearance, Theme, ThemeMode};

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
    /// Text scale as 0..=1 across 12..=20 px.
    pub text_scale: f32,
    /// Launch at login.
    pub launch_at_login: bool,
    /// Check for updates.
    pub auto_update: bool,
}

/// Settings messages.
#[derive(Debug, Clone)]
pub enum Msg {
    /// Switch section.
    Section(Section),
    /// Pick theme mode.
    Mode(ThemeMode),
    /// Move text scale.
    Scale(f32),
    /// Toggle launch at login.
    Login,
    /// Toggle updates.
    Update,
}

impl Settings {
    /// Defaults.
    pub fn new() -> Self {
        Self {
            section: Section::Appearance,
            mode: ThemeMode::default(),
            text_scale: 0.25,
            launch_at_login: false,
            auto_update: true,
        }
    }

    /// Text size in px for the current scale.
    pub fn text_px(&self) -> f32 {
        12.0 + 8.0 * self.text_scale
    }

    fn theme(&self) -> Theme {
        // The panel previews a fixed system appearance of Light when following the system.
        Theme::for_appearance(self.mode.resolve(Appearance::Light))
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Application for Settings {
    type Message = Msg;

    fn update(&mut self, m: Msg) -> Command<Msg> {
        match m {
            Msg::Section(s) => self.section = s,
            Msg::Mode(m) => self.mode = m,
            Msg::Scale(v) => self.text_scale = v,
            Msg::Login => self.launch_at_login = !self.launch_at_login,
            Msg::Update => self.auto_update = !self.auto_update,
        }
        Command::None
    }

    fn view(&self) -> Element<Msg> {
        let t = self.theme();
        let p = &t.palette;
        let nav = |text: &'static str, s: Section| -> Element<Msg> {
            button(text, Emphasis::Quiet, p, Msg::Section(s))
                .selected(self.section == s)
                .into()
        };
        let mode = |text: &'static str, m: ThemeMode| -> Element<Msg> {
            button(text, Emphasis::Secondary, p, Msg::Mode(m))
                .selected(self.mode == m)
                .into()
        };
        let body: Vec<Element<Msg>> = match self.section {
            Section::Appearance => vec![
                label("Appearance", 20.0, p.text).into(),
                label("Theme", 14.0, p.text_secondary).into(),
                row(vec![
                    mode("System", ThemeMode::System),
                    mode("Light", ThemeMode::Light),
                    mode("Dark", ThemeMode::Dark),
                ])
                .spacing(6.0)
                .into(),
                label(
                    format!("Text size {:.0} px", self.text_px()),
                    14.0,
                    p.text_secondary,
                )
                .into(),
                slider(self.text_scale, p, Msg::Scale).into(),
                label(
                    "The quick brown fox jumps over the lazy dog.",
                    self.text_px(),
                    p.text,
                )
                .into(),
            ],
            Section::General => vec![
                label("General", 20.0, p.text).into(),
                checkbox("Launch at login", self.launch_at_login, p, Msg::Login).into(),
                checkbox(
                    "Check for updates automatically",
                    self.auto_update,
                    p,
                    Msg::Update,
                )
                .into(),
            ],
        };
        row(vec![
            column(vec![
                nav("Appearance", Section::Appearance),
                nav("General", Section::General),
            ])
            .spacing(4.0)
            .padding(Padding::all(12.0))
            .width(Length::Fixed(150.0))
            .height(Length::Fill(1))
            .background(p.surface)
            .into(),
            column(body)
                .spacing(14.0)
                .padding(Padding::all(28.0))
                .width(Length::Fill(1))
                .height(Length::Fill(1))
                .into(),
        ])
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .background(p.background)
        .into()
    }
}

/// Headless demo: dark mode, bigger text, then General with login on.
pub fn demo() -> Demo<Settings> {
    Demo {
        name: "settings",
        app: Settings::new(),
        size: (560.0, 300.0),
        steps: vec![
            Step::Click("Dark"),
            Step::Click("General"),
            Step::Click("Launch at login"),
        ],
    }
}
