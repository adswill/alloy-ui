use alloy_ui::prelude::*;

choices! { enum Page { Appearance, Notifications, Privacy, About } }
choices! { enum Accent { Indigo, Teal, Rose, Amber } }

struct Prefs {
    page: Page,
    dark: bool,
    accent: Accent,
    text_px: f32,
    mail: bool,
    push: bool,
    sounds: bool,
    volume: f32,
    delivery: usize,
    analytics: bool,
    crash: bool,
    cleared: bool,
}

impl Accent {
    fn color(self) -> Color {
        Color::hex(match self {
            Accent::Indigo => 0x4F6BED,
            Accent::Teal => 0x0F8B8D,
            Accent::Rose => 0xD6336C,
            Accent::Amber => 0xC77800,
        })
    }
}

fn view(s: &Prefs) -> impl View<Act<Prefs>> {
    set_accent(s.accent.color());
    let page = match s.page {
        Page::Appearance => col![
            heading("Appearance"),
            section("Theme", choose([("Light", false), ("Dark", true)], s.dark).act(|s: &mut Prefs, d| {
                s.dark = d;
                set_dark(d);
            })),
            section("Accent", options(s.accent).bind(|s: &mut Prefs| &mut s.accent)),
            section("Text size", col![
                slider(12.0..=20.0, at!(s.text_px)).step(1.0),
                text("The quick brown fox jumps over the lazy dog."),
                caption(format!("{:.0} px", s.text_px)).dim(),
            ]),
        ],
        Page::Notifications => col![
            heading("Notifications"),
            section("Channels", col![
                check("Email summaries", at!(s.mail)),
                check("Push notifications", at!(s.push)),
                check("Sounds", at!(s.sounds)),
            ]),
            section("Volume", slider(0.0..=100.0, at!(s.volume)).step(5.0)),
            section("Delivery", pick(["Instantly", "Hourly digest", "Daily digest"], Some(s.delivery)).act(|s: &mut Prefs, i| s.delivery = i)),
        ],
        Page::Privacy => col![
            heading("Privacy"),
            section("Sharing", col![
                check("Share anonymous usage data", at!(s.analytics)),
                check("Send crash reports", at!(s.crash)),
            ]),
            section("History", col![
                button("Clear history").act(|s: &mut Prefs| s.cleared = true).confirm("Clear all history?"),
                caption(if s.cleared { "History cleared." } else { "" }).dim(),
            ]),
        ],
        Page::About => col![
            heading("About"),
            text("Preferences 1.0.0"),
            caption("A small settings panel, drawn natively.").dim(),
        ],
    };
    row![
        nav(&s.page).bind(|s: &mut Prefs| &mut s.page).surface().w(200.0).fill_h(),
        page.gap(22.0).pad(32.0).fill()
    ]
    .gap(0.0)
    .fill()
}

fn main() {
    let state = Prefs {
        page: Page::Appearance, dark: false, accent: Accent::Indigo, text_px: 14.0,
        mail: true, push: true, sounds: false, volume: 60.0, delivery: 0,
        analytics: false, crash: true, cleared: false,
    };
    sketch(state, view).run_sized("Preferences", 960.0, 640.0)
}
