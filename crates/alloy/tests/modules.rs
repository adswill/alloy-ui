//! Ready-made modules: behaviour through the real runtime, and golden images (shapes only, no
//! glyphs, so they hold on every OS). Regenerate with `UPDATE_GOLDEN=1 cargo test -p alloy-ui --test modules`.
use alloy_ui::prelude::*;
use alloy_ui::render::testing::{assert_golden, Tolerance};
use alloy_ui::render::{CpuRenderer, HeadlessRenderer};
use alloy_ui::testing::Harness;
use alloy_ui::theme::Theme;
use alloy_ui::widgets::Tone;
use alloy_ui::DrawCommand;
use std::path::Path;

fn golden<A: App>(h: &Harness<A>, w: u32, ht: u32, name: &str, dark: bool) {
    let mut list = h.draw_list();
    list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
    let mut r = CpuRenderer::new(w * 2, ht * 2);
    let theme = if dark { Theme::dark() } else { Theme::light() };
    r.set_background(theme.palette.background);
    let img = r.render_to_image(&list, w * 2, ht * 2, 2.0);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden_modules");
    assert_golden(&dir, name, &img, Tolerance::CPU);
}

fn themed<A: App>(app: A, w: f32, h: f32, dark: bool) -> Harness<A> {
    Harness::themed(app, w, h, if dark { Theme::dark() } else { Theme::light() })
}

// ------------------------------------------------------------- dashboards

fn dash(_: &()) -> impl View<Act<()>> {
    col![
        kpi_row([
            kpi("Revenue", "$48.2k")
                .delta(12.4)
                .hint("vs last week")
                .trend([1.0, 3.0, 2.0, 5.0, 4.0, 7.0]),
            kpi("p95 latency", "74 ms")
                .delta(-8.2)
                .lower_is_better()
                .trend([9.0, 8.0, 8.5, 7.0, 7.4, 6.0]),
            kpi("Errors", "0.4%").delta(0.0),
        ]),
        row![
            gauge(0.42).label("CPU").bands().size(110.0),
            gauge(0.78).label("Memory").bands().size(110.0),
            gauge(0.95).label("Disk").bands().size(110.0),
            radial(0.64).label("Done").size(110.0),
        ]
        .gap(16.0),
        row![
            avatar_group([
                "Mira Okafor",
                "Jon Reyes",
                "Sam Lee",
                "Ana Cruz",
                "Dev Patel"
            ])
            .max(3),
            avatar("Mira Okafor").presence(Presence::Online),
            avatar("Jon Reyes").presence(Presence::Busy),
        ]
        .gap(14.0)
        .align_center(),
    ]
    .gap(16.0)
    .pad(16.0)
}

#[test]
fn dashboard_modules_render() {
    for dark in [false, true] {
        let h = themed(sketch((), dash), 640.0, 360.0, dark);
        for t in [
            "Revenue",
            "$48.2k",
            "12.4%",
            "p95 latency",
            "8.2%",
            "CPU",
            "42%",
            "95%",
            "Done",
            "64%",
            "+2",
        ] {
            assert!(h.has_text(t), "missing {t}: {:?}", h.texts());
        }
        golden(
            &h,
            640,
            360,
            if dark {
                "dashboard_dark"
            } else {
                "dashboard_light"
            },
            dark,
        );
    }
}

// ------------------------------------------------------------------ feeds

fn feeds_view(_: &()) -> impl View<Act<()>> {
    col![
        moments([
            moment("12:04", "Deploy finished")
                .detail("Live in 3 regions.")
                .success(),
            moment("11:30", "Build passed"),
            moment("10:18", "Rollback")
                .detail("Scroll jitter.")
                .danger(),
        ]),
        changelog([
            release("0.4.0", "Nov 21")
                .latest()
                .added(["Gauges", "Tour"])
                .fixed(["Jitter"]),
            release("0.3.2", "Nov 14").changed(["Hover"]),
        ]),
        activity_feed([activity("Mira Okafor", "merged", "Add gauges", "4 min")]),
    ]
    .gap(14.0)
    .pad(16.0)
}

#[test]
fn feed_modules_render() {
    for dark in [false, true] {
        let h = themed(sketch((), feeds_view), 520.0, 560.0, dark);
        for t in [
            "Deploy finished",
            "Live in 3 regions.",
            "0.4.0",
            "Latest",
            "Added",
            "Fixed",
            "Changed",
            "merged",
        ] {
            assert!(h.has_text(t), "missing {t}: {:?}", h.texts());
        }
        golden(
            &h,
            520,
            560,
            if dark { "feeds_dark" } else { "feeds_light" },
            dark,
        );
    }
}

struct Inbox {
    items: Vec<(u64, bool)>,
}

fn inbox_view(s: &Inbox) -> impl View<Act<Inbox>> {
    notification_center::<Inbox>(s.items.iter().map(|&(id, unread)| {
        let n = notice(id, format!("Item {id}"))
            .body("Details")
            .when("1 min");
        if unread {
            n.unread()
        } else {
            n
        }
    }))
    .on_read(|s, id| {
        s.items
            .iter_mut()
            .filter(|i| i.0 == id)
            .for_each(|i| i.1 = false)
    })
    .on_read_all(|s| s.items.iter_mut().for_each(|i| i.1 = false))
}

#[test]
fn notification_center_marks_read() {
    let mut h = Harness::new(
        sketch(
            Inbox {
                items: vec![(1, true), (2, true), (3, false)],
            },
            inbox_view,
        ),
        420.0,
        480.0,
    );
    h.expect("Notifications")
        .expect("Mark all read")
        .expect("2");
    h.click("Item 1");
    h.expect("1");
    h.click("Mark all read");
    h.expect_no("Mark all read");
    golden(&h, 420, 480, "notifications_all_read", false);
}

#[test]
fn notification_center_empty_state() {
    let mut h = Harness::new(sketch(Inbox { items: vec![] }, inbox_view), 420.0, 480.0);
    h.expect("You are all caught up");
}

// ------------------------------------------------------------------ pages

fn pages_view(_: &()) -> impl View<Act<()>> {
    col![
        page_header("Deployments")
            .subtitle("Production, last 7 days")
            .crumbs(breadcrumb::<()>(["Projects", "Alloy", "Deployments"]))
            .badge(badge("Live").success().dot())
            .actions([button("Export").act(|_| {})]),
        tracker::<Act<()>>(["Cart", "Shipping", "Payment", "Done"], 2),
        settings_page::<Act<()>>("Settings").w(420.0).section(
            "Sync",
            [
                setting("Wi-Fi only", "Save mobile data.", toggle("", true)),
                setting("Digest", "", toggle("", false))
            ],
        ),
        row![
            empty_state::<Act<()>>(Art::Inbox, "All caught up", "Nothing new."),
            empty_state::<Act<()>>(Art::Search, "No matches", "Try again."),
            empty_state::<Act<()>>(Art::Folder, "Empty", "Drop files."),
            empty_state::<Act<()>>(Art::Offline, "Offline", "Later."),
            empty_state::<Act<()>>(Art::Chart, "No data", "Connect."),
        ],
    ]
    .gap(16.0)
    .pad(16.0)
}

#[test]
fn page_modules_render() {
    for dark in [false, true] {
        let h = themed(sketch((), pages_view), 1120.0, 620.0, dark);
        for t in [
            "Deployments",
            "Production, last 7 days",
            "Cart",
            "Payment",
            "Wi-Fi only",
            "All caught up",
            "Offline",
        ] {
            assert!(h.has_text(t), "missing {t}: {:?}", h.texts());
        }
        golden(
            &h,
            1120,
            620,
            if dark { "pages_dark" } else { "pages_light" },
            dark,
        );
    }
}

#[test]
fn breadcrumb_click_reports_index() {
    let mut h = Harness::new(
        sketch(String::new(), |s: &String| {
            col![
                breadcrumb::<String>(["Home", "Projects", "Alloy"])
                    .act(|s, i| *s = format!("went {i}")),
                s.clone(),
            ]
        }),
        420.0,
        120.0,
    );
    h.click("Projects");
    h.expect("went 1");
    h.click("Home");
    h.expect("went 0");
}

// ------------------------------------------------------------- auth + chat

#[derive(Default)]
struct Auth {
    got: String,
    forgot: bool,
}

#[test]
fn login_validates_then_submits() {
    let mut h = Harness::new(
        sketch(Auth::default(), |s: &Auth| {
            col![
                login::<Auth>("Alloy")
                    .on_submit(|s, e, p| s.got = format!("{e}|{p}"))
                    .on_forgot(|s| s.forgot = true),
                s.got.clone(),
            ]
        }),
        520.0,
        640.0,
    );
    h.expect("Sign in to Alloy");
    h.click("Sign in");
    h.expect("Enter an email like name@company.com")
        .expect("At least 8 characters");
    h.fill("name@company.com", "mira@alloy.dev");
    h.fill("Your password", "hunter2");
    h.click("Sign in");
    h.expect_no("mira@alloy.dev|hunter2");
    h.fill("•••••••", "hunter2hunter2");
    h.click("Sign in");
    h.expect("mira@alloy.dev|hunter2hunter2");
    h.click("Forgot password?");
}

#[test]
fn login_sign_up_asks_for_a_name() {
    let mut h = Harness::new(
        sketch((), |_: &()| {
            login::<()>("Alloy").create_account().providers(["GitHub"])
        }),
        520.0,
        720.0,
    );
    h.expect("Create your Alloy account")
        .expect("Name")
        .expect("Continue with GitHub")
        .expect("Create account");
    golden(&h, 520, 720, "login_signup", false);
}

struct Room {
    log: Vec<(bool, String)>,
}

fn room_view(s: &Room) -> impl View<Act<Room>> {
    col![
        chat::<Act<Room>>(s.log.iter().map(|(m, t)| {
            if *m {
                reply(t.clone(), "now")
            } else {
                message("Jon Reyes", t.clone(), "10:41")
            }
        })),
        composer::<Room>().on_send(|s, t| s.log.push((true, t))),
    ]
    .gap(12.0)
    .pad(16.0)
}

#[test]
fn chat_composer_sends_and_clears() {
    let mut h = Harness::new(
        sketch(
            Room {
                log: vec![(false, "Did it pass?".into())],
            },
            room_view,
        ),
        520.0,
        420.0,
    );
    h.expect("Jon Reyes").expect("Did it pass?");
    h.fill("Write a message", "Yes, all green");
    h.click("Send");
    h.expect("Yes, all green").expect_no("Yes, all green ");
    golden(&h, 520, 420, "chat_after_send", false);
}

// --------------------------------------------------------- plans + files

#[derive(Default)]
struct Shop {
    plan: usize,
    opened: String,
}

fn shop_view(s: &Shop) -> impl View<Act<Shop>> {
    col![
        plan_picker::<Shop>([
            plan("Hobby", 0.0).features(["1 project"]),
            plan("Team", 12.0)
                .features(["10 seats", "Priority support"])
                .recommended(),
        ])
        .on_choose(|s, i| s.plan = i),
        format!("plan {}", s.plan),
        file_browser::<Shop>([
            folder("Designs", "Today"),
            file("main.rs", "3 KB", "Today"),
            file("logo.png", "48 KB", "Nov 19")
        ])
        .path(["Home", "Alloy"])
        .on_open(|s, n| s.opened = n.to_string()),
        format!("opened {}", s.opened),
    ]
    .gap(14.0)
    .pad(16.0)
}

#[test]
fn plan_picker_chooses_and_switches_billing() {
    let mut h = Harness::new(sketch(Shop::default(), shop_view), 760.0, 640.0);
    h.expect("Most popular").expect("$12").expect("Selected");
    h.click("Choose Hobby");
    h.expect("plan 0");
    h.click("Yearly");
    h.expect("$9.60");
    golden(&h, 760, 640, "plans_yearly", false);
}

#[test]
fn file_browser_selects_then_opens_and_switches_to_grid() {
    let mut h = Harness::new(sketch(Shop::default(), shop_view), 760.0, 640.0);
    h.expect("Designs").expect("3 items");
    h.click("main.rs");
    h.expect("opened").expect_no("opened main.rs");
    h.click("main.rs");
    h.expect("opened main.rs");
    h.click("Grid");
    h.expect("logo.png");
    golden(&h, 760, 640, "files_grid", false);
}

#[test]
fn gallery_grid_renders_tiles() {
    let h = Harness::new(
        sketch((), |_: &()| {
            gallery_grid::<()>([
                tile("Dusk", "2 MB"),
                tile("Harbour", "3 MB"),
                tile("Fog", "1 MB"),
            ])
            .cols(3)
        }),
        640.0,
        200.0,
    );
    for t in ["Dusk", "Harbour", "Fog", "2 MB"] {
        assert!(h.has_text(t));
    }
    golden(&h, 640, 200, "gallery_grid", false);
}

// ------------------------------------------------------------------- tour

fn tour_view(_: &()) -> impl View<Act<()>> {
    let t = tour(2);
    col![
        t.spot(
            0,
            button("New project").primary().act(|_| {}),
            "Start here",
            "Create a project."
        ),
        gap(0.0, 80.0),
        t.spot(
            1,
            button("Share").act(|_| {}),
            "Invite people",
            "Share a link."
        ),
    ]
    .pad(24.0)
}

#[test]
fn tour_steps_through_and_finishes() {
    let mut h = Harness::new(sketch((), tour_view), 520.0, 420.0);
    h.expect("1 of 2").expect("Start here");
    golden(&h, 520, 420, "tour_step_one", false);
    h.click("Next");
    h.advance(10)
        .advance(400)
        .expect("2 of 2")
        .expect("Invite people")
        .expect("Back");
    h.click("Back");
    h.advance(10).advance(400).expect("Start here");
    h.click("Next");
    h.advance(10).advance(400);
    h.click("Done");
    h.advance(10)
        .advance(400)
        .expect_no("Invite people")
        .expect_no("Start here");
}

#[test]
fn tour_skip_closes_it_and_escape_too() {
    let mut h = Harness::new(sketch((), tour_view), 520.0, 420.0);
    h.click("Skip");
    h.advance(10).advance(400).expect_no("Start here");
    let mut h = Harness::new(sketch((), tour_view), 520.0, 420.0);
    h.key("Escape");
    h.advance(10).advance(400).expect_no("Start here");
}

// ------------------------------------------------------------ command bar

#[derive(Default)]
struct Cmd {
    picked: String,
}

fn bar_view(s: &Cmd) -> impl View<Act<Cmd>> {
    col![
        command_bar::<Cmd>([
            hit("Actions", "New project").keys("Ctrl+N"),
            hit("Actions", "Open file"),
            hit("Go to", "Settings"),
        ])
        .on_pick(|s, i| s.picked = format!("picked {i}")),
        s.picked.clone(),
    ]
    .gap(10.0)
    .pad(16.0)
}

#[test]
fn command_bar_filters_and_picks() {
    let mut h = Harness::new(sketch(Cmd::default(), bar_view), 640.0, 420.0);
    h.expect("Actions").expect("Go to").expect("New project");
    golden(&h, 640, 420, "command_bar", false);
    h.fill("Search or run a command", "sett");
    h.expect("Settings").expect_no("New project");
    h.click("Settings");
    h.expect("picked 2");
    h.fill("sett", "zzzz");
    h.expect("No matches");
}

// ----------------------------------------------------------------- toasts

#[test]
fn toast_tones_show_their_text() {
    let mut h = Harness::new(
        sketch((), |_: &()| {
            row![
                button("a").act(|_| {
                    toast("Saved").success();
                }),
                button("b").act(|_| {
                    toast("Almost full").warning();
                }),
                button("c").act(|_| {
                    toast("New version").info();
                }),
            ]
            .pad(16.0)
        }),
        420.0,
        200.0,
    );
    h.click("a").click("b").click("c");
    h.advance(10).advance(300);
    for t in ["Saved", "Almost full", "New version"] {
        assert!(h.has_text(t), "{t}: {:?}", h.texts());
    }
    golden(&h, 420, 200, "toast_tones", false);
}

#[allow(dead_code)]
fn _tones(_: Tone) {}
