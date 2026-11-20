//! Status board: async fetch with loading and error states, auto-refresh, filter, sortable table, detail chart.
use alloy_ui::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};

choices! { enum Status { Up, Degraded, Down } }

#[derive(Record, Clone, Default)]
struct Service {
    name: String,
    region: String,
    status: Status,
    latency: u32,
    uptime: f32,
    #[form(skip)]
    history: Vec<f32>,
}

async fn fetch() -> Result<Vec<Service>, String> {
    static ATTEMPT: AtomicU32 = AtomicU32::new(0);
    sleep(secs(0.7)).await;
    let n = ATTEMPT.fetch_add(1, Relaxed);
    if n % 3 == 2 {
        return Err("503 from api.status.local: upstream timed out".into());
    }
    let rows = [
        ("api-gateway", "eu-west", Status::Up, 42, 99.99),
        ("auth", "eu-west", Status::Up, 31, 99.98),
        ("billing", "us-east", Status::Degraded, 380, 99.12),
        ("search", "us-east", Status::Up, 88, 99.90),
        ("media-cdn", "ap-south", Status::Down, 0, 97.40),
        ("notifications", "eu-west", Status::Up, 64, 99.95),
    ];
    Ok(rows
        .iter()
        .enumerate()
        .map(|(i, &(name, region, status, latency, uptime))| Service {
            name: name.into(),
            region: region.into(),
            status,
            latency: latency + (n * 7 + i as u32 * 3) % 20,
            uptime,
            history: (0..16)
                .map(|k| ((k * 37 + i as u32 * 11 + n * 5) % 100) as f32 / 100.0)
                .collect(),
        })
        .collect())
}

fn main() {
    app("Status")
        .page("Services", || {
            let (auto, filter, sel) = (local(false), local(None), local(None::<usize>));
            let svc = resource(fetch).every_if(auto.get(), secs(10.0));
            let top = row![
                heading("Services"),
                space(),
                toggle("Auto refresh", auto.get()).local(&auto),
                svc.refresh_button()
            ];
            let body = svc.show(|all| {
                let rows: Vec<Service> = all
                    .iter()
                    .filter(|s| filter.get().is_none() || filter.get() == Some(s.status))
                    .cloned()
                    .collect();
                let stat = |st: Status| {
                    col![
                        caption(format!("{st:?}")),
                        heading(all.iter().filter(|s| s.status == st).count().to_string())
                    ]
                    .card()
                    .pad(16.0)
                    .fill_w()
                };
                let detail = sel.get().and_then(|i| rows.get(i)).map(|s| {
                    col![
                        heading(&s.name),
                        caption(format!("{} · {:?}", s.region, s.status)),
                        bars(s.history.clone()),
                        caption(format!("{} ms now", s.latency))
                    ]
                    .gap(10.0)
                    .card()
                    .pad(20.0)
                    .w(300.0)
                });
                let opts = [
                    ("All", None),
                    ("Up", Some(Status::Up)),
                    ("Degraded", Some(Status::Degraded)),
                    ("Down", Some(Status::Down)),
                ];
                col![
                    row![stat(Status::Up), stat(Status::Degraded), stat(Status::Down)].gap(12.0),
                    choose(opts, &filter.get()).local(&filter),
                    row![
                        table_of(&rows)
                            .selected(sel.get())
                            .on_select(move |_, i| sel.set(Some(i))),
                        detail.map_or_else(|| gap(0.0, 0.0), Node::from)
                    ]
                    .gap(16.0)
                    .fill(),
                ]
                .gap(14.0)
                .fill()
                .into()
            });
            col![top.align_center().gap(12.0), body]
                .gap(16.0)
                .pad(24.0)
                .fill()
        })
        .run()
}
