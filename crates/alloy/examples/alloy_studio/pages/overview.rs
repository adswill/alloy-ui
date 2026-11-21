//! Dashboard: key figures, line, bar, pie, candlestick and live charts, and a checklist whose
//! deletes can be undone.
use crate::data::*;
use crate::ui::*;
use crate::{Studio, Task};
use alloy_ui::i18n;
use alloy_ui::prelude::*;

const RANGES: [(&str, usize); 3] = [("7 days", 7), ("30 days", 30), ("90 days", 90)];

pub fn view(s: &Studio) -> N {
    let days = RANGES[s.range].1;
    let revenue = walk(days, 11, 42.0, 6.0, 0.35);
    let cost = walk(days, 29, 24.0, 4.0, 0.12);
    let visitors = walk(days, 53, 800.0, 120.0, 6.0);
    let conv = walk(days, 71, 3.2, 0.5, 0.01);
    let latency = walk(days, 91, 120.0, 14.0, -0.4);
    let last = |v: &[f32]| v.last().copied().unwrap_or(0.0);
    let change = |v: &[f32]| (v[v.len() - 1] / v[0] - 1.0) * 100.0;
    let labels = day_labels(days);
    let bars: Vec<f32> = (0..7)
        .map(|i| (noise(40 + i) * 7.0 + 1.0).round())
        .collect();

    let open_tasks = s.work.tasks.iter().filter(|t| !t.done).count();
    let tasks = col![
        row![
            caption(i18n::translate("ov.tasks", &[])).dim().fill_w(),
            badge(format!("{open_tasks} open")).accent(),
        ]
        .align_center(),
        col(s.work.tasks.iter().map(|t| {
            let id = t.id;
            row![
                check(&t.title, t.done).act(move |s: &mut Studio, v| {
                    if let Some(t) = s.work.tasks.iter_mut().find(|t| t.id == id) {
                        t.done = v;
                    }
                }),
                space(),
                button("Delete").ghost().act(move |s: &mut Studio| {
                    s.work.tasks.retain(|t| t.id != id);
                    toast("Task deleted").undo();
                }),
            ]
            .align_center()
        }))
        .gap(2.0),
        button("Add a task").act(|s: &mut Studio| {
            let id = s.work.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            s.work.tasks.push(Task {
                id,
                title: format!("Follow up on item {id}"),
                done: false,
            });
        }),
    ]
    .gap(10.0)
    .card()
    .pad(16.0)
    .fill_w();

    let acts = row![
        choose(
            RANGES.iter().enumerate().map(|(i, (l, _))| (*l, i)),
            at!(s.range)
        ),
        popover(
            button("Export"),
            col![
                caption("Export this view").dim(),
                button("Copy as HTML table").fill_w().act(|s: &mut Studio| {
                    alloy_ui::clipboard::copy_html(
                        "<table><tr><th>Day</th><th>Revenue</th></tr><tr><td>Today</td><td>$48.2k</td></tr></table>",
                        "Day\tRevenue\nToday\t$48.2k",
                    );
                    s.log("Copied the table as HTML");
                    toast("Copied as HTML and plain text");
                }),
                button("Notify me when done").fill_w().act(|s: &mut Studio| {
                    notify("Export finished", "revenue.csv is ready");
                    s.log("Sent a notification");
                }),
            ]
            .gap(6.0)
            .w(220.0),
        ),
    ]
    .gap(8.0)
    .align_center();

    page(col![
        header(
            i18n::translate("ov.title", &[]),
            i18n::translate("ov.sub", &[]),
            acts
        ),
        row![
            kpi_card(
                "Revenue",
                &format!("${:.1}k", last(&revenue)),
                change(&revenue),
                true,
                revenue.clone()
            ),
            kpi_card(
                "Visitors",
                &format!("{:.0}", last(&visitors)),
                change(&visitors),
                true,
                visitors.clone()
            ),
            kpi_card(
                "Conversion",
                &format!("{:.1}%", last(&conv)),
                change(&conv),
                true,
                conv.clone()
            ),
            kpi_card(
                "p95 latency",
                &format!("{:.0} ms", last(&latency)),
                change(&latency),
                false,
                latency.clone()
            ),
        ]
        .gap(14.0),
        row![
            panel(
                "Revenue and cost, $k",
                line_chart([("Revenue", revenue), ("Cost", cost)])
                    .labels(labels)
                    .area()
                    .height(250.0)
            ),
            panel(
                "Revenue by channel",
                pie([
                    ("Direct", 1200.0),
                    ("Search", 840.0),
                    ("Referral", 410.0),
                    ("Social", 230.0),
                    ("Email", 190.0)
                ])
                .donut()
                .title("This month")
                .unit(" $k")
                .selected(s.slice)
                .act(|s: &mut Studio, i| s.slice = Some(i))
                .height(250.0)
            ),
        ]
        .gap(14.0),
        row![
            panel(
                "ACME, last 32 sessions",
                candles(s.market.clone()).unit("$").height(240.0)
            ),
            panel(
                "Live load",
                stream([("CPU", &s.cpu), ("Memory", &s.mem)])
                    .unit("%")
                    .window("last 60 s")
                    .height(240.0)
            ),
        ]
        .gap(14.0),
        row![
            panel(
                "Deploys this week",
                bar_chart([("Deploys", bars)])
                    .labels(["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"])
                    .height(190.0)
            ),
            tasks,
        ]
        .gap(14.0),
    ])
}
