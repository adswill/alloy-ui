//! Deterministic sample data. Nothing here touches the network or the disk.
use alloy_ui::prelude::*;
use alloy_ui::widgets::Date;
use alloy_ui::{card, entry, lane, node, Candle, Entry, GraphNode, Lane, Ring, Wire};

/// The day the sample data is anchored on, so screenshots and tests never drift.
pub const TODAY: Date = Date {
    year: 2026,
    month: 11,
    day: 21,
};

/// Cheap deterministic noise in 0..1.
pub fn noise(i: u32) -> f32 {
    let x = i
        .wrapping_mul(2_654_435_761)
        .rotate_left(13)
        .wrapping_mul(0x85EB_CA6B);
    (x >> 8) as f32 / (1u32 << 24) as f32
}

pub fn ring(cap: usize, seed: u32, base: f32, amp: f32) -> Ring {
    let mut r = Ring::new(cap);
    for i in 0..cap as u32 {
        let t = i as f32 / 9.0;
        r.push(base + amp * (t.sin() * 0.6 + (noise(seed + i) - 0.5)));
    }
    r
}

/// A smooth-ish random walk of `n` points that trends by `drift` per step.
pub fn walk(n: usize, seed: u32, start: f32, step: f32, drift: f32) -> Vec<f32> {
    let mut v = start;
    (0..n as u32)
        .map(|i| {
            v = (v + (noise(seed + i * 7) - 0.5) * step + drift).max(start * 0.2);
            v
        })
        .collect()
}

pub fn day_labels(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let d = Date::from_days(TODAY.to_days() - (n - 1 - i) as i64);
            format!("{:02}/{:02}", d.month, d.day)
        })
        .collect()
}

pub fn market() -> Vec<Candle> {
    let mut price = 118.0;
    (0..32u32)
        .map(|i| {
            let open = price;
            let close = (open + (noise(i * 7 + 3) - 0.46) * 9.0).max(60.0);
            let high = open.max(close) + noise(i * 3 + 1) * 4.0;
            let low = open.min(close) - noise(i * 5 + 2) * 4.0;
            price = close;
            Candle::new(format!("{:02}", i % 28 + 1), open, high, low, close)
                .volume(800.0 + noise(i + 90) * 2600.0)
        })
        .collect()
}

// ------------------------------------------------------------------ orders

#[derive(Clone)]
pub struct Order {
    pub id: u32,
    pub customer: String,
    pub region: &'static str,
    pub total: f64,
    pub status: &'static str,
    pub packed: f32,
}

pub fn orders(n: u32) -> Vec<Order> {
    let names = [
        "Northwind",
        "Contoso",
        "Fabrikam",
        "Initech",
        "Hooli",
        "Umbrella",
        "Soylent",
        "Stark",
    ];
    let regions = ["EMEA", "APAC", "AMER"];
    let status = ["Shipped", "Pending", "Delayed", "Shipped", "Shipped"];
    (0..n)
        .map(|i| {
            let h = i.wrapping_mul(2_654_435_761) >> 8;
            Order {
                id: 10_000 + i,
                customer: format!("{} {}", names[h as usize % 8], 1 + h % 97),
                region: regions[(h >> 3) as usize % 3],
                total: ((h % 90_000) as f64) / 100.0 + 20.0,
                status: status[(h >> 5) as usize % 5],
                packed: if i % 5 == 0 {
                    0.4
                } else {
                    ((h >> 7) % 100) as f32 / 100.0
                },
            }
        })
        .collect()
}

pub fn tone_of(s: &str) -> Tone {
    match s {
        "Shipped" => Tone::Success,
        "Delayed" => Tone::Danger,
        _ => Tone::Warning,
    }
}

// --------------------------------------------------------------- customers

choices! { pub enum Tier { Free, Pro, Team, Enterprise } }

#[derive(Clone, Default)]
pub struct Customer {
    pub id: usize,
    pub name: String,
    pub email: String,
    pub company: String,
    pub tier: Tier,
    pub seats: u32,
    pub mrr: f32,
    pub notes: String,
}

pub fn customers() -> Vec<Customer> {
    let rows: [(&str, &str, Tier, u32, f32, &str); 14] = [
        (
            "Ada Lovelace",
            "Analytical Engines",
            Tier::Enterprise,
            240,
            9600.0,
            "Renews in March. Asked about SSO and audit logs.",
        ),
        (
            "Grace Hopper",
            "Navy Software",
            Tier::Team,
            42,
            1680.0,
            "Wants a COBOL importer, half joking.",
        ),
        (
            "Alan Turing",
            "Bletchley Labs",
            Tier::Pro,
            8,
            96.0,
            "Trial converted after the benchmarks post.",
        ),
        (
            "Katherine Johnson",
            "Orbital Math",
            Tier::Enterprise,
            120,
            4800.0,
            "Reference customer. Happy to take calls.",
        ),
        (
            "Margaret Hamilton",
            "Apollo Systems",
            Tier::Team,
            31,
            1240.0,
            "Reports bugs with a fix attached.",
        ),
        (
            "Linus Torvalds",
            "Kernel Works",
            Tier::Pro,
            5,
            60.0,
            "Says the idle CPU number is impressive.",
        ),
        (
            "Barbara Liskov",
            "Substitution Inc",
            Tier::Team,
            56,
            2240.0,
            "Asked for stricter validation in forms.",
        ),
        (
            "Dennis Ritchie",
            "Bell Tools",
            Tier::Pro,
            12,
            144.0,
            "Prefers the keyboard to the mouse.",
        ),
        (
            "Radia Perlman",
            "Spanning Tree Co",
            Tier::Enterprise,
            310,
            12400.0,
            "Multi-region rollout planned for Q1.",
        ),
        (
            "Ken Thompson",
            "Plan Nine",
            Tier::Free,
            1,
            0.0,
            "Evaluating on a very old laptop.",
        ),
        (
            "Hedy Lamarr",
            "Spread Spectrum",
            Tier::Pro,
            9,
            108.0,
            "Loves the node graph.",
        ),
        (
            "Donald Knuth",
            "Literate Press",
            Tier::Team,
            18,
            720.0,
            "Sent a check for the typography.",
        ),
        (
            "Frances Allen",
            "Optimizers Ltd",
            Tier::Pro,
            14,
            168.0,
            "Profiles everything, twice.",
        ),
        (
            "Tim Berners-Lee",
            "Open Web Group",
            Tier::Team,
            64,
            2560.0,
            "Asked why there is no browser. We said: on purpose.",
        ),
    ];
    rows.iter()
        .enumerate()
        .map(|(i, (name, company, tier, seats, mrr, notes))| Customer {
            id: i,
            name: name.to_string(),
            email: format!(
                "{}@{}.example",
                name.split(' ').next().unwrap_or("hi").to_lowercase(),
                company.split(' ').next().unwrap_or("co").to_lowercase()
            ),
            company: company.to_string(),
            tier: *tier,
            seats: *seats,
            mrr: *mrr,
            notes: notes.to_string(),
        })
        .collect()
}

// -------------------------------------------------------------- planner

pub fn lanes() -> Vec<Lane> {
    vec![
        lane(
            "Backlog",
            [
                card(1, "Plan the Q1 roadmap").detail("Product"),
                card(2, "Audit icon set")
                    .detail("Design")
                    .tag("Needs owner", Tone::Warning),
                card(3, "Retire legacy export"),
            ],
        ),
        lane(
            "In progress",
            [
                card(4, "Rebuild onboarding")
                    .detail("Maya · 3 days")
                    .tag("On track", Tone::Success),
                card(5, "Fix sync conflicts")
                    .detail("Jon · 1 day")
                    .tag("Blocked", Tone::Danger),
            ],
        ),
        lane(
            "Review",
            [card(6, "Dark theme contrast pass")
                .detail("Priya")
                .tag("Ready", Tone::Accent)],
        ),
        lane(
            "Done",
            [
                card(7, "Ship offline mode"),
                card(8, "Migrate billing webhooks").detail("Jon"),
            ],
        ),
    ]
}

pub fn entries() -> Vec<Entry> {
    let d = |n: i64| Date::from_days(TODAY.to_days() + n);
    let m = |h: u32, min: u32| h * 60 + min;
    vec![
        entry(1, "Standup", d(0), m(9, 30), m(10, 0)).tone(Tone::Neutral),
        entry(2, "Design review", d(0), m(11, 0), m(12, 30)).tone(Tone::Accent),
        entry(3, "Lunch with Priya", d(0), m(12, 30), m(13, 30)).tone(Tone::Success),
        entry(4, "Roadmap sync", d(0), m(12, 45), m(14, 0)).tone(Tone::Warning),
        entry(5, "Interview: staff eng", d(1), m(10, 0), m(11, 0)).tone(Tone::Accent),
        entry(6, "Release train", d(2), m(15, 0), m(16, 30)).tone(Tone::Danger),
        entry(7, "1:1 with Maya", d(-1), m(14, 0), m(14, 30)).tone(Tone::Neutral),
        entry(8, "Budget workshop", d(-2), m(9, 0), m(11, 30)).tone(Tone::Warning),
        entry(9, "Team offsite", d(8), m(9, 0), m(17, 0)).tone(Tone::Success),
        entry(10, "Demo day", d(5), m(16, 0), m(17, 0)).tone(Tone::Accent),
        entry(11, "Retro", d(-6), m(15, 0), m(16, 0)).tone(Tone::Neutral),
        entry(12, "Dentist", d(3), m(8, 0), m(9, 0)).tone(Tone::Neutral),
        entry(13, "Security training", d(3), m(13, 0), m(14, 0)).tone(Tone::Warning),
        entry(14, "Pair on parser", d(3), m(14, 0), m(15, 0)).tone(Tone::Accent),
        entry(15, "Ship notes", d(3), m(16, 0), m(16, 30)).tone(Tone::Success),
    ]
}

pub fn graph() -> (Vec<GraphNode>, Vec<Wire>) {
    let nodes = vec![
        node(1, "Load image", 20.0, 40.0)
            .outputs(["Image", "Alpha"])
            .tone(Tone::Accent),
        node(2, "Blur", 270.0, 20.0)
            .inputs(["Image", "Radius"])
            .outputs(["Result"])
            .tone(Tone::Neutral),
        node(3, "Threshold", 270.0, 150.0)
            .inputs(["Image", "Level"])
            .outputs(["Mask"])
            .tone(Tone::Neutral),
        node(4, "Composite", 520.0, 80.0)
            .inputs(["Base", "Overlay", "Mask"])
            .outputs(["Image"])
            .tone(Tone::Warning),
        node(5, "Save PNG", 770.0, 100.0)
            .inputs(["Image"])
            .tone(Tone::Success)
            .width(150.0),
    ];
    let wires = vec![
        Wire::new((1, 0), (2, 0)),
        Wire::new((1, 0), (3, 0)),
        Wire::new((2, 0), (4, 0)),
        Wire::new((1, 0), (4, 1)),
        Wire::new((3, 0), (4, 2)),
        Wire::new((4, 0), (5, 0)),
    ];
    (nodes, wires)
}
