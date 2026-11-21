use alloy_ui::prelude::*;

choices! { enum Range { Week = "7 days", Month = "30 days", Quarter = "90 days" } }

#[derive(Default)]
struct Pulse {
    range: Range,
}

const ORDERS: [(&str, &str, &str, &str); 6] = [
    ("Northwind Traders", "Team", "$480.00", "Paid"),
    ("Acme Robotics", "Business", "$1,240.00", "Paid"),
    ("Lumen Studio", "Solo", "$29.00", "Pending"),
    ("Orbit Labs", "Business", "$1,240.00", "Refunded"),
    ("Fjord & Co", "Team", "$480.00", "Paid"),
    ("Kite Health", "Solo", "$29.00", "Pending"),
];

fn view(s: &Pulse) -> impl View<Act<Pulse>> {
    set_accent(Color::hex(0x4F6BED));
    let (tiles, bars_data) = match s.range {
        Range::Week => (
            [("Revenue", "$18,240", "+4.2%"), ("Active users", "2,418", "+1.8%"), ("Conversion", "3.9%", "+0.3 pt"), ("p95 latency", "212 ms", "-8 ms")],
            vec![0.42, 0.55, 0.48, 0.71, 0.66, 0.83, 0.77],
        ),
        Range::Month => (
            [("Revenue", "$76,905", "+9.6%"), ("Active users", "9,872", "+6.1%"), ("Conversion", "4.1%", "+0.5 pt"), ("p95 latency", "228 ms", "-4 ms")],
            vec![0.35, 0.4, 0.52, 0.47, 0.6, 0.66, 0.58, 0.74, 0.7, 0.88],
        ),
        Range::Quarter => (
            [("Revenue", "$231,410", "+14.0%"), ("Active users", "21,306", "+11.4%"), ("Conversion", "4.4%", "+0.9 pt"), ("p95 latency", "205 ms", "-21 ms")],
            vec![0.3, 0.33, 0.41, 0.45, 0.5, 0.48, 0.57, 0.63, 0.69, 0.72, 0.8, 0.92],
        ),
    };
    let tile = |(label, value, delta): (&str, &str, &str)| {
        col![caption(label).dim(), heading(value), caption(delta).success()]
            .gap(4.0)
            .card()
            .pad(16.0)
            .fill_w()
    };
    let cell = |s: &str, w: f32| text(s.to_string()).no_wrap().w(w);
    col![
        row![heading("Pulse"), space(), options(s.range).bind(|s: &mut Pulse| &mut s.range)].align_center(),
        row(tiles.map(tile)).gap(12.0),
        col![caption("Revenue per interval").dim(), bars(bars_data)].gap(10.0).card().pad(16.0).fill_w(),
        col![
            caption("Recent orders").dim(),
            col(ORDERS.map(|(who, plan, amount, status)| row![
                cell(who, 240.0),
                cell(plan, 140.0).dim(),
                cell(amount, 140.0),
                cell(status, 100.0).danger_if(status == "Refunded").dim_if(status == "Pending"),
            ]
            .gap(8.0))),
        ]
        .gap(10.0)
        .card()
        .pad(16.0)
        .fill_w(),
    ]
    .gap(14.0)
    .pad(24.0)
    .fill()
}

fn main() {
    sketch(Pulse::default(), view).run_sized("Pulse", 960.0, 640.0)
}
