use alloy_ui::prelude::*;
use alloy_ui::Boxed;
use rich::*;

choices! { enum Range { Week = "7 days", Month = "30 days", Quarter = "90 days" } }
choices! { enum Show { All, Paid, Pending, Refunded } }

#[derive(Clone, PartialEq)]
struct Pulse {
    range: Range,
    show: Show,
    sel: usize,
    grow: f32,
    dark: bool,
    line: bool,
    query: String,
    by_amount: bool,
}

impl Pulse {
    fn pick(&mut self, r: Range) {
        self.range = r;
        self.sel = r.bars().len() - 1;
        self.grow = 0.0;
    }
}

const ORDERS: [(&str, &str, &str, &str); 7] = [
    ("Northwind Traders", "Team", "$1,240.00", "Paid"),
    ("Acme Robotics", "Enterprise", "$4,800.00", "Paid"),
    ("Lumen Studio", "Starter", "$49.00", "Pending"),
    ("Harbor & Co", "Team", "$620.00", "Refunded"),
    ("Pinecone Labs", "Enterprise", "$3,950.00", "Paid"),
    ("Oakline Dental", "Starter", "$49.00", "Pending"),
    ("Fjord Analytics", "Team", "$1,240.00", "Paid"),
];

impl Range {
    /// Revenue in thousands per interval, with the axis label of each interval.
    fn bars(self) -> Vec<(String, f32)> {
        let (labels, values): (Vec<String>, &[f32]) = match self {
            Range::Week => (["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map(Into::into).into(), &[2.1, 2.6, 2.3, 3.4, 3.1, 4.0, 3.7]),
            Range::Month => ((0..10).map(|i| format!("{}", i * 3 + 1)).collect(), &[6.8, 7.4, 8.1, 7.0, 8.6, 9.2, 8.4, 9.9, 9.1, 10.7]),
            Range::Quarter => ((1..=12).map(|i| format!("W{i}")).collect(), &[18.0, 19.5, 21.0, 20.2, 23.0, 22.4, 24.5, 26.0, 25.1, 27.3, 28.8, 31.0]),
        };
        labels.into_iter().zip(values.iter().copied()).collect()
    }
    fn top(self) -> f32 {
        [4.0, 12.0, 40.0][self as usize]
    }
    /// Label, value, change, whether the change is good news, and a trend line.
    fn tiles(self) -> [(&'static str, String, &'static str, bool, f32); 4] {
        let r = self as usize;
        let n = (self.bars().iter().map(|b| b.1).sum::<f32>() * 1000.0).round() as u32;
        [
            ("Revenue", format!("${},{:03}", n / 1000, n % 1000), ["+8.2%", "+9.6%", "+14.0%"][r], true, 0.6),
            ("Active users", ["2,418", "9,872", "21,306"][r].into(), ["+3.1%", "+6.1%", "+11.4%"][r], true, 0.4),
            ("Conversion", ["3.42%", "4.10%", "4.40%"][r].into(), ["+0.4 pt", "+0.5 pt", "+0.9 pt"][r], true, 0.2),
            ("p95 latency", ["182 ms", "228 ms", "205 ms"][r].into(), ["-6.0%", "-1.7%", "-9.3%"][r], true, -0.3),
        ]
    }
}

fn k(v: f32) -> String {
    if v == 0.0 { "$0".into() } else { format!("${v:.0}k") }
}

fn trend(seed: f32) -> Vec<f32> {
    (0..9).map(|i| i as f32 * seed + (i as f32 * 1.7 + seed * 9.0).sin() * 0.8).collect()
}

fn tile(label: &str, value: String, delta: &str, good: bool, seed: f32) -> Boxed<Act<Pulse>> {
    let c = if good { palette().success } else { palette().danger };
    col![
        caption(label).dim(),
        row![heading(value), space(), spark(&trend(seed), c)].align_center(),
        row![caption(delta).color(c).label(), caption("vs previous").dim()].gap(6.0),
    ]
    .gap(4.0)
    .card()
    .fill_w()
}

fn orders(s: &Pulse) -> Boxed<Act<Pulse>> {
    let q = s.query.to_lowercase();
    let mut rows: Vec<_> = ORDERS
        .iter()
        .filter(|o| (s.show == Show::All || name(s.show) == o.3) && o.0.to_lowercase().contains(&q))
        .collect();
    let cents = |o: &&(&str, &str, &str, &str)| o.2.trim_start_matches('$').replace(',', "").parse::<f32>().unwrap_or(0.0);
    if s.by_amount {
        rows.sort_by(|a, b| cents(b).total_cmp(&cents(a)));
    }
    let n = rows.len();
    let right = |n: Node<Act<Pulse>>, w: f32| col![n].align_end().w(w);
    let head = row![
        caption("Customer").dim().fill_w(),
        caption("Plan").dim().w(96.0),
        right(caption("Amount").dim().into(), 84.0),
        right(caption("Status").dim().into(), 84.0)
    ]
    .gap(8.0);
    let body = rows.into_iter().flat_map(|(who, plan, amount, status)| {
        let line = row![
            text(*who).no_wrap().fill_w(),
            text(*plan).dim().w(96.0),
            right(text(*amount).into(), 84.0),
            right(pill(status).into(), 84.0)
        ]
        .gap(8.0)
        .align_center();
        [Node::from(rule()), line.pad_xy(0.0, 5.0).into()]
    });
    let filters = choose(Show::all().iter().map(|&(l, v)| (l, v)), s.show).act(|s: &mut Pulse, v| s.show = v);
    let list: Node<_> = if n == 0 {
        empty("No orders match", "Try another name or status.", button("Reset filters").act(|s: &mut Pulse| {
            s.query.clear();
            s.show = Show::All;
        }))
    } else {
        col(body).gap(0.0).into()
    };
    col![
        row![heading("Recent orders").sub(), space(), caption(format!("{n} shown")).dim(), check("Largest first", at!(s.by_amount)), button("Export CSV").act(|_| {
 toast("Exported orders.csv");
 })].gap(10.0).align_center(),
        row![input(&s.query).hint("Search customers").bind(|s: &mut Pulse| &mut s.query).fill_w(), filters].gap(10.0),
        head,
        list,
    ]
        .gap(10.0)
        .card()
        .fill()
}

fn chart(s: &Pulse) -> Boxed<Act<Pulse>> {
    let bars = s.range.bars();
    let (at, v) = bars[s.sel.min(bars.len() - 1)].clone();
    let p = palette();
    let peak = bars.iter().map(|b| b.1).fold(0.0, f32::max);
    col![
        row![heading("Revenue").sub(), caption(format!("peak ${peak:.1}k")).dim(), space(), choose([("Bars", false), ("Line", true)], s.line).act(|s: &mut Pulse, l| s.line = l)]
            .gap(10.0)
            .align_center(),
        row![text(format!("${:.1}k", v)).label(), caption(format!("on {at}")).dim()].gap(6.0),
        plot(bars, s.range.top(), k).selected(s.sel).grow(s.grow).line(s.line).act(|s: &mut Pulse, i| s.sel = i).fill(),
        caption("Revenue by plan").dim(),
        stack(&[(48.0, p.accent), (31.0, p.accent.with_alpha(0.6)), (21.0, p.accent.with_alpha(0.3))]),
        row![legend(p.accent, "Enterprise 48%"), legend(p.accent.with_alpha(0.6), "Team 31%"), legend(p.accent.with_alpha(0.3), "Starter 21%")].gap(14.0),
    ]
    .gap(8.0)
    .card()
    .w(430.0)
    .fill_h()
}

fn view(s: &Pulse) -> impl View<Act<Pulse>> {
    set_accent(Color::hex(INDIGO));
    set_dark(s.dark);
    col![
        row![
            col![heading("Pulse"), caption("Compared with the previous period").dim()].gap(2.0),
            space(),
            toggle("Dark", at!(s.dark)),
            options(s.range).act(|s: &mut Pulse, r| s.pick(r)),
        ]
        .gap(16.0)
        .align_center(),
        row(s.range.tiles().map(|(l, v, d, g, sd)| tile(l, v, d, g, sd))).gap(12.0),
        row![chart(s), orders(s)].gap(12.0).fill(),
    ]
    .gap(14.0)
    .pad(20.0)
    .fill()
}

fn main() {
    let s = Pulse { range: Range::Week, show: Show::All, sel: 6, grow: 1.0, dark: false, line: false, query: String::new(), by_amount: false };
    sketch(s, view)
        .tick_while(|s| s.grow < 1.0, secs(0.016), |s| s.grow = (s.grow + 0.06).min(1.0))
        .shortcut_as("mod+.", "Skip animation", |s| s.grow = 1.0)
        .shortcut("1", |s| s.pick(Range::Week))
        .shortcut("2", |s| s.pick(Range::Month))
        .shortcut("3", |s| s.pick(Range::Quarter))
        .shortcut_as("mod+l", "Switch bars and line", |s| s.line ^= true)
        .shortcut_as("mod+d", "Toggle dark", |s| s.dark ^= true)
        .run_sized("Pulse", 960.0, 640.0)
}
