//! Data: a 100 000 row grid with sorting, resizing and inline edits, and a customer list that
//! drills down through the router.
use crate::data::*;
use crate::ui::*;
use crate::{Route, Studio};
use alloy_ui::prelude::*;
use alloy_ui::{i18n, valid, Router};
use std::sync::Arc;

pub fn view(s: &Studio, nav: Router<Route>) -> N {
    let body: N = match s.data_tab {
        0 => orders(s),
        _ => customers_list(s, nav),
    };
    fixed(col![
        header(
            i18n::translate("data.title", &[]),
            i18n::translate("data.sub", &[]),
            row![
                button("Export CSV").act(|s: &mut Studio| super::system::export_csv(s)),
                button("New order").primary().act(|s: &mut Studio| {
                    let id = 10_000 + s.orders.len() as u32;
                    Arc::make_mut(&mut s.orders).insert(
                        0,
                        Order {
                            id,
                            customer: "New customer".into(),
                            region: "AMER",
                            total: 120.0,
                            status: "Pending",
                            packed: 0.0,
                        },
                    );
                    toast("Order created at the top of the grid");
                }),
            ]
            .gap(8.0)
        ),
        tab_bar(
            [
                format!("Orders ({})", s.orders.len()),
                format!("Customers ({})", s.customers.len())
            ],
            at!(s.data_tab)
        ),
        body.fill_h(),
    ])
}

fn orders(s: &Studio) -> N {
    let total: f64 = s.orders.iter().take(100_000).map(|o| o.total).sum();
    col![
        row![
            caption(format!(
                "{} orders · ${:.0}k booked · {} selected",
                s.orders.len(),
                total / 1000.0,
                s.picked_rows
            ))
            .dim()
            .fill_w(),
            caption("Click a header to sort, drag its edge to resize, double-click a customer to rename it.")
                .dim(),
        ]
        .align_center(),
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
                |s: &mut Studio, row, _, text| Arc::make_mut(&mut s.orders)[row].customer = text
            )
            .on_select(|s: &mut Studio, rows| s.picked_rows = rows.len()),
    ]
    .gap(8.0)
    .into()
}

fn customers_list(s: &Studio, nav: Router<Route>) -> N {
    col![
        row![
            input(at!(s.customer_filter))
                .hint("Filter by name, company or tier")
                .w(320.0),
            caption("Click a row to open the customer. Alt+Left comes back.").dim(),
        ]
        .gap(12.0)
        .align_center(),
        scroll(
            table(&s.customers)
                .col("Customer", |c: &Customer| c.name.clone())
                .col("Company", |c: &Customer| c.company.clone())
                .col("Plan", |c: &Customer| format!("{:?}", c.tier))
                .num("Seats", |c: &Customer| c.seats.to_string())
                .num("MRR", |c: &Customer| format!("${:.0}", c.mrr))
                .filter(&s.customer_filter)
                .count("customers")
                .on_row(move |_: &mut Studio, c: &Customer| nav.push(Route::Customer(c.id)))
        )
        .fill_h(),
    ]
    .gap(10.0)
    .into()
}

pub fn customer(s: &Studio, id: usize, nav: Router<Route>) -> N {
    let Some(c) = s.customers.get(id) else {
        return fixed(text("That customer no longer exists."));
    };
    let arr = c.mrr * 12.0;
    let trend = walk(
        24,
        200 + id as u32,
        c.mrr.max(50.0),
        c.mrr.max(50.0) * 0.12,
        c.mrr.max(50.0) * 0.01,
    );
    let email = c.email.clone();
    page(col![
        header(
            c.name.clone(),
            format!("{} · customer since 2024", c.company),
            row![
                button("Copy email").act(move |s: &mut Studio| {
                    alloy_ui::clipboard::copy(&email);
                    s.log("Copied an email address");
                    toast("Email copied");
                }),
                button("Back to customers")
                    .primary()
                    .act(move |_| nav.back()),
            ]
            .gap(8.0)
        ),
        row![
            col![
                field(
                    "Name",
                    input(at!(s.customers[id].name)),
                    valid::required(&c.name)
                ),
                field(
                    "Email",
                    input(at!(s.customers[id].email)),
                    valid::email(&c.email)
                ),
                field(
                    "Company",
                    input(at!(s.customers[id].company)),
                    valid::required(&c.company)
                ),
                field(
                    "Plan",
                    options(c.tier).bind(move |s: &mut Studio| &mut s.customers[id].tier),
                    None::<&str>
                ),
                field(
                    "Seats",
                    stepper(at!(s.customers[id].seats)).min(1).max(5000),
                    None::<&str>
                ),
                field(
                    "Notes",
                    editor(at!(s.customers[id].notes)).h(110.0),
                    None::<&str>
                ),
                row![
                    button("Save changes").primary().act(|_| {
                        toast("Customer saved");
                    }),
                    button("Discard").ghost().act(move |_| nav.back()),
                ]
                .gap(8.0),
            ]
            .gap(12.0)
            .card()
            .pad(20.0)
            .w(520.0),
            col![
                stats([
                    ("MRR", format!("${:.0}", c.mrr)),
                    ("ARR", format!("${:.1}k", arr / 1000.0)),
                    ("Seats", c.seats.to_string()),
                ]),
                panel(
                    "Monthly revenue, last 24 months",
                    line_chart([("MRR", trend)])
                        .area()
                        .no_legend()
                        .height(200.0)
                ),
                panel(
                    "Account notes",
                    markdown(format!("**{}**\n\n{}", c.company, c.notes)).width(380.0)
                ),
            ]
            .gap(14.0)
            .fill_w(),
        ]
        .gap(18.0),
    ])
}
