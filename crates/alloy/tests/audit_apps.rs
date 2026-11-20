//! Behaviour tests for the audit apps in `examples/` (pomodoro, files, wizard): the Alloy
//! versions must do what the iced versions in `loc/iced` do. Each app file is included
//! unchanged so the tests see its private state and view.

#![allow(dead_code)] // each included example has an unused `main`

mod pomodoro {
    include!("../examples/pomodoro.rs");
    use alloy_ui::testing::Harness;

    #[test]
    fn cycle() {
        let mut t = Timer {
            mode: Mode::Focus,
            minutes: [1.0, 1.0, 2.0],
            left: 60,
            running: false,
            done: 0,
            auto: false,
        };
        t.tick();
        assert_eq!(t.left, 60, "paused timers do not move");
        t.running = true;
        for _ in 0..60 {
            t.tick();
        }
        assert!(
            matches!(t.mode, Mode::Short) && !t.running && t.done == 1,
            "focus ends in a stopped short break"
        );
        t.auto = true;
        t.running = true;
        for _ in 0..60 {
            t.tick();
        }
        assert!(
            matches!(t.mode, Mode::Focus) && t.running,
            "break ends, auto-start runs the next focus"
        );
        t.done = 3;
        t.mode = Mode::Focus;
        t.left = 1;
        t.tick();
        assert!(
            matches!(t.mode, Mode::Long) && t.left == 120,
            "every 4th focus earns a long break"
        );
    }

    #[test]
    fn ui() {
        let t = Timer {
            mode: Mode::Focus,
            minutes: [25.0, 5.0, 15.0],
            left: 1500,
            running: false,
            done: 0,
            auto: false,
        };
        let mut ui = Harness::new(sketch(t, view), 480.0, 560.0);
        assert!(ui.has_text("25:00") && ui.has_text("Sessions completed: 0"));
        ui.click("Start");
        assert!(ui.has_text("Pause"));
        ui.click("Short break");
        assert!(
            ui.has_text("05:00") && ui.has_text("Start"),
            "switching modes resets and stops"
        );
    }
}

mod wizard {
    include!("../examples/wizard.rs");
    use alloy_ui::{testing::Harness, Key};

    #[test]
    fn flow() {
        let mut ui = Harness::new(
            sketch(
                Wizard {
                    plan: Plan::Pro,
                    ..Default::default()
                },
                view,
            ),
            520.0,
            520.0,
        );
        assert!(ui.has_text("Required") && ui.has_text("At least 8 characters"));
        ui.click("Next");
        assert!(
            ui.has_text("1. Account") && !ui.has_text("Free ($0/mo)"),
            "Next is disabled until the form is valid"
        );
        ui.click("Ada Lovelace").type_text("Grace");
        ui.click("ada@example.com").type_text("grace@x");
        assert!(ui.has_text("Enter a valid email"));
        ui.type_text(".dev");
        ui.press(Key::Tab);
        ui.type_text("hunter22");
        assert!(!ui.has_text("hunter22"), "passwords are masked");
        ui.click("Next");
        assert!(ui.has_text("Free ($0/mo)"), "reached the plan step");
        ui.click("Team ($40/mo)")
            .click("Bill yearly (2 months free)")
            .click("Next");
        assert!(ui.has_text("Name: Grace") && ui.has_text("Plan: Team at $33/mo, billed yearly"));
        ui.click("Create account");
        assert!(
            !ui.has_text("Welcome, Grace."),
            "terms must be accepted first"
        );
        ui.click("I accept the terms").click("Create account");
        assert!(ui.has_text("Welcome, Grace."));
    }
}

mod files {
    include!("../examples/files.rs");
    use alloy_ui::testing::Harness;

    #[test]
    fn navigate() {
        let root = std::env::temp_dir().join(format!("alloy-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("b.txt"), vec![0; 500]).unwrap();
        std::fs::write(root.join("a.txt"), vec![0; 2000]).unwrap();
        std::fs::write(root.join("sub/inner.txt"), "x").unwrap();
        std::fs::write(root.join(".secret"), "x").unwrap();
        let s = Files {
            entries: load(&root),
            dir: root.clone(),
            hidden: false,
            query: String::new(),
            selected: None,
        };
        let mut ui = Harness::new(sketch(s, view), 900.0, 600.0);
        assert!(ui.has_text("sub/") && ui.has_text("a.txt") && !ui.has_text(".secret"));
        assert!(
            ui.position("sub/").unwrap().y < ui.position("a.txt").unwrap().y,
            "folders first"
        );
        ui.click("Show hidden");
        assert!(ui.has_text(".secret"));
        ui.click("b.txt");
        assert!(ui.has_text("Size: 500 B"));
        ui.click("sub/");
        assert!(ui.has_text("inner.txt") && ui.has_text("1 items") && !ui.has_text("b.txt"));
        ui.click("Up");
        assert!(ui.has_text("b.txt"));
        ui.click("Size");
        let order = |ui: &Harness<_>| ["a.txt", "b.txt"].map(|n| ui.position(n).unwrap().y);
        let [a, b] = order(&ui);
        assert!(b < a, "ascending size: 500 B before 2.0 KB");
        let _ = std::fs::remove_dir_all(&root);
    }
}
