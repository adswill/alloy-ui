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
            let filter = local(None);
            resource(fetch).auto(secs(10.0)).page("Services", |all| {
                col![
                    tally(all, |s| s.status),
                    filter_by(&filter),
                    table_of(&filter.apply(all, |s| s.status)).detail(|s| col![
                        heading(&s.name),
                        caption(format!("{} · {:?} · {} ms", s.region, s.status, s.latency)),
                        bars(s.history.clone())
                    ]
                    .gap(10.0))
                ]
                .gap(14.0)
                .fill()
            })
        })
        .run()
}
