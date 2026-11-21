//! Live views: edit the file while the app runs and the screen follows, state intact.
use alloy_ui::{live_file, prelude::*, testing::Harness};

const V1: &str = "col gap=8\n  heading \"Hello, {name}\"\n  text \"{n} so far\" dim\n  button \"Add\" primary @add\n";

#[test]
fn edits_show_up_without_restart_and_errors_keep_the_last_good_screen() {
    std::env::set_var("ALLOY_DEV", "1"); // what `alloy dev` sets; only this test file runs here
    let path = std::env::temp_dir().join(format!("alloy-live-{}.alloy", std::process::id()));
    std::fs::write(&path, V1).unwrap();
    let p = path.to_str().unwrap().to_string();
    let app = sketch(0u32, move |n: &u32| {
        live_file(&p, V1)
            .var("name", "Ada")
            .var("n", n)
            .on("add", |n: &mut u32| *n += 1)
    });
    let mut h = Harness::new(app, 400.0, 240.0);
    h.expect("Hello, Ada")
        .expect("0 so far")
        .click("Add")
        .expect("1 so far");

    // Rewrite the copy and add a button; the counter keeps its value.
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, "col\n  heading \"Welcome back\"\n  text \"{n} so far\"\n  button \"Add\" @add\n  button \"Other\"\n").unwrap();
    h.advance(300)
        .click("Add")
        .expect("Welcome back")
        .expect("2 so far")
        .expect("Other");
    h.expect_no("Hello, Ada");

    // A typo shows a message over the previous screen instead of breaking it.
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&path, "col\n  bttn \"x\"\n").unwrap();
    h.advance(300);
    h.expect_containing("line 2: unknown element `bttn`")
        .expect("Welcome back");
    let _ = std::fs::remove_file(&path);
}
