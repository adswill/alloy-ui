//! Status board: async fetch with loading and error states, auto-refresh, filter, sortable table, detail chart.
use alloy_ui::prelude::*;

choices! { enum Status { Up, Degraded, Down } }

#[derive(Record, Clone, Default)]
struct Service {
    name: String,
    region: String,
    status: Status,
    latency: u32,
    uptime: f32,
}

fn main() {
    app("Status")
        .page("Services", || {
            Service::mock([
                vals!["api-gateway", "eu-west", Status::Up, 42, 99.99],
                vals!["auth", "eu-west", Status::Up, 31, 99.98],
                vals!["billing", "us-east", Status::Degraded, 380, 99.12],
                vals!["search", "us-east", Status::Up, 88, 99.90],
                vals!["media-cdn", "ap-south", Status::Down, 0, 97.40],
                vals!["notifications", "eu-west", Status::Up, 64, 99.95],
            ])
            .delay(0.7)
            .fail_every(3, "503 from api.status.local: upstream timed out")
            .resource()
            .auto(secs(10.0))
            .board("Services", |s| s.status)
            .trend(|s| series(&s.name, 16))
        })
        .run()
}
