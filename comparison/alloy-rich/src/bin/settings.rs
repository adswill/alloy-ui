use alloy_ui::prelude::*;
use rich::*;

choices! { enum Page { Appearance, Notifications, Language, Storage, Privacy, About } }
choices! { enum Density { Compact, Comfortable, Spacious } }
choices! { enum Clock { Hours12 = "12-hour", Hours24 = "24-hour" } }

const ACCENTS: [(&str, u32); 4] = [("Indigo", INDIGO), ("Teal", 0x0F8B8D), ("Rose", 0xD6336C), ("Amber", 0xC77800)];
const DELIVERY: [&str; 3] = ["Instantly", "Hourly digest", "Daily digest"];

#[derive(Clone, PartialEq)]
struct Prefs {
    page: Page,
    dark: bool,
    accent: usize,
    text_px: f32,
    mail: bool,
    push: bool,
    sounds: bool,
    volume: f32,
    delivery: usize,
    analytics: bool,
    crash: bool,
    history: u32,
    density: Density,
    motion: bool,
    lang: usize,
    clock: Clock,
    cache_mb: u32,
}

const LANGS: [&str; 5] = ["English", "Deutsch", "Fran\u{e7}ais", "Espa\u{f1}ol", "Lietuvi\u{173}"];

fn appearance(s: &Prefs) -> Node<Act<Prefs>> {
    let colors = ACCENTS.map(|(_, c)| Color::hex(c));
    col![
        group(vec![
            setting("Theme", "Applies immediately", choose([("Light", false), ("Dark", true)], s.dark).act(|s: &mut Prefs, d| s.dark = d)),
            setting("Accent colour", ACCENTS[s.accent].0, swatches(&colors, s.accent, |s: &mut Prefs, i| s.accent = i)),
            setting("Text size", format!("{:.0} px", s.text_px), slider(12.0..=20.0, at!(s.text_px)).step(1.0).w(200.0)),
            setting("Density", "Row height in lists and tables", options(s.density).bind(|s: &mut Prefs| &mut s.density)),
            setting("Reduce motion", "Skip transitions and animations", toggle("", at!(s.motion))),
        ]),
        caption("Preview").dim(),
        col![sized_text("The quick brown fox jumps over the lazy dog.", s.text_px), caption("Body text, as it will appear in your documents.").dim()]
            .gap(4.0)
            .card()
            .fill_w(),
    ]
    .gap(10.0)
    .into()
}

fn notifications(s: &Prefs) -> Node<Act<Prefs>> {
    col![
        group(vec![
            setting("Email summaries", "A weekly recap of what changed", toggle("", at!(s.mail))),
            setting("Push notifications", "Mentions and direct messages", toggle("", at!(s.push))),
            setting("Sounds", "Play a chime for new items", toggle("", at!(s.sounds))),
        ]),
        group(vec![
            setting("Volume", format!("{:.0}%", s.volume), slider(0.0..=100.0, at!(s.volume)).step(5.0).w(200.0)),
            setting("Delivery", "How often alerts arrive", pick(DELIVERY, Some(s.delivery)).act(|s: &mut Prefs, i| s.delivery = i)),
        ]),
        button("Send a test notification").act(|s: &mut Prefs| {
            toast(format!("Test sent, {}", DELIVERY[s.delivery].to_lowercase()));
        }),
    ]
    .gap(14.0)
    .into()
}

fn language(s: &Prefs) -> Node<Act<Prefs>> {
    let now = if s.clock == Clock::Hours12 { "4:25 PM" } else { "16:25" };
    col![
        group(vec![
            setting("Language", "Restart to apply everywhere", pick(LANGS, Some(s.lang)).act(|s: &mut Prefs, i| s.lang = i)),
            setting("Time format", format!("Shown as {now}"), options(s.clock).bind(|s: &mut Prefs| &mut s.clock)),
        ]),
        caption("Changing the language only affects menus and dialogs; your documents are untouched.").dim(),
    ]
    .gap(10.0)
    .into()
}

fn storage(s: &Prefs) -> Node<Act<Prefs>> {
    let used = [("Documents", 412.0), ("Media", 268.0), ("Cache", s.cache_mb as f32)];
    let total: f32 = used.iter().map(|u| u.1).sum();
    let p = palette();
    let shades = [p.accent, p.accent.with_alpha(0.6), p.accent.with_alpha(0.3)];
    col![
        col![
            row![heading(format!("{total:.0} MB")).sub(), caption("of 2 GB used").dim()].gap(8.0).align_center(),
            stack(&used.iter().zip(shades).map(|(u, c)| (u.1, c)).collect::<Vec<_>>()),
            row(used.iter().zip(shades).map(|(u, c)| legend(c, format!("{} {:.0} MB", u.0, u.1)))).gap(16.0),
        ]
        .gap(10.0)
        .card()
        .fill_w(),
        group(vec![setting(
            "Clear cache",
            "Downloaded previews and thumbnails",
            button("Clear cache").act(|s: &mut Prefs| {
                s.cache_mb = 0;
                toast("Cache cleared").undo();
            }).enabled(s.cache_mb > 0),
        )]),
    ]
    .gap(14.0)
    .into()
}

fn privacy(s: &Prefs) -> Node<Act<Prefs>> {
    let history = if s.history == 0 { "Nothing stored".to_string() } else { format!("{} items stored on this device", s.history) };
    col![
        group(vec![
            setting("Share usage data", "Anonymous, never sold", toggle("", at!(s.analytics))),
            setting("Send crash reports", "Helps us fix problems faster", toggle("", at!(s.crash))),
        ]),
        group(vec![setting(
            "Clear history",
            history,
            button("Clear history").act(|s: &mut Prefs| {
                s.history = 0;
                toast("History cleared").undo();
            }).confirm("Clear all history?"),
        )]),
    ]
    .gap(14.0)
    .into()
}

fn about() -> Node<Act<Prefs>> {
    let keys = [("mod+1 to mod+6", "Switch page"), ("mod+d", "Toggle dark mode"), ("mod+k", "Command palette"), ("mod+z", "Undo")];
    col![
        row![
            heading("P").pad_xy(14.0, 8.0).bg(palette().accent.with_alpha(0.15)).round(20.0),
            col![text("Preferences").label(), caption("A small settings panel, drawn natively.").dim()].gap(2.0),
            space(),
            pill("Active"),
        ]
        .gap(12.0)
        .align_center()
        .card()
        .fill_w(),
        group(vec![
            setting("Version", "Released 14 November", caption("1.0.0").dim()),
            setting("Updates", "Last checked today", button("Check for updates").act(|_| {
 toast("You are on the latest version");
 })),
        ]),
        button("Reset all settings").act(|s: &mut Prefs| {
            *s = Prefs::new();
            toast("Settings reset").undo();
        }).confirm("Reset every preference to its default?"),
        caption("Keyboard").dim(),
        group(keys.iter().map(|(k, what)| setting(what, "", kbd(k))).collect()),
    ]
    .gap(10.0)
    .into()
}

fn view(s: &Prefs) -> impl View<Act<Prefs>> {
    set_accent(Color::hex(ACCENTS[s.accent].1));
    set_dark(s.dark);
    let page = match s.page {
        Page::Appearance => appearance(s),
        Page::Notifications => notifications(s),
        Page::Language => language(s),
        Page::Storage => storage(s),
        Page::Privacy => privacy(s),
        Page::About => about(),
    };
    row![
        col![heading("Preferences").pad_xy(12.0, 6.0), nav(&s.page).bind(|s: &mut Prefs| &mut s.page), vspace(), caption("Ctrl+K opens commands").dim().pad(12.0)]
            .gap(8.0)
            .pad(8.0)
            .surface()
            .w(220.0)
            .fill_h(),
        scroll(col![heading(name(s.page)), page].gap(16.0).pad(32.0).fill_w()).fill(),
    ]
    .gap(0.0)
    .fill()
}

impl Prefs {
    fn new() -> Self {
        Prefs {
            page: Page::Appearance, dark: false, accent: 0, text_px: 15.0, mail: true, push: true, sounds: false,
            volume: 60.0, delivery: 0, analytics: false, crash: true, history: 214, density: Density::Comfortable,
            motion: false, lang: 0, clock: Clock::Hours24, cache_mb: 96,
        }
    }
}

fn main() {
    sketch(Prefs::new(), view)
        .undoable()
        .shortcut_as("mod+1", "Appearance", |s| s.page = Page::Appearance)
        .shortcut_as("mod+2", "Notifications", |s| s.page = Page::Notifications)
        .shortcut_as("mod+3", "Language", |s| s.page = Page::Language)
        .shortcut_as("mod+4", "Storage", |s| s.page = Page::Storage)
        .shortcut_as("mod+5", "Privacy", |s| s.page = Page::Privacy)
        .shortcut_as("mod+6", "About", |s| s.page = Page::About)
        .shortcut_as("mod+d", "Toggle dark", |s| s.dark ^= true)
        .run_sized("Preferences", 960.0, 640.0)
}
