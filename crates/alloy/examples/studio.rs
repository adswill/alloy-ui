//! Studio: a 100 000-row data grid, a code editor and a rich-text editor in one small app.
use alloy_ui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct Order {
    id: u32,
    customer: String,
    region: &'static str,
    total: f64,
    status: &'static str,
    packed: f32,
}

fn orders(n: u32) -> Vec<Order> {
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

const NOTES: &str = "# Q4 planning\n\nThe **orders grid** now handles a hundred thousand rows without a hitch. Sort any column, drag a header edge to resize it, double-click a *Customer* to rename it.\n\n## Before launch\n\n- Freeze the public API\n- Write the changelog\n- Tag **v0.4** and publish the [release notes](https://alloy.dev/notes)\n\n1. Smoke test on three platforms\n2. Check idle CPU stays near zero\n\n> Small, sharp tools win. Ship the editor, then ship the docs.";

state! {
    struct Studio {
        tab: usize,
        orders: Arc<Vec<Order>> = Arc::new(orders(100_000)),
        src: String = include_str!("../src/editors.rs").to_string(),
        doc: RichText = RichText::from_markdown(NOTES),
        picked: usize,
        opened: String,
    }
}

fn view(s: &Studio) -> impl View<Act<Studio>> {
    let page: Node<Act<Studio>> = match s.tab {
        0 => col![
            caption(format!("{} orders · {} selected", s.orders.len(), s.picked)).dim(),
            data_grid(&s.orders)
                .col("Order", |o: &Order| format!("#{}", o.id))
                .width(96.0)
                .col("Customer", |o: &Order| o.customer.clone())
                .editable()
                .flex()
                .col("Region", |o: &Order| o.region)
                .col("Packed", |o: &Order| GridCell::bar(o.packed))
                .col("Status", |o: &Order| GridCell::status(
                    tone_of(o.status),
                    o.status
                ))
                .col("Total", |o: &Order| format!("${:.2}", o.total))
                .right()
                .edit(
                    |s: &mut Studio, row, _, text| Arc::make_mut(&mut s.orders)[row].customer =
                        text
                )
                .on_select(|s: &mut Studio, rows| s.picked = rows.len()),
        ]
        .gap(8.0)
        .into(),
        1 => code_editor(at!(s.src)).file("editors.rs").into(),
        _ => rich_editor(at!(s.doc))
            .placeholder("Start writing")
            .link(|s: &mut Studio, url| s.opened = url)
            .into(),
    };
    col![
        row![heading("Studio").fill_w(), caption(&s.opened).dim()].align_center(),
        tab_bar(["Orders", "Source", "Notes"], at!(s.tab)),
        page.fill_h(),
    ]
    .gap(12.0)
    .pad(16.0)
}

fn tone_of(s: &str) -> Tone {
    match s {
        "Shipped" => Tone::Success,
        "Delayed" => Tone::Danger,
        _ => Tone::Warning,
    }
}

fn main() {
    sketch(Studio::default(), view).run_sized("Alloy Studio", 1040.0, 700.0)
}
