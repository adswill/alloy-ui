//! Behaviour tests for the audit apps in `examples/` (pomodoro, files, wizard): the Alloy
//! versions must do what the iced versions in `loc/iced` do. Each app file is included
//! unchanged so the tests see its private state and view.

#![allow(dead_code)] // each included example has an unused `main`

mod pomodoro {
    include!("../examples/pomodoro.rs");
    use alloy_ui::testing::Harness;

    fn timer(minutes: [f32; 3]) -> Timer {
        Timer {
            minutes,
            ..Default::default()
        }
    }

    #[test]
    fn cycle() {
        let mut t = timer([1.0, 1.0, 2.0]);
        for _ in 0..60 {
            t.tick();
        }
        assert!(
            t.mode == Mode::Short && !t.running && t.done == 1,
            "focus ends in a stopped short break"
        );
        t.auto = true;
        for _ in 0..60 {
            t.tick();
        }
        assert!(
            t.mode == Mode::Focus && t.running,
            "break ends, auto-start runs the next focus"
        );
        t.done = 3;
        t.elapsed = 59;
        t.tick();
        assert!(
            t.mode == Mode::Long && t.total() - t.elapsed == 120,
            "every 4th focus earns a long break"
        );
    }

    #[test]
    fn ui() {
        let t = timer([25.0, 5.0, 15.0]);
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
        assert!(
            !ui.has_text("Required") && !ui.has_text("At least 8 characters"),
            "untouched fields show no errors"
        );
        ui.click("Next");
        assert!(
            ui.has_text("Account") && !ui.has_text("Free ($0/mo)"),
            "Next is disabled until the form is valid"
        );
        ui.click("Ada Lovelace").type_text("Grace");
        ui.click("ada@example.com").type_text("grace@x");
        assert!(!ui.has_text("Enter a valid email"), "not while typing");
        ui.press(Key::Tab);
        assert!(ui.has_text("Enter a valid email"), "shown once left");
        ui.click("grace@x").chord("mod+a").type_text("grace@x.dev");
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
            dir: root.clone(),
            ..Default::default()
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

mod split {
    include!("../examples/split.rs");
    use alloy_ui::testing::Harness;

    #[test]
    fn math_and_inputs() {
        let s = Split {
            bill: 84.5,
            tip: 15.0,
            people: 2,
            round_up: false,
        };
        let mut ui = Harness::new(sketch(s, view), 480.0, 520.0);
        assert!(ui.has_text("Each pays $48.59") && ui.has_text("Tip: $12.68"));
        ui.click("+");
        assert!(ui.has_text("Each pays $32.39"), "three people");
        ui.click("Round each share up");
        assert!(ui.has_text("Each pays $33.00"));
        ui.click("-").click("-").click("-");
        assert!(ui.has_text("Each pays $98.00"), "people stops at 1");
        ui.click("84.5").type_text("2");
        assert!(
            ui.has_text("Each pays $328.00"),
            "typed digits reach the state"
        );
        ui.type_text("x");
        assert!(
            !ui.has_text("2x84.5"),
            "letters are refused in a number field"
        );
    }
}
