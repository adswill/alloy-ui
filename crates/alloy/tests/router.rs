//! `router`: push, back, forward, breadcrumbs and the alt+arrow keys.
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Clone, PartialEq, Debug)]
enum Route {
    List,
    Note(u32),
}

fn name(r: &Route) -> String {
    match r {
        Route::List => "Notes".into(),
        Route::Note(i) => format!("Note {i}"),
    }
}

fn app() -> impl App {
    sketch((), |_| {
        let nav = router(Route::List);
        nav.show(|route| match route {
            Route::List => col![
                heading("All notes"),
                button("Open 1").act(move |_| nav.push(Route::Note(1))),
                button("Open 2").act(move |_| nav.push(Route::Note(2))),
            ]
            .into(),
            Route::Note(i) => col![
                nav.breadcrumbs(name),
                heading(format!("Body {i}")),
                nav.back_button(),
                button("Replace").act(move |_| nav.replace(Route::Note(9))),
            ]
            .into(),
        })
    })
}

#[test]
fn push_back_forward() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.expect("All notes")
        .click("Open 2")
        .expect("Body 2")
        .expect_no("All notes");
    h.click("Back").expect("All notes").expect_no("Body 2");
    h.chord("alt+ArrowRight").expect("Body 2");
    h.chord("alt+ArrowLeft").expect("All notes");
}

#[test]
fn push_after_back_clears_forward() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.click("Open 1").click("Back").click("Open 2");
    h.chord("alt+ArrowRight").expect("Body 2"); // no forward history: stays put
}

#[test]
fn breadcrumbs_jump_back_and_replace_keeps_depth() {
    let mut h = Harness::new(app(), 500.0, 300.0);
    h.click("Open 1").click("Replace").expect("Body 9");
    h.expect("Note 9").click("Notes").expect("All notes");
}

fn linked_app() -> impl App {
    sketch((), |_| {
        let nav = router(Route::List).links(|url| match link_path(url).as_slice() {
            [a, n] if a == "note" => n.parse().ok().map(Route::Note),
            _ => None,
        });
        nav.show(|route| match route {
            Route::List => heading("All notes").into(),
            Route::Note(i) => col![heading(format!("Body {i}")), nav.back_button()].into(),
        })
    })
}

#[test]
fn link_path_splits_scheme_query_and_fragment() {
    assert_eq!(link_path("myapp://invite/42?x=1#top"), ["invite", "42"]);
    assert_eq!(link_path("/a//b/"), ["a", "b"]);
    assert!(link_path("myapp://").is_empty());
}

#[test]
fn deep_link_opens_a_screen_and_back_returns() {
    let mut h = Harness::new(linked_app(), 500.0, 300.0);
    h.expect("All notes");
    h.open_url("myapp://note/7").expect("Body 7");
    h.open_url("myapp://other/1").expect("Body 7"); // not a route: ignored
    h.open_url("myapp://note/7").expect("Body 7"); // already there: no duplicate step
    h.click("Back").expect("All notes");
}

#[test]
fn link_before_the_router_exists_is_kept_for_it() {
    // A launch link arrives before the first view; the router picks it up when it registers.
    std::thread::spawn(|| {
        use alloy_ui::testing::Harness;
        let app = sketch(0u8, |_| {
            let nav = router(Route::List).links(|url| match link_path(url).as_slice() {
                [a, n] if a == "note" => n.parse().ok().map(Route::Note),
                _ => None,
            });
            nav.show(|route| match route {
                Route::List => heading("All notes").into(),
                Route::Note(i) => heading(format!("Body {i}")).into(),
            })
        });
        // Deliver before building anything: nothing is registered yet.
        alloy_ui::router_deliver_for_tests("myapp://note/3");
        let mut h = Harness::new(app, 400.0, 200.0);
        h.expect("Body 3");
    })
    .join()
    .unwrap();
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
enum Saved {
    List,
    Note(u32),
}

fn saved_app(path: std::path::PathBuf) -> impl App {
    sketch((), move |_| {
        let nav = router_saved_at(path.clone(), Saved::List);
        nav.show(|r| match r {
            Saved::List => button("Open 5")
                .act(move |_| nav.push(Saved::Note(5)))
                .into(),
            Saved::Note(i) => col![heading(format!("Saved {i}")), nav.back_button()].into(),
        })
    })
}

#[test]
fn saved_router_restores_the_stack_after_a_restart() {
    let dir = std::env::temp_dir().join(format!("alloy-router-{}", std::process::id()));
    let file = dir.join("router.json");
    let _ = std::fs::remove_file(&file);
    // Each "run" gets its own thread, so view-local state starts empty like a new process.
    let run = |f: std::path::PathBuf, check: fn(&mut Harness<_>)| {
        std::thread::spawn(move || {
            let mut h = Harness::new(saved_app(f), 400.0, 200.0);
            check(&mut h);
        })
        .join()
        .unwrap();
    };
    run(file.clone(), |h| {
        h.expect("Open 5").click("Open 5").expect("Saved 5");
    });
    run(file.clone(), |h| {
        h.expect("Saved 5"); // reopened where it was left
        h.click("Back").expect("Open 5");
    });
    run(file.clone(), |h| {
        h.expect("Open 5"); // the back step was saved too
    });
    // A corrupt file falls back to home.
    std::fs::write(&file, b"{ nope").unwrap();
    run(file.clone(), |h| {
        h.expect("Open 5");
    });
    let _ = std::fs::remove_dir_all(&dir);
}
