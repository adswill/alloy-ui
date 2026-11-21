//! Workbench: tree, split panes, a 100 000 row virtual list, tabs with overflow, context menus,
//! popovers, and drag and drop between two lists.
use crate::ui::*;
use crate::Studio;
use alloy_ui::i18n;
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

const LOG: &str = "# Build 99,988\n\nShipped **api-gateway** to production after the `rate-limit` change. See [the runbook](runbook) for rollback steps.\n\n## What changed\n\n- Token bucket limits per API key\n- Retry budget for upstream calls\n  - capped at 10% of traffic\n1. Deploy to canary\n2. Watch p99 for ten minutes\n3. Roll out to the rest\n\n> Latency stayed flat; error rate dropped from 0.8% to 0.2%.\n\n```\nrate_limit:\n  burst: 200\n  refill_per_sec: 50\n```\n";

fn files() -> Vec<TreeItem> {
    vec![
        TreeItem::new(1, "crates").children([
            TreeItem::new(2, "core")
                .children([TreeItem::new(3, "draw.rs"), TreeItem::new(4, "widget.rs")]),
            TreeItem::new(5, "widgets").children([
                TreeItem::new(6, "kanban.rs"),
                TreeItem::new(7, "calendar.rs"),
                TreeItem::new(8, "node_graph.rs"),
            ]),
            TreeItem::new(9, "theme"),
            TreeItem::new(10, "render"),
        ]),
        TreeItem::new(11, "docs")
            .children([TreeItem::new(12, "DESIGN.md"), TreeItem::new(13, "guide")]),
        TreeItem::new(14, "Cargo.toml"),
    ]
}

fn name_of(id: u64) -> &'static str {
    match id {
        3 => "draw.rs",
        4 => "widget.rs",
        6 => "kanban.rs",
        7 => "calendar.rs",
        8 => "node_graph.rs",
        12 => "DESIGN.md",
        14 => "Cargo.toml",
        _ => "folder",
    }
}

fn row_of(i: usize) -> impl Into<Node<Act<Studio>>> {
    let n = BUILDS - i;
    row![
        caption(format!("#{n}")).dim().w(64.0),
        text(SERVICES[n % SERVICES.len()]).fill_w(),
        badge(STATUS[n % STATUS.len()]).auto(),
    ]
    .gap(10.0)
    .align_center()
}

fn close_tab(s: &mut Studio, i: usize) {
    if s.docs.len() > 1 {
        s.docs.remove(i);
        s.wb_tab = s.wb_tab.min(s.docs.len() - 1);
    }
}

fn explorer(s: &Studio) -> N {
    split(
        col![
            caption("Project").dim(),
            tree(files())
                .open([1, 5, 11])
                .selected(s.file)
                .act(|s: &mut Studio, id| s.file = Some(id)),
            caption(match s.file {
                Some(id) => format!("Selected: {}", name_of(id)),
                None => "Nothing selected".into(),
            })
            .dim(),
        ]
        .gap(8.0)
        .pad(14.0),
        col![
            tab_bar(&s.docs, at!(s.wb_tab)).close(close_tab),
            context_menu(
                scroll(markdown(LOG).width(520.0)).fill_h(),
                [
                    menu_item("Copy link").key("Ctrl+L").act(|s: &mut Studio| {
                        alloy_ui::clipboard::copy("alloy://builds/99988");
                        s.log("Copied a build link");
                        toast("Link copied");
                    }),
                    menu_item("Open in new tab").act(|s: &mut Studio| {
                        s.docs.push("Notes".into());
                        s.wb_tab = s.docs.len() - 1;
                    }),
                    menu_rule(),
                    menu_item("Close tab").danger().act(|s: &mut Studio| {
                        let i = s.wb_tab;
                        close_tab(s, i)
                    }),
                ]
            ),
            caption("Right-click the page for a context menu. Middle-click a tab to close it; extra tabs fold into +N.").dim(),
        ]
        .gap(10.0)
        .pad(14.0),
    )
    .ratio(0.3)
    .into()
}

fn builds(s: &Studio) -> N {
    split(
        col![
            row![
                caption(format!("{BUILDS} builds, newest first"))
                    .dim()
                    .fill_w(),
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
            .align_center(),
            vlist(BUILDS, row_of)
                .ruled()
                .selected(Some(s.build))
                .row_height(36.0)
                .act(|s: &mut Studio, i| s.build = i),
        ]
        .gap(8.0)
        .pad(14.0),
        scroll(
            col![
                heading(format!("Build {}", BUILDS - s.build)),
                badge(STATUS[(BUILDS - s.build) % STATUS.len()])
                    .dot()
                    .auto(),
                markdown(LOG).width(480.0),
            ]
            .gap(10.0)
            .pad(16.0),
        )
        .fill_h(),
    )
    .ratio(0.42)
    .into()
}

fn drag(s: &Studio) -> N {
    let chip = |t: &String| {
        row![icon(Icon::File).dim(), text(t.clone())]
            .gap(8.0)
            .align_center()
            .pad(10.0)
            .card()
            .drag(t.clone())
    };
    let list = |title: &str, items: &Vec<String>| {
        col![
            caption(title.to_string()).dim(),
            col(items.iter().map(chip).collect::<Vec<_>>()).gap(8.0),
            vspace(),
        ]
        .gap(10.0)
        .pad(16.0)
        .card()
        .w(300.0)
        .h(320.0)
    };
    col![
        row![
            list("Inbox", &s.inbox).drop_act(|s: &mut Studio, f: String| {
                s.archive.retain(|x| *x != f);
                s.inbox.retain(|x| *x != f);
                s.inbox.push(f);
            }),
            list("Archive", &s.archive).drop_act(|s: &mut Studio, f: String| {
                s.inbox.retain(|x| *x != f);
                s.archive.retain(|x| *x != f);
                s.archive.push(f);
                toast("Archived").undo();
            }),
        ]
        .gap(20.0),
        caption("Drag a file from one list to the other. A translucent copy follows the pointer, and a target that accepts it lights up.").dim(),
    ]
    .gap(14.0)
    .pad(4.0)
    .into()
}

pub fn view(s: &Studio) -> N {
    let body: N = match s.wb_page {
        0 => explorer(s),
        1 => builds(s),
        _ => drag(s),
    };
    fixed(col![
        header(
            i18n::translate("wb.title", &[]),
            i18n::translate("wb.sub", &[]),
            text("")
        ),
        tab_bar(
            ["Explorer", "Builds (100k)", "Drag and drop"],
            at!(s.wb_page)
        ),
        body.fill_h(),
    ])
}
