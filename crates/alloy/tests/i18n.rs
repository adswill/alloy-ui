//! Translation and right-to-left mirroring through the real runtime, headless. This file is
//! its own process, so switching the global locale cannot disturb other tests.
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::{i18n, t};

const EN: &str = "greet = Hello, {name}!\nfiles.one = {n} file\nfiles.other = {n} files\nadd = Add";
const FR: &str =
    "greet = Bonjour, {name} !\nfiles.one = {n} fichier\nfiles.other = {n} fichiers\nadd = Ajouter";
const AR: &str = "greet = مرحبا، {name}!\nfiles.zero = لا ملفات\nfiles.one = ملف واحد\nfiles.two = ملفان\nfiles.few = {n} ملفات\nfiles.many = {n} ملفًا\nfiles.other = {n} ملف\nadd = إضافة";

state! { struct Docs { files: u32 = 1 } }

fn view(s: &Docs) -> impl View<Act<Docs>> {
    col![
        row![
            text(t!("greet", name = "Ada")),
            button(t!("add")).act(|s: &mut Docs| s.files += 1)
        ]
        .gap(40.0),
        text(t!("files", n = s.files)),
        i18n::language_picker::<Docs>(&[("en", "English"), ("fr", "Français"), ("ar", "العربية")]),
    ]
    .pad(16.0)
}

#[test]
fn locale_switching_plurals_and_rtl() {
    i18n::add("en", EN);
    i18n::add("fr", FR);
    i18n::add("ar", AR);
    assert!(i18n::missing("fr", "en").is_empty());
    assert!(i18n::missing("ar", "en").is_empty());

    i18n::set_locale("en");
    let mut ui = Harness::new(sketch(Docs::default(), view), 420.0, 200.0);
    assert!(ui.has_text("Hello, Ada!") && ui.has_text("1 file"));
    let x = |ui: &Harness<_>, t: &str| ui.position(t).unwrap().x;
    assert!(
        x(&ui, "Hello, Ada!") < x(&ui, "Add"),
        "LTR: text before button"
    );

    ui.click("Add");
    assert!(ui.has_text("2 files"));

    // Switching the language rebuilds the view in place, state intact.
    ui.click("Français");
    assert!(
        ui.has_text("Bonjour, Ada !") && ui.has_text("2 fichiers"),
        "{:?}",
        i18n::locale()
    );
    ui.click("Ajouter");
    assert!(ui.has_text("3 fichiers"));

    ui.click("العربية");
    assert!(i18n::is_rtl());
    assert!(ui.has_text("مرحبا، Ada!") && ui.has_text("3 ملفات"));
    assert!(x(&ui, "مرحبا، Ada!") > x(&ui, "إضافة"), "RTL: row mirrors");

    ui.click("English");
    assert!(!i18n::is_rtl());
    assert!(ui.has_text("Hello, Ada!") && ui.has_text("3 files"));
}
