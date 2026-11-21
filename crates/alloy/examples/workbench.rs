//! Workbench: a split pane, a 100 000-row list, tabs with overflow, a context menu, Markdown
//! and a popover, in one small app.
use alloy_ui::prelude::*;

const BUILDS: usize = 100_000;
const SERVICES: [&str; 6] = [
    "api-gateway",
    "billing",
    "search",
    "auth",
    "web-app",
    "ingest",
];
const STATUS: [&str; 5] = ["Done", "Done", "Done", "Failed", "Pending"];

const NOTES: &str = "# Build 99,988\n\nShipped **api-gateway** to production after the `rate-limit` change. See [the runbook](runbook) for rollback steps.\n\n## What changed\n\n- Token bucket limits per API key\n- Retry budget for upstream calls\n  - capped at 10% of traffic\n1. Deploy to canary\n2. Watch p99 for ten minutes\n3. Roll out to the rest\n\n> Latency stayed flat; error rate dropped from 0.8% to 0.2%.\n\n```\nrate_limit:\n  burst: 200\n  refill_per_sec: 50\n```\n\nNothing else is waiting on this release.";

state! {
    struct Bench {
        docs: Vec<String> = ["Notes", "Build log", "Config", "Deploy plan", "Metrics", "Alerts", "Runbook", "Changelog", "Owners"].map(String::from).to_vec(),
        tab: usize,
        build: usize = 11,
        link: String,
        only_failed: bool,
    }
}

fn row_of(i: usize) -> impl Into<Node<Act<Bench>>> {
    let n = BUILDS - i;
    row![
        caption(format!("#{n}")).dim().w(64.0),
        text(SERVICES[n % SERVICES.len()]).fill_w(),
        badge(STATUS[n % STATUS.len()]).auto(),
    ]
    .gap(10.0)
    .align_center()
}

fn view(s: &Bench) -> impl View<Act<Bench>> {
    let page = col![
        tab_bar(&s.docs, at!(s.tab)).close(|s: &mut Bench, i| {
            if s.docs.len() > 1 {
                s.docs.remove(i);
                s.tab = s.tab.min(s.docs.len() - 1);
            }
        }),
        context_menu(
            scroll(
                markdown(NOTES)
                    .width(520.0)
                    .link(|s: &mut Bench, url| s.link = url)
            )
            .fill_h(),
            [
                menu_item("Copy link")
                    .key("Ctrl+L")
                    .act(|s: &mut Bench| s.link = "copied".into()),
                menu_item("Open in new tab").act(|s: &mut Bench| {
                    s.docs.push("Notes".into());
                    s.tab = s.docs.len() - 1;
                }),
                menu_rule(),
                menu_item("Close tab").danger().act(|s: &mut Bench| {
                    if s.docs.len() > 1 {
                        s.docs.remove(s.tab);
                        s.tab = s.tab.min(s.docs.len() - 1);
                    }
                }),
            ]
        ),
    ]
    .gap(14.0)
    .pad(16.0);
    col![
        row![
            heading("Builds").fill_w(),
            popover(
                button("Filters"),
                col![
                    caption("Show").dim(),
                    check("Only failed", at!(s.only_failed)),
                ]
                .gap(8.0)
                .w(180.0),
            ),
        ]
        .align_center()
        .pad_xy(16.0, 12.0),
        split(
            col![
                caption(format!("{BUILDS} builds, newest first")).dim(),
                vlist(BUILDS, row_of)
                    .ruled()
                    .selected(Some(s.build))
                    .row_height(36.0)
                    .act(|s: &mut Bench, i| s.build = i),
            ]
            .gap(8.0)
            .pad(16.0),
            page,
        )
        .ratio(0.38),
    ]
}

fn main() {
    sketch(Bench::default(), view).run_sized("Alloy workbench", 980.0, 640.0)
}
