//! Ready-made modules: KPI cards, gauges, timelines, notifications, settings, sign-in, chat,
//! plans, files and a spotlight tour. Render one tab headless with
//! `cargo run -p alloy-ui --example modules -- --headless out.png --click Feeds`.
use alloy_ui::prelude::*;
use alloy_ui::widgets::Tone;

const TABS: [&str; 6] = [
    "Overview",
    "Feeds",
    "Pages",
    "Auth & chat",
    "Files & plans",
    "Tour",
];

struct Gallery {
    tab: usize,
    notices: Vec<(u64, &'static str, &'static str, &'static str, bool, Tone)>,
    chat: Vec<(bool, &'static str, String, &'static str)>,
    dark_mode: bool,
    sync: bool,
    digest: bool,
}

impl Default for Gallery {
    fn default() -> Self {
        Self {
            tab: 0,
            notices: vec![
                (
                    1,
                    "Build failed on main",
                    "3 tests failed in alloy-ui-widgets",
                    "2 min",
                    true,
                    Tone::Danger,
                ),
                (
                    2,
                    "Mira Okafor commented",
                    "\"Can we ship the gauge before the demo?\"",
                    "18 min",
                    true,
                    Tone::Accent,
                ),
                (
                    3,
                    "Deploy finished",
                    "v0.4.2 is live in 3 of 3 regions",
                    "1 h",
                    false,
                    Tone::Success,
                ),
                (
                    4,
                    "Storage at 82%",
                    "Add capacity before the weekend",
                    "Yesterday",
                    false,
                    Tone::Warning,
                ),
            ],
            chat: vec![
                (
                    false,
                    "Jon Reyes",
                    "Did the golden tests pass on Windows?".into(),
                    "10:41",
                ),
                (
                    false,
                    "Jon Reyes",
                    "Asking because the font stack differs.".into(),
                    "10:41",
                ),
                (
                    true,
                    "",
                    "Yes, all green. Shapes only, no glyphs.".into(),
                    "10:43",
                ),
                (
                    false,
                    "Mira Okafor",
                    "Great. Merging the module PR now.".into(),
                    "10:44",
                ),
            ],
            dark_mode: false,
            sync: true,
            digest: false,
        }
    }
}

fn overview(_: &Gallery) -> impl View<Act<Gallery>> {
    let spend = [12.0, 14.0, 13.0, 17.0, 16.0, 21.0, 20.0, 24.0, 23.0, 28.0];
    let lat = [92.0, 88.0, 90.0, 84.0, 86.0, 79.0, 81.0, 74.0];
    col![
        page_header("Overview")
            .subtitle("Production, last 7 days")
            .crumbs(breadcrumb::<Gallery>(["Projects", "Alloy", "Overview"]))
            .badge(badge("Live").success().dot())
            .actions([button("Export"), button("New report").primary().act(|_| {})]),
        kpi_row([
            kpi("Revenue", "$48.2k")
                .delta(12.4)
                .hint("vs last week")
                .trend(spend),
            kpi("Active users", "3,481")
                .delta(4.1)
                .hint("vs last week")
                .trend(spend.iter().rev().copied()),
            kpi("p95 latency", "74 ms")
                .delta(-8.2)
                .lower_is_better()
                .hint("vs last week")
                .trend(lat),
            kpi("Error rate", "0.42%")
                .delta(0.0)
                .hint("steady")
                .trend([0.4, 0.5, 0.4, 0.45, 0.42, 0.43, 0.42]),
        ]),
        row![
            col![
                text("Capacity").label(),
                row![
                    gauge(0.42).label("CPU").bands(),
                    gauge(0.78).label("Memory").bands(),
                    gauge(0.93).label("Disk").bands(),
                    radial(0.64).label("Uploaded").size(120.0),
                ]
                .gap(18.0),
            ]
            .gap(12.0)
            .card()
            .fill_w(),
            col![
                text("Team").label(),
                row![
                    avatar_group([
                        "Mira Okafor",
                        "Jon Reyes",
                        "Sam Lee",
                        "Ana Cruz",
                        "Dev Patel",
                        "Kim Ho"
                    ])
                    .max(4),
                    caption("6 online").dim(),
                ]
                .gap(12.0)
                .align_center(),
                rule(),
                row![
                    avatar("Mira Okafor").presence(Presence::Online).size(36.0),
                    avatar("Jon Reyes").presence(Presence::Away).size(36.0),
                    avatar("Sam Lee").presence(Presence::Busy).size(36.0),
                    avatar("Ana Cruz").presence(Presence::Offline).size(36.0),
                ]
                .gap(10.0),
                rule(),
                activity_feed([
                    activity("Mira Okafor", "merged", "Add gauges", "4 min"),
                    activity("Jon Reyes", "commented on", "Q4 plan", "21 min"),
                    activity("Sam Lee", "deployed", "v0.4.2", "1 h"),
                ]),
            ]
            .gap(12.0)
            .card()
            .w(380.0),
        ]
        .gap(12.0),
    ]
    .gap(16.0)
}

fn feeds(s: &Gallery) -> impl View<Act<Gallery>> {
    let items = s.notices.iter().map(|&(id, t, b, w, unread, tone)| {
        let n = notice(id, t).body(b).when(w).tone(tone);
        if unread {
            n.unread()
        } else {
            n
        }
    });
    row![
        col![
            text("Timeline").label(),
            moments([
                moment("12:04", "Deploy finished")
                    .detail("v0.4.2 is live in 3 of 3 regions.")
                    .success(),
                moment("11:52", "Canary at 10%")
                    .detail("Error rate steady at 0.4%.")
                    .accent(),
                moment("11:30", "Build passed").detail("212 tests, 0 warnings."),
                moment("10:18", "Rollback of v0.4.1")
                    .detail("Scroll jitter on Windows.")
                    .danger(),
            ]),
            gap(0.0, 8.0),
            text("Changelog").label(),
            changelog([
                release("0.4.0", "Nov 21")
                    .latest()
                    .note("Ready-made modules for dashboards, feeds and sign-in.")
                    .added([
                        "KPI cards, gauges and radial progress",
                        "Spotlight tour",
                        "Notification centre"
                    ])
                    .fixed(["Toasts no longer overlap the palette"]),
                release("0.3.2", "Nov 14").changed(["Hover overlays use the text colour"]),
            ]),
        ]
        .gap(14.0)
        .card()
        .fill_w(),
        col![
            row![
                button("Success").act(|_| {
                    toast("Report exported").success();
                }),
                button("Warning").act(|_| {
                    toast("Storage at 82%").warning();
                }),
                button("Info").act(|_| {
                    toast("Version 0.4.2 is ready").info();
                }),
                button("Error").act(|_| {
                    toast("Sync failed").error();
                }),
            ]
            .gap(8.0),
            notification_center::<Gallery>(items)
                .on_read(|s, id| {
                    if let Some(n) = s.notices.iter_mut().find(|n| n.0 == id) {
                        n.4 = false;
                    }
                })
                .on_read_all(|s| s.notices.iter_mut().for_each(|n| n.4 = false)),
        ]
        .gap(12.0),
    ]
    .gap(16.0)
}

fn pages(s: &Gallery) -> impl View<Act<Gallery>> {
    row![
        settings_page("Settings")
            .intro("Appearance and sync for this device.")
            .w(520.0)
            .section(
                "Appearance",
                [
                    setting(
                        "Dark mode",
                        "Follow the system or pick one.",
                        toggle("", at!(s.dark_mode))
                    ),
                    setting(
                        "Accent colour",
                        "Used for focus, links and primary buttons.",
                        swatch(alloy_ui::rgb(0x3558A8))
                    ),
                ]
            )
            .section_with(
                "Sync",
                "Keeps notes and themes the same on every device.",
                [
                    setting("Sync over Wi-Fi only", "", toggle("", at!(s.sync))),
                    setting(
                        "Weekly digest",
                        "A summary every Monday.",
                        toggle("", at!(s.digest))
                    ),
                ]
            ),
        col![
            empty_state::<Act<Gallery>>(
                Art::Inbox,
                "You are all caught up",
                "New mail shows up here."
            )
            .action(button("Compose").primary().act(|_| {})),
            row![
                empty_state::<Act<Gallery>>(Art::Search, "No matches", "Try fewer words."),
                empty_state::<Act<Gallery>>(Art::Folder, "Empty folder", "Drop files here."),
            ],
            row![
                empty_state::<Act<Gallery>>(Art::Offline, "You are offline", "Changes sync later."),
                empty_state::<Act<Gallery>>(Art::Chart, "No data yet", "Connect a source."),
            ],
        ]
        .gap(4.0)
        .card()
        .fill_w(),
    ]
    .gap(20.0)
}

fn auth_chat(s: &Gallery) -> impl View<Act<Gallery>> {
    let msgs = s.chat.iter().map(|(mine, who, t, w)| {
        if *mine {
            reply(t.clone(), *w)
        } else {
            message(*who, t.clone(), *w)
        }
    });
    row![
        login::<Gallery>("Alloy")
            .subtitle("Welcome back. Pick up where you left off.")
            .providers(["GitHub"])
            .on_forgot(|_| {}),
        col![
            text("#modules").label(),
            chat::<Act<Gallery>>(msgs),
            composer::<Gallery>().on_send(|s, t| s.chat.push((true, "", t, "now"))),
        ]
        .gap(14.0)
        .card()
        .w(520.0),
    ]
    .gap(24.0)
}

fn commerce(s: &Gallery) -> impl View<Act<Gallery>> {
    let _ = s;
    col![
        plan_picker::<Gallery>([
            plan("Hobby", 0.0)
                .about("For side projects.")
                .features(["1 project", "Community support"]),
            plan("Team", 12.0)
                .about("For small teams shipping weekly.")
                .features(["Unlimited projects", "10 seats", "Priority support"])
                .recommended(),
            plan("Studio", 39.0)
                .about("For agencies and larger teams.")
                .features(["Unlimited seats", "Audit log", "Single sign-on"]),
        ]),
        file_browser::<Gallery>([
            folder("Designs", "Today"),
            folder("Releases", "Nov 18"),
            file("logo.png", "48 KB", "Yesterday"),
            file("main.rs", "3.1 KB", "Today"),
            file("notes.md", "820 B", "Nov 19"),
            file("budget.pdf", "1.2 MB", "Nov 12"),
        ])
        .path(["Home", "Projects", "Alloy"]),
    ]
    .gap(20.0)
}

fn command(s: &Gallery) -> impl View<Act<Gallery>> {
    let _ = s;
    command_bar::<Gallery>([
        hit("Actions", "New project").keys("Ctrl+N"),
        hit("Actions", "Open file").keys("Ctrl+O"),
        hit("Actions", "Toggle dark mode"),
        hit("Go to", "Settings").keys("Ctrl+,"),
        hit("Go to", "Deployments"),
    ])
}

fn tour_tab(_: &Gallery) -> impl View<Act<Gallery>> {
    let t = tour(3);
    col![
        row![
            t.spot(
                0,
                button("New project").primary(),
                "Start here",
                "Create a project to get a dashboard, a board and a changelog."
            ),
            space(),
            t.spot(
                1,
                input("").hint("Search"),
                "Find anything",
                "Press Ctrl+K from anywhere to jump to a page or run a command."
            ),
        ]
        .align_center(),
        gap(0.0, 120.0),
        t.spot(
            2,
            kpi("Revenue", "$48.2k")
                .delta(12.4)
                .hint("vs last week")
                .w(300.0),
            "Numbers that move",
            "Cards show the change and a trend, coloured by whether it is good news."
        ),
    ]
    .gap(16.0)
}

fn view(s: &Gallery) -> impl View<Act<Gallery>> {
    set_dark(s.dark_mode);
    let page: Node<Act<Gallery>> = match s.tab {
        0 => scroll(overview(s)).fill().into(),
        1 => scroll(feeds(s)).fill().into(),
        2 => scroll(pages(s)).fill().into(),
        3 => scroll(auth_chat(s)).fill().into(),
        4 => scroll(commerce(s)).fill().into(),
        _ => col![tour_tab(s), command(s)].gap(24.0).into(),
    };
    col![tab_bar(TABS, at!(s.tab)), page]
        .gap(14.0)
        .pad(20.0)
        .fill()
}

fn main() {
    // `--dark` starts in the dark theme; the Dark mode switch on the Pages tab changes it live.
    let gallery = Gallery {
        dark_mode: std::env::args().any(|a| a == "--dark"),
        ..Gallery::default()
    };
    sketch(gallery, view).run_sized("Modules", 1120.0, 820.0)
}
