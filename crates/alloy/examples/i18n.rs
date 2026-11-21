//! Translated and right-to-left aware: switch the language live, plurals included.
//! Try `LANG=ar cargo run --example i18n`; the sidebar moves to the right on its own.
use alloy_ui::{i18n, prelude::*};

state! { struct Mail { page: usize, unread: u32 = 3 } }

fn view(s: &Mail) -> impl View<Act<Mail>> {
    let pages = [t!("nav.inbox"), t!("nav.sent"), t!("nav.drafts")];
    row![
        col![
            heading(t!("app.title")),
            col(pages.iter().enumerate().map(|(i, p)| button(p)
                .nav(i == s.page)
                .act(move |s: &mut Mail| s.page = i)))
            .gap(2.0),
        ]
        .gap(16.0)
        .pad(16.0)
        .w(180.0)
        .fill_h(),
        col![
            heading(pages[s.page].clone()),
            caption(t!("greeting", name = "Ada")).dim(),
            text(t!("inbox.summary", n = s.unread)),
            row![
                button(t!("inbox.read"))
                    .primary()
                    .act(|s: &mut Mail| s.unread = s.unread.saturating_sub(1)),
                button(t!("inbox.reset")).act(|s: &mut Mail| s.unread = 12),
            ],
            vspace(),
            i18n::language_picker::<Mail>(&[
                ("en", "English"),
                ("fr", "Français"),
                ("ar", "العربية")
            ]),
        ]
        .gap(12.0)
        .pad(24.0)
        .fill()
    ]
    .gap(0.0)
    .fill()
}

fn main() {
    i18n::add("en", include_str!("i18n/en.txt"));
    i18n::add("fr", include_str!("i18n/fr.txt"));
    i18n::add("ar", include_str!("i18n/ar.txt"));
    i18n::set_locale(&i18n::detect());
    sketch(Mail::default(), view).run_sized("Mail", 640.0, 300.0)
}
