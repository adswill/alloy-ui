//! Pulse: a small analytics dashboard with a range switcher.

#[path = "../ui.rs"]
mod ui;

use iced::widget::{column, container, row, space, text, toggler};
use iced::{Element, Length, Settings, Theme};

fn main() -> iced::Result {
    iced::application(Pulse::default, Pulse::update, Pulse::view)
        .title("Pulse")
        .theme(Pulse::theme)
        .settings(Settings {
            default_text_size: 14.into(),
            ..Settings::default()
        })
        .window_size((960.0, 640.0))
        .centered()
        .run()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Range {
    #[default]
    Week,
    Month,
    Quarter,
}

struct Tile {
    label: &'static str,
    value: &'static str,
    delta: &'static str,
    /// Whether the change is an improvement (drives the delta colour).
    good: bool,
}

struct Snapshot {
    caption: &'static str,
    tiles: [Tile; 4],
    /// Revenue per interval, in thousands, with its axis label.
    bars: &'static [(&'static str, f32)],
}

impl Range {
    const ALL: [(&'static str, Range); 3] = [
        ("7 days", Range::Week),
        ("30 days", Range::Month),
        ("90 days", Range::Quarter),
    ];

    fn snapshot(self) -> Snapshot {
        match self {
            Range::Week => Snapshot {
                caption: "Per day",
                tiles: [
                    Tile {
                        label: "Revenue",
                        value: "$48,210",
                        delta: "+8.2%",
                        good: true,
                    },
                    Tile {
                        label: "Active users",
                        value: "12,480",
                        delta: "+3.1%",
                        good: true,
                    },
                    Tile {
                        label: "Conversion",
                        value: "3.42%",
                        delta: "+0.4 pt",
                        good: true,
                    },
                    Tile {
                        label: "p95 latency",
                        value: "182 ms",
                        delta: "-6.0%",
                        good: true,
                    },
                ],
                bars: &[
                    ("Mon", 5.2),
                    ("Tue", 6.1),
                    ("Wed", 5.8),
                    ("Thu", 7.4),
                    ("Fri", 8.9),
                    ("Sat", 6.3),
                    ("Sun", 8.5),
                ],
            },
            Range::Month => Snapshot {
                caption: "Per 3 days",
                tiles: [
                    Tile {
                        label: "Revenue",
                        value: "$196,540",
                        delta: "+12.7%",
                        good: true,
                    },
                    Tile {
                        label: "Active users",
                        value: "41,902",
                        delta: "+9.4%",
                        good: true,
                    },
                    Tile {
                        label: "Conversion",
                        value: "3.18%",
                        delta: "-0.2 pt",
                        good: false,
                    },
                    Tile {
                        label: "p95 latency",
                        value: "191 ms",
                        delta: "+2.1%",
                        good: false,
                    },
                ],
                bars: &[
                    ("3", 16.2),
                    ("6", 17.8),
                    ("9", 15.9),
                    ("12", 19.4),
                    ("15", 21.0),
                    ("18", 18.7),
                    ("21", 20.6),
                    ("24", 22.3),
                    ("27", 19.1),
                    ("30", 25.5),
                ],
            },
            Range::Quarter => Snapshot {
                caption: "Per week",
                tiles: [
                    Tile {
                        label: "Revenue",
                        value: "$571,980",
                        delta: "+21.3%",
                        good: true,
                    },
                    Tile {
                        label: "Active users",
                        value: "96,340",
                        delta: "+15.8%",
                        good: true,
                    },
                    Tile {
                        label: "Conversion",
                        value: "3.51%",
                        delta: "+0.6 pt",
                        good: true,
                    },
                    Tile {
                        label: "p95 latency",
                        value: "176 ms",
                        delta: "-9.4%",
                        good: true,
                    },
                ],
                bars: &[
                    ("W1", 38.0),
                    ("W2", 41.0),
                    ("W3", 39.5),
                    ("W4", 44.0),
                    ("W5", 46.5),
                    ("W6", 45.0),
                    ("W7", 48.5),
                    ("W8", 50.0),
                    ("W9", 49.0),
                    ("W10", 52.5),
                    ("W11", 57.0),
                    ("W12", 61.0),
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Paid,
    Pending,
    Refunded,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Paid => "Paid",
            Status::Pending => "Pending",
            Status::Refunded => "Refunded",
        }
    }

    fn color(self) -> fn(&ui::Skin) -> iced::Color {
        match self {
            Status::Paid => |s| s.success,
            Status::Pending => |s| s.warning,
            Status::Refunded => |s| s.danger,
        }
    }
}

const ORDERS: [(&str, &str, &str, Status); 6] = [
    ("Northwind Traders", "Team", "$1,240.00", Status::Paid),
    ("Acme Robotics", "Enterprise", "$4,800.00", Status::Paid),
    ("Lumen Studio", "Starter", "$49.00", Status::Pending),
    ("Harbor & Co", "Team", "$620.00", Status::Refunded),
    ("Pinecone Labs", "Enterprise", "$3,950.00", Status::Paid),
    ("Oakline Dental", "Starter", "$49.00", Status::Pending),
];

#[derive(Default)]
struct Pulse {
    range: Range,
    dark: bool,
}

#[derive(Debug, Clone)]
enum Message {
    SetRange(Range),
    ToggleDark(bool),
}

impl Pulse {
    fn theme(&self) -> Theme {
        ui::theme(self.dark, ui::INDIGO)
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SetRange(range) => self.range = range,
            Message::ToggleDark(dark) => self.dark = dark,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let snapshot = self.range.snapshot();

        let header = row![
            ui::heading("Pulse"),
            space::horizontal(),
            ui::caption("Dark"),
            toggler(self.dark)
                .on_toggle(Message::ToggleDark)
                .size(18)
                .style(ui::switch),
            space::horizontal().width(8),
            ui::segmented(&Range::ALL, self.range, Message::SetRange),
        ]
        .spacing(8)
        .align_y(iced::Center);

        let tiles = row(snapshot.tiles.iter().map(tile)).spacing(14);

        let lower = row![chart(&snapshot), orders()]
            .spacing(14)
            .height(Length::Fill);

        container(column![header, tiles, lower].spacing(16))
            .padding([20, 28])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(ui::page)
            .into()
    }
}

fn tile(tile: &Tile) -> Element<'static, Message> {
    let delta = text(tile.delta)
        .size(13)
        .font(ui::SEMIBOLD)
        .style(if tile.good { ui::success } else { ui::danger });
    container(
        column![
            ui::caption(tile.label),
            text(tile.value).size(24).font(ui::SEMIBOLD),
            row![delta, ui::caption("vs previous")]
                .spacing(6)
                .align_y(iced::Center),
        ]
        .spacing(4),
    )
    .padding([14, 16])
    .width(Length::Fill)
    .style(ui::card)
    .into()
}

fn chart<'a>(snapshot: &Snapshot) -> Element<'a, Message> {
    let peak = snapshot.bars.iter().map(|&(_, v)| v).fold(0.0, f32::max);
    let last = snapshot.bars.len() - 1;

    let bars = snapshot
        .bars
        .iter()
        .enumerate()
        .map(|(i, &(label, value))| {
            // Heights are fill portions so the bars scale with the card.
            let height = (value / peak * 1000.0) as u16;
            let strength = if i == last { 1.0 } else { 0.55 };
            column![
                column![
                    space::vertical().height(Length::FillPortion(1000 - height + 1)),
                    container(space::vertical())
                        .width(Length::Fill)
                        .height(Length::FillPortion(height))
                        .style(ui::bar(strength)),
                ]
                .height(Length::Fill),
                text(label)
                    .size(11)
                    .style(ui::muted)
                    .center()
                    .width(Length::Fill),
            ]
            .spacing(6)
            .width(Length::Fill)
            .into()
        });

    container(
        column![
            row![
                text("Revenue").size(14).font(ui::SEMIBOLD),
                space::horizontal(),
                ui::caption(format!("{}  |  peak ${peak:.1}k", snapshot.caption)),
            ],
            row(bars).spacing(8).height(Length::Fill),
        ]
        .spacing(14),
    )
    .padding(16)
    .width(Length::FillPortion(5))
    .height(Length::Fill)
    .style(ui::card)
    .into()
}

fn orders<'a>() -> Element<'a, Message> {
    let cells = |a: Element<'a, Message>,
                 b: Element<'a, Message>,
                 c: Element<'a, Message>,
                 d: Element<'a, Message>| {
        row![
            container(a).width(Length::FillPortion(4)),
            container(b).width(Length::FillPortion(3)),
            container(c)
                .width(Length::FillPortion(3))
                .align_right(Length::FillPortion(3)),
            container(d)
                .width(Length::FillPortion(3))
                .align_right(Length::FillPortion(3)),
        ]
        .spacing(8)
        .align_y(iced::Center)
    };

    let head = cells(
        ui::caption("Customer").into(),
        ui::caption("Plan").into(),
        ui::caption("Amount").into(),
        ui::caption("Status").into(),
    );

    let mut table = column![head, rule()].spacing(8);
    for (customer, plan, amount, status) in ORDERS {
        let pill = container(text(status.label()).size(12))
            .padding([2, 8])
            .style(ui::pill(status.color()));
        table = table.push(
            container(cells(
                text(customer).into(),
                text(plan).style(ui::muted).into(),
                text(amount).into(),
                pill.into(),
            ))
            .padding([5, 0]),
        );
        table = table.push(rule());
    }

    container(column![text("Recent orders").size(14).font(ui::SEMIBOLD), table].spacing(14))
        .padding(16)
        .width(Length::FillPortion(7))
        .height(Length::Fill)
        .style(ui::card)
        .into()
}

fn rule<'a>() -> Element<'a, Message> {
    container(space::horizontal())
        .width(Length::Fill)
        .height(1)
        .style(ui::hairline)
        .into()
}
