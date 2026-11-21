//! The starter apps `alloy new --template ..` writes, included unchanged: they must build
//! against this version of the framework and do what they claim.
#![allow(dead_code)] // each included template has an unused `main`

use alloy_ui::testing::Harness;

mod dashboard {
    include!("../../cli/src/template/dashboard/main.rs.txt");
    use super::Harness;

    #[test]
    fn shows_stats_chart_and_jobs() {
        let mut ui = Harness::new(sketch(Dash::default(), view), 560.0, 520.0);
        for t in [
            "Requests",
            "48.2k",
            "Recent jobs",
            "export-ledger",
            "failed",
        ] {
            assert!(ui.has_text(t), "{t}");
        }
        ui.advance(1000);
    }
}

mod editor {
    include!("../../cli/src/template/editor/main.rs.txt");
    use super::Harness;

    #[test]
    fn starts_saved_with_a_disabled_save_button() {
        let mut ui = Harness::new(sketch(Doc::default(), view), 620.0, 420.0);
        assert!(ui.has_text("Saved") && ui.has_text("0 lines, 0 words"));
        ui.click("Save").settle();
        assert!(ui.has_text("Saved"), "nothing to save yet");
    }

    #[test]
    fn open_and_save_round_trip() {
        let dir = std::env::temp_dir().join(format!("alloy-tpl-editor2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("b.txt");
        std::fs::write(&file, "one two\nthree").unwrap();
        let mut d = Doc {
            path: file.display().to_string(),
            ..Doc::default()
        };
        d.open();
        assert_eq!(
            (d.text.as_str(), d.saved.as_str()),
            ("one two\nthree", "one two\nthree")
        );
        d.text.push_str("\nfour");
        d.save();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "one two\nthree\nfour"
        );
        assert_eq!(d.saved, d.text);
        std::fs::remove_dir_all(dir).ok();
    }
}

mod settings {
    include!("../../cli/src/template/settings/main.rs.txt");
    use super::Harness;

    #[test]
    fn pages_switch() {
        let mut ui = Harness::new(sketch(Prefs::default(), view), 560.0, 340.0);
        assert!(ui.has_text("Text size") && !ui.has_text("Startup"));
        ui.click("General");
        assert!(ui.has_text("Startup") && ui.has_text("Launch at login"));
    }
}

mod localized {
    include!("../../cli/src/template/localized/src/main.rs.txt");
    use super::Harness;

    #[test]
    fn switches_language_and_catalogs_are_complete() {
        alloy_ui::i18n::add(
            "en",
            include_str!("../../cli/src/template/localized/locales/en.txt"),
        );
        alloy_ui::i18n::add(
            "fr",
            include_str!("../../cli/src/template/localized/locales/fr.txt"),
        );
        alloy_ui::i18n::add(
            "ar",
            include_str!("../../cli/src/template/localized/locales/ar.txt"),
        );
        for l in ["fr", "ar"] {
            assert!(alloy_ui::i18n::missing(l, "en").is_empty(), "{l}");
        }
        alloy_ui::i18n::set_locale("en");
        let mut ui = Harness::new(sketch(Mail::default(), view), 640.0, 300.0);
        assert!(ui.has_text("You have 3 unread messages"));
        ui.click("Français");
        assert!(ui.has_text("Vous avez 3 messages non lus"));
    }
}
