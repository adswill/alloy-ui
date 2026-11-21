//! Alloy Studio: one app that uses nearly everything Alloy ships.
//!
//! ```text
//! cargo run --release -p alloy-ui --example alloy_studio
//! ```
//!
//! Ten pages behind a sidebar and a router, a command palette, native menus, a tray icon,
//! a second window, undo with toasts, five languages' worth of plumbing (three catalogs,
//! one right-to-left), ten themes and the F12 inspector. See `RUN.md` for a tour.
//!
//! * `main.rs`: state, the shell (sidebar, top bar, router), menus and windows.
//! * `data.rs`: deterministic sample data.
//! * `ui.rs`: the few layout helpers every page shares.
//! * `pages/`: one file per page.
use alloy_ui::prelude::*;
use alloy_ui::widgets::Date;
use alloy_ui::{
    i18n, CalView, Desktop, DesktopSpec, Entry, GraphNode, IconPack, Lane, Meta, Ring, Snap, Wire,
};
use std::sync::Arc;

mod data;
mod pages;
mod ui;

use data::*;

/// Every screen. `Customer` is a drill-down reached from the Data page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Route {
    Overview,
    Data,
    Editors,
    Planner,
    Canvas,
    Forms,
    Workbench,
    Motion,
    Appearance,
    System,
    Customer(usize),
}

impl Route {
    /// Pages in the sidebar, grouped, with their translation key.
    const GROUPS: [(&'static str, &'static [Route]); 4] = [
        ("group.insight", &[Route::Overview, Route::Data]),
        (
            "group.make",
            &[Route::Editors, Route::Planner, Route::Canvas, Route::Forms],
        ),
        ("group.explore", &[Route::Workbench, Route::Motion]),
        ("group.system", &[Route::Appearance, Route::System]),
    ];

    pub fn key(self) -> &'static str {
        match self {
            Route::Overview => "nav.overview",
            Route::Data => "nav.data",
            Route::Editors => "nav.editors",
            Route::Planner => "nav.planner",
            Route::Canvas => "nav.canvas",
            Route::Forms => "nav.forms",
            Route::Workbench => "nav.workbench",
            Route::Motion => "nav.motion",
            Route::Appearance => "nav.appearance",
            Route::System => "nav.system",
            Route::Customer(_) => "nav.customer",
        }
    }

    pub fn label(self) -> String {
        i18n::translate(self.key(), &[])
    }

    /// The sidebar row that stays lit while a drill-down is open.
    fn section(self) -> Route {
        match self {
            Route::Customer(_) => Route::Data,
            r => r,
        }
    }
}

/// A checklist item on the overview page.
#[derive(Clone, PartialEq)]
pub struct Task {
    pub id: u32,
    pub title: String,
    pub done: bool,
}

/// Everything undo covers: tasks, the board and the calendar. `mod+z` walks back through it.
#[derive(Clone, PartialEq)]
pub struct Work {
    pub tasks: Vec<Task>,
    pub lanes: Vec<Lane>,
    pub entries: Vec<Entry>,
}

fn work() -> Work {
    let t = |id, title: &str, done| Task {
        id,
        title: title.into(),
        done,
    };
    Work {
        tasks: vec![
            t(1, "Review the pricing page copy", false),
            t(2, "Reply to Radia about the multi-region rollout", false),
            t(3, "Ship the 0.4 release notes", true),
            t(4, "Check idle CPU on the new Mac mini", false),
        ],
        lanes: lanes(),
        entries: entries(),
    }
}

const DENSITIES: [Density; 3] = [Density::Compact, Density::Comfortable, Density::Spacious];
const EFFECTS: [(&str, WindowEffect); 4] = [
    ("None", WindowEffect::None),
    ("Blur", WindowEffect::Blur),
    ("Mica", WindowEffect::Mica),
    ("Glass", WindowEffect::Transparent),
];

state! {
    pub struct Studio {
        // look and feel
        dark: bool,
        preset: usize = 1,
        accent: Option<Color>,
        picker: Color = Color::hex(0x1F5FD6),
        density: usize = 1,
        contrast: bool,
        phosphor: bool,
        effect: usize,
        // overview
        range: usize = 1,
        cpu: Ring = ring(60, 1, 42.0, 30.0),
        mem: Ring = ring(60, 77, 58.0, 12.0),
        market: Vec<alloy_ui::Candle> = market(),
        slice: Option<usize>,
        work: Work = work(),
        // data
        orders: Arc<Vec<Order>> = Arc::new(orders(100_000)),
        picked_rows: usize,
        data_tab: usize,
        customers: Vec<Customer> = customers(),
        customer_filter: String,
        // editors
        editors_tab: usize,
        src: String = include_str!("pages/editors.rs").to_string(),
        doc: RichText = RichText::from_markdown(pages::editors::NOTES),
        md: String = pages::editors::MARKDOWN.to_string(),
        opened_link: String,
        // planner
        planner_tab: usize,
        opened_card: Option<u64>,
        day: Date = TODAY,
        cal_view: CalView = CalView::Week,
        // canvas
        canvas_tab: usize,
        nodes: Vec<GraphNode> = graph().0,
        wires: Vec<Wire> = graph().1,
        selected_node: Option<u64>,
        doodle: Vec<(f32, f32)>,
        play: alloy_ui::Playback,
        // forms
        forms_tab: usize,
        form: pages::forms::Profile,
        step: usize,
        plan: pages::forms::Plan,
        yearly: bool,
        terms: bool,
        name: String,
        email: String,
        password: String,
        // workbench
        wb_tab: usize,
        wb_page: usize,
        docs: Vec<String> = ["Notes", "Build log", "Config", "Deploy plan", "Metrics", "Alerts", "Runbook"].map(String::from).to_vec(),
        file: Option<u64>,
        build: usize = 11,
        only_failed: bool,
        inbox: Vec<String> = ["Brief.pdf", "Mockups.fig", "Budget.xlsx"].map(String::from).to_vec(),
        archive: Vec<String> = ["Old plan.md"].map(String::from).to_vec(),
        // motion
        details: bool,
        wide: bool,
        synced: bool,
        spring_v: f32 = 40.0,
        // system
        dropped: Vec<String>,
        picked_file: String,
        clip_note: String,
        last_url: String,
        activity: Vec<String>,
        monitor: bool,
        monitor_open: bool,
        // desktop
        desktop: Desktop<Studio> = build_desktop(),
    }
}

impl Studio {
    fn look(&self) -> Look {
        let mut look = Look::light()
            .is_dark(self.dark)
            .preset(Preset::ALL[self.preset])
            .high_contrast(self.contrast)
            .density(DENSITIES[self.density]);
        if let Some(c) = self.accent {
            look = look.accent(c);
        }
        look
    }

    pub fn log(&mut self, what: impl Into<String>) {
        self.activity.insert(0, what.into());
        self.activity.truncate(8);
    }

    pub fn orders_csv(&self) -> String {
        let mut out = String::from("id,customer,region,status,total\n");
        for o in self.orders.iter().take(5000) {
            out += &format!(
                "{},{},{},{},{:.2}\n",
                o.id, o.customer, o.region, o.status, o.total
            );
        }
        out
    }
}

/// Native menus, tray and the global hotkey. Every item is also a command in the palette.
fn build_desktop() -> Desktop<Studio> {
    let mut d = Desktop::default();
    d.menu("File", |m| {
        m.item("New monitor window", "mod+n", |s: &mut Studio| {
            s.monitor = !s.monitor
        })
        .item("Export orders as CSV...", "mod+e", |s: &mut Studio| {
            pages::system::export_csv(s)
        })
        .item("Open a file...", "mod+o", |s: &mut Studio| {
            pages::system::open_file(s)
        })
        .separator()
        .item("Quit", "mod+q", |_| quit())
    });
    d.menu("View", |m| {
        m.item("Toggle dark mode", "mod+shift+d", |s: &mut Studio| {
            s.dark = !s.dark
        })
        .item("Denser layout", "mod+-", |s: &mut Studio| {
            s.density = s.density.saturating_sub(1)
        })
        .item("Roomier layout", "mod+=", |s: &mut Studio| {
            s.density = (s.density + 1).min(2)
        })
        .sub("Theme", |m| {
            Preset::ALL.iter().enumerate().fold(m, |m, (i, p)| {
                m.item(p.name(), "", move |s: &mut Studio| {
                    s.preset = i;
                    s.accent = None
                })
            })
        })
    });
    d.menu("Help", |m| {
        m.item("Send a test notification", "", |s: &mut Studio| {
            pages::system::notify_now(s)
        })
    });
    d.tray("Alloy Studio", |t| {
        t.click(|s: &mut Studio| {
            toast("Hello from the tray");
            s.log("Tray icon clicked");
        })
        .item("Toggle dark mode", |s: &mut Studio| s.dark = !s.dark)
        .item("Monitor window", |s: &mut Studio| s.monitor = !s.monitor)
        .separator()
        .item("Quit", |_| quit())
    });
    d.global_hotkey("ctrl+alt+s", |s: &mut Studio| {
        toast("Global hotkey pressed");
        s.log("Global hotkey Ctrl+Alt+S");
    });
    d
}

impl App for Studio {
    type Msg = Act<Studio>;

    fn update_cmd(&mut self, msg: Act<Studio>) -> Cmd<Act<Studio>> {
        let before = self.monitor_open;
        msg.apply(self);
        match (before, self.monitor) {
            (false, true) => {
                self.monitor_open = true;
                Cmd::open_window(1, "Alloy Studio · Monitor", 460.0, 520.0)
            }
            (true, false) => {
                self.monitor_open = false;
                Cmd::close_window(1)
            }
            _ => Cmd::none(),
        }
    }

    fn view(&self) -> impl View<Act<Studio>> {
        shell(self)
    }

    fn view_window(&self, id: u64) -> impl View<Act<Studio>> {
        if id == 0 {
            return shell(self).boxed();
        }
        pages::system::monitor(self).boxed()
    }

    fn shortcuts(&self) -> Vec<Hotkey<Act<Studio>>> {
        self.desktop.hotkeys()
    }

    fn desktop(&self) -> DesktopSpec {
        self.desktop.spec()
    }

    fn snapshot(&self) -> Option<Snap> {
        Some(Snap::new(self.work.clone()))
    }

    fn restore(&mut self, snap: Snap) {
        if let Some(w) = snap.take::<Work>() {
            self.work = w;
        }
    }

    fn meta(&self, what: Meta) -> Option<Act<Studio>> {
        Some(match what {
            Meta::Undo => Act::new(|_: &mut Studio| undo()),
            Meta::Redo => Act::new(|_: &mut Studio| redo()),
        })
    }

    fn on_drop(&self, files: Vec<std::path::PathBuf>) -> Option<Act<Studio>> {
        Some(Act::new(move |s: &mut Studio| {
            for f in &files {
                s.dropped.push(f.display().to_string());
            }
            s.log(format!("Dropped {} file(s)", files.len()));
            toast(format!("Received {} file(s)", files.len()));
        }))
    }

    fn on_open_url(&self, url: String) -> Option<Act<Studio>> {
        Some(Act::new(move |s: &mut Studio| {
            s.last_url = url.clone();
            s.log(format!("Opened {url}"));
            toast("Deep link received");
        }))
    }

    fn single_instance(&self) -> Option<String> {
        Some("dev.alloyui.studio".into())
    }

    fn url_schemes(&self) -> Vec<String> {
        vec!["alloy-studio".into()]
    }

    fn window_effect(&self) -> WindowEffect {
        EFFECTS[self.effect].1
    }

    fn subscriptions(&self) -> Vec<alloy_ui::Sub<Act<Studio>>> {
        vec![every(secs(1.0), || {
            Act::new(|s: &mut Studio| {
                let n = s.cpu.len() as u32 + 100;
                let last = s.cpu.last().unwrap_or(40.0);
                s.cpu
                    .push((last + (noise(n) - 0.5) * 26.0).clamp(4.0, 96.0));
                let m = s.mem.last().unwrap_or(58.0);
                s.mem
                    .push((m + (noise(n + 9) - 0.5) * 6.0).clamp(30.0, 92.0));
            })
        })]
    }
}

// ------------------------------------------------------------------ shell

fn shell(s: &Studio) -> impl View<Act<Studio>> {
    set_look(s.look());
    set_icon_pack(if s.phosphor {
        IconPack::Phosphor
    } else {
        IconPack::Lucide
    });
    let nav = router(Route::Overview);
    let current = nav.current();

    // Every page is a palette command and a shortcut (mod+1 .. mod+0), listed in the menu bar.
    let mut all: Vec<Route> = Vec::new();
    for (_, rs) in Route::GROUPS {
        all.extend(rs.iter().copied());
    }

    let sidebar = col![
        col![heading("Alloy Studio"), caption(t!("brand.tagline")).dim(),]
            .gap(2.0)
            .pad_xy(14.0, 6.0),
        button(t!("palette.open"))
            .ghost()
            .fill_w()
            .act(|_| alloy_ui::open_palette()),
        col(Route::GROUPS.iter().map(|(key, routes)| {
            col![
                caption(i18n::translate(key, &[])).dim().pad_xy(14.0, 0.0),
                col(routes.iter().map(|&r| {
                    button(r.label())
                        .nav(current.section() == r)
                        .fill_w()
                        .act(move |_| nav.reset(r))
                }))
                .gap(1.0),
            ]
            .gap(4.0)
        }))
        .gap(14.0),
        vspace(),
        row![
            col![
                text(t!("user.name")).label(),
                caption(t!("user.role")).dim()
            ]
            .gap(1.0)
            .fill_w(),
            button(if s.dark {
                t!("top.dark")
            } else {
                t!("top.light")
            })
            .ghost()
            .act(|s: &mut Studio| s.dark = !s.dark),
        ]
        .align_center()
        .pad_xy(14.0, 4.0),
    ]
    .gap(12.0)
    .pad(10.0)
    .w(236.0)
    .fill_h();

    let body = nav.show(|route| pages::page(s, route, nav));

    let top = row![
        nav.back_button(),
        button(t!("top.forward"))
            .ghost()
            .enabled(nav.can_forward())
            .act(move |_| nav.forward()),
        nav.breadcrumbs(|r| r.label()),
        space(),
        popover(
            button(t!("top.language")),
            col![
                caption(t!("top.language")).dim(),
                i18n::language_picker::<Studio>(&[
                    ("en", "English"),
                    ("fr", "Français"),
                    ("ar", "العربية"),
                ]),
            ]
            .gap(8.0),
        ),
        button(if s.monitor {
            t!("top.close_monitor")
        } else {
            t!("top.monitor")
        })
        .act(|s: &mut Studio| s.monitor = !s.monitor),
    ]
    .gap(8.0)
    .align_center()
    .pad_xy(20.0, 10.0);

    let mut page = row![sidebar, col![top, rule(), body.fill()].gap(0.0).fill(),]
        .gap(0.0)
        .fill();
    for (i, r) in all.iter().copied().enumerate() {
        let keys = format!("mod+{}", (i + 1) % 10);
        page = page.shortcut_as(&keys, &format!("Go to {}", r.label()), move |_| {
            nav.reset(r)
        });
    }
    page
}

fn main() {
    // The F12 inspector is on in debug builds; this turns it on in release builds too.
    if std::env::var_os("ALLOY_INSPECT").is_none() {
        std::env::set_var("ALLOY_INSPECT", "1");
    }
    i18n::add("en", include_str!("i18n/en.txt"));
    i18n::add("fr", include_str!("i18n/fr.txt"));
    i18n::add("ar", include_str!("i18n/ar.txt"));
    i18n::set_locale(&i18n::detect());
    let mut studio = Studio {
        dark: std::env::var_os("STUDIO_DARK").is_some(),
        ..Studio::default()
    };
    if let Some(p) = std::env::var("STUDIO_PRESET")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        studio.preset = p;
    }
    studio.run_sized("Alloy Studio", 1360.0, 860.0)
}
