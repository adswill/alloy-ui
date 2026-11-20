//! Widget gallery: every widget in every state, light and dark, in one frame.
//!
//! This is the visual review surface for the theme. `--headless [file.png] [--dark]` renders it
//! without a window; hover, pressed and focus are pinned with [`Forced`] so one image shows them all.
use alloy_ui::layout::Length;
use alloy_ui::render::{CpuRenderer, HeadlessRenderer, TextMeasurer};
use alloy_ui::style::Padding;
use alloy_ui::theme::Theme;
use alloy_ui::widgets::*;
use alloy_ui::{DrawList, Element, Point, Size};
use std::rc::Rc;

/// Window size in logical pixels.
pub const SIZE: (f32, f32) = (1360.0, 1640.0);

type E = Element<()>;

struct Metrics(TextMeasurer);
impl TextMetrics for Metrics {
    fn width(&self, text: &str, size: f32) -> f32 {
        self.0.measure(text, size, 400, None).width
    }
}

fn label(ui: &Ui, s: &str) -> E {
    Text::new(ui, s)
        .role(TextRole::Caption)
        .secondary()
        .into_element()
}

fn panel(ui: &Ui, title: &str, rows: Vec<E>) -> E {
    let t = ui.theme();
    let mut kids = vec![Text::new(ui, title)
        .role(TextRole::Subheading)
        .into_element()];
    kids.extend(rows);
    Container::new(
        Column::new(kids)
            .spacing(t.spacing.md)
            .width(Length::Fill(1))
            .into_element(),
    )
    .padding(Padding::all(t.spacing.lg))
    .background(t.palette.surface)
    .border(t.palette.border, 1.0)
    .radius(t.radii.large)
    .width(Length::Fill(1))
    .into_element()
}

fn states(ui: &Ui, names: &[&str], cells: Vec<E>) -> E {
    let t = ui.theme();
    let mut cols = Vec::new();
    for (n, c) in names.iter().zip(cells) {
        cols.push(
            Column::new(vec![label(ui, n), c])
                .spacing(t.spacing.xs + 2.0)
                .into_element(),
        );
    }
    Row::new(cols).spacing(t.spacing.md + 4.0).into_element()
}

fn buttons(ui: &Ui, v: Variant, name: &str, id: u64) -> E {
    let b = |l: &str, i: u64| Button::new(ui, l).variant(v).on_press(()).id(i);
    let cells = vec![
        b(name, id).into_element(),
        Forced::new(ui, b(name, id + 1).into_element())
            .hover()
            .into_element(),
        Forced::new(ui, b(name, id + 2).into_element())
            .press()
            .into_element(),
        Forced::new(ui, b(name, id + 3).into_element())
            .focus(id + 3)
            .into_element(),
        Button::<()>::new(ui, name).variant(v).into_element(),
    ];
    states(
        ui,
        &["Rest", "Hover", "Pressed", "Focus", "Disabled"],
        cells,
    )
}

fn toggles(ui: &Ui) -> E {
    let t = ui.theme();
    let cb = |on: bool, id: u64, en: bool| {
        let c = Checkbox::new(ui, "Notify me", on).id(id);
        if en { c.on_toggle(|_| ()) } else { c }.into_element()
    };
    let rd = |on: bool, id: u64, en: bool| {
        let r = Radio::new(ui, "Weekly", on).id(id);
        if en { r.on_select(()) } else { r }.into_element()
    };
    let tg = |on: bool, id: u64, en: bool| {
        let r = Toggle::new(ui, "Sync", on).id(id);
        if en { r.on_toggle(|_| ()) } else { r }.into_element()
    };
    let names = ["Rest", "Hover", "Focus", "Disabled"];
    let rowof = |f: &dyn Fn(bool, u64, bool) -> E, base: u64| {
        let mut out = Vec::new();
        for on in [false, true] {
            let o = on as u64 * 10 + base;
            out.push(states(
                ui,
                &names,
                vec![
                    f(on, o, true),
                    Forced::new(ui, f(on, o + 1, true)).hover().into_element(),
                    Forced::new(ui, f(on, o + 2, true))
                        .focus(o + 2)
                        .into_element(),
                    f(on, o + 3, false),
                ],
            ));
        }
        Column::new(out).spacing(t.spacing.md).into_element()
    };
    Column::new(vec![rowof(&cb, 100), rowof(&rd, 120), rowof(&tg, 140)])
        .spacing(t.spacing.md)
        .into_element()
}

fn inputs(ui: &Ui) -> E {
    let t = ui.theme();
    let ti = |id: u64, v: &str| {
        TextInput::new(ui, id, v)
            .placeholder("name@company.com")
            .on_input(|_| ())
            .width(Length::Fixed(176.0))
    };
    let row1 = states(
        ui,
        &["Empty", "Filled", "Hover"],
        vec![
            ti(200, "").into_element(),
            ti(201, "kris@alloy.dev").into_element(),
            Forced::new(ui, ti(202, "").into_element())
                .hover()
                .into_element(),
        ],
    );
    let row2 = states(
        ui,
        &["Focus", "Error", "Read only"],
        vec![
            Forced::new(ui, ti(203, "kris@alloy.dev").into_element())
                .focus(203)
                .into_element(),
            ti(204, "kris@alloy").invalid(true).into_element(),
            TextInput::<()>::new(ui, 205, "Locked value")
                .width(Length::Fixed(176.0))
                .into_element(),
        ],
    );
    Column::new(vec![row1, row2])
        .spacing(t.spacing.md)
        .into_element()
}

fn selection(ui: &Ui) -> E {
    let t = ui.theme();
    let seg = Segmented::new(
        ui,
        300,
        vec!["Day".into(), "Week".into(), "Month".into()],
        1,
    )
    .on_select(|_| ());
    let pick = PickList::new(
        ui,
        301,
        vec!["Light".into(), "Dark".into(), "System".into()],
        Some(2),
    )
    .on_select(|_| ())
    .width(Length::Fixed(180.0));
    let num = NumberInput::new(ui, 302, 14.0)
        .range(8.0, 32.0)
        .unit("px")
        .on_change(|_| ())
        .width(Length::Fixed(130.0));
    let slider = |v: f32, id: u64| {
        Slider::new(ui, 0.0, 100.0, v)
            .on_change(|_| ())
            .id(id)
            .width(Length::Fixed(176.0))
    };
    Column::new(vec![
        Row::new(vec![
            seg.into_element(),
            pick.into_element(),
            num.into_element(),
        ])
        .spacing(t.spacing.md)
        .into_element(),
        Row::new(vec![
            slider(35.0, 310).into_element(),
            Forced::new(ui, slider(60.0, 311).into_element())
                .hover()
                .into_element(),
            Forced::new(ui, slider(80.0, 312).into_element())
                .focus(312)
                .into_element(),
        ])
        .spacing(t.spacing.md)
        .into_element(),
        Row::new(vec![
            ProgressBar::new(ui, 0.0, 100.0, 64.0)
                .width(Length::Fixed(176.0))
                .into_element(),
            ProgressBar::new(ui, 0.0, 100.0, 12.0)
                .width(Length::Fixed(176.0))
                .into_element(),
        ])
        .spacing(t.spacing.lg)
        .into_element(),
    ])
    .spacing(t.spacing.lg)
    .into_element()
}

fn type_scale(ui: &Ui) -> E {
    let t = ui.theme();
    let line = |r: TextRole, s: &str| Text::new(ui, s).role(r).into_element();
    Column::new(vec![
        line(TextRole::Display, "Ship the quiet interface"),
        line(TextRole::Heading, "Heading, section title"),
        line(TextRole::Subheading, "Subheading for a group of settings"),
        line(
            TextRole::Body,
            "Body text sits at fourteen pixels with a generous line height.",
        ),
        line(TextRole::Label, "Label, used on buttons and field names"),
        Text::new(ui, "Caption: secondary detail, hints and timestamps")
            .role(TextRole::Caption)
            .secondary()
            .into_element(),
    ])
    .spacing(t.spacing.sm)
    .into_element()
}

fn surfaces(ui: &Ui) -> E {
    let t = ui.theme();
    let p = &t.palette;
    let card = |name: &str, bg, shadow: Option<alloy_ui::Shadow>| {
        let mut c = Container::new(
            Column::new(vec![
                Text::new(ui, name).role(TextRole::Label).into_element(),
                label(ui, "Height only"),
            ])
            .spacing(2.0)
            .into_element(),
        )
        .padding(Padding::all(t.spacing.md))
        .background(bg)
        .border(p.border, 1.0)
        .radius(t.radii.large)
        .width(Length::Fixed(118.0));
        if let Some(s) = shadow {
            c = c.shadow(s);
        }
        c.into_element()
    };
    Row::new(vec![
        card("Flat", p.surface, None),
        card("Raised", p.surface_raised, Some(t.elevation.low)),
        card("Popover", p.surface_raised, Some(t.elevation.medium)),
        card("Dialog", p.surface_raised, Some(t.elevation.high)),
    ])
    .spacing(t.spacing.lg)
    .into_element()
}

fn data(ui: &Ui) -> E {
    let t = ui.theme();
    let tbl = Table::new(
        ui,
        400,
        vec![
            TableColumn::new("Job", Length::Fill(2)),
            TableColumn::new("State", Length::Fill(1)),
            TableColumn::new("Took", Length::Fill(1)).right(),
        ],
        |_| (),
    )
    .row(cells(ui, ["nightly-index", "done", "4m 12s"]))
    .row(cells(ui, ["thumbnail-sweep", "done", "38s"]))
    .row(cells(ui, ["export-ledger", "failed", "1m 05s"]))
    .row(cells(ui, ["sync-calendars", "running", "12s"]))
    .selected(Some(2))
    .sorted(0, true)
    .on_sort(|_| ());
    let chart = BarChart::new(
        ui,
        vec![32.0, 41.0, 38.0, 52.0, 61.0, 58.0, 72.0, 69.0, 81.0, 64.0],
    )
    .labels(["9", "10", "11", "12", "13", "14", "15", "16", "17", "18"])
    .max(100.0)
    .height(120.0)
    .into_element();
    let chart = Container::new(chart)
        .height(Length::Fixed(150.0))
        .into_element();
    Column::new(vec![tbl.into_element(), chart])
        .spacing(t.spacing.lg)
        .into_element()
}

fn navigation(ui: &Ui) -> E {
    let t = ui.theme();
    let row = |l: &str, id: u64, sel: bool| {
        Button::new(ui, l)
            .variant(Variant::Nav)
            .selected(sel)
            .on_press(())
            .width(Length::Fill(1))
            .id(id)
            .into_element()
    };
    let nav = Column::new(vec![
        row("Appearance", 500, true),
        Forced::new(ui, row("Keyboard", 501, false))
            .hover()
            .into_element(),
        Forced::new(ui, row("Notifications", 502, false))
            .focus(502)
            .into_element(),
        row("Accounts", 503, false),
    ])
    .spacing(2.0)
    .width(Length::Fill(1))
    .into_element();
    let _ = t;
    nav
}

fn dates(ui: &Ui) -> E {
    let t = ui.theme();
    Column::new(vec![
        label(ui, "Due date"),
        DatePicker::new(ui, 510, Some(Date::new(2026, 11, 20)))
            .today(Date::new(2026, 11, 18))
            .on_select(|_| ())
            .width(Length::Fixed(240.0))
            .into_element(),
    ])
    .spacing(t.spacing.xs + 2.0)
    .width(Length::Fill(1))
    .into_element()
}

fn cells(ui: &Ui, c: [&str; 3]) -> Vec<E> {
    c.iter().map(|s| Text::new(ui, *s).into_element()).collect()
}

fn page(ui: &Ui) -> E {
    let t = ui.theme();
    let title = Row::new(vec![
        Text::new(ui, "Alloy widget gallery")
            .role(TextRole::Heading)
            .into_element(),
        Space::fill_w().into_element(),
        Text::new(
            ui,
            if t.appearance == alloy_ui::theme::Appearance::Dark {
                "Dark"
            } else {
                "Light"
            },
        )
        .role(TextRole::Caption)
        .secondary()
        .into_element(),
    ])
    .width(Length::Fill(1))
    .into_element();
    let left = Column::new(vec![
        panel(
            ui,
            "Primary",
            vec![buttons(ui, Variant::Primary, "Save", 10)],
        ),
        panel(
            ui,
            "Secondary",
            vec![buttons(ui, Variant::Secondary, "Cancel", 20)],
        ),
        panel(ui, "Ghost", vec![buttons(ui, Variant::Ghost, "Skip", 30)]),
        panel(ui, "Inputs", vec![inputs(ui)]),
        panel(ui, "Choices", vec![toggles(ui)]),
        panel(ui, "Navigation", vec![navigation(ui)]),
    ])
    .spacing(t.spacing.md + 4.0)
    .width(Length::Fill(11))
    .into_element();
    let right = Column::new(vec![
        panel(ui, "Type scale", vec![type_scale(ui)]),
        panel(ui, "Selection and range", vec![selection(ui), dates(ui)]),
        panel(ui, "Elevation", vec![surfaces(ui)]),
        panel(ui, "Data", vec![data(ui)]),
    ])
    .spacing(t.spacing.md + 4.0)
    .width(Length::Fill(10))
    .into_element();
    Container::new(
        Column::new(vec![
            title,
            Row::new(vec![left, right])
                .spacing(t.spacing.md + 4.0)
                .width(Length::Fill(1))
                .into_element(),
        ])
        .spacing(t.spacing.md + 4.0)
        .width(Length::Fill(1))
        .into_element(),
    )
    .padding(Padding::all(t.spacing.lg))
    .width(Length::Fill(1))
    .height(Length::Fill(1))
    .into_element()
}

/// Render the gallery at 2x as a PNG.
pub fn render_png(dark: bool) -> Vec<u8> {
    let theme = if dark { Theme::dark() } else { Theme::light() };
    let bg = theme.palette.background;
    let ui = Ui::new(theme);
    ui.set_metrics(Rc::new(Metrics(TextMeasurer::new())));
    ui.set_popup(Some(510));
    let root = Root::new(&ui, page(&ui)).into_element();
    let size = Size::new(SIZE.0, SIZE.1);
    let layout = alloy_ui::widgets::layout(&root, size);
    let mut list = DrawList::default();
    root.0.draw(&layout, &mut list);
    let (w, h) = (SIZE.0 as u32 * 2, SIZE.1 as u32 * 2);
    let mut r = CpuRenderer::new(w, h);
    r.set_background(bg);
    let _ = Point::default();
    r.render_to_image(&list, w, h, 2.0)
        .encode_png()
        .expect("png")
}

/// Write the PNG named by `--headless [file]` (light, or `--dark`).
pub fn run() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .iter()
        .position(|a| a == "--headless")
        .and_then(|i| args.get(i + 1))
        .filter(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(|| "gallery.png".into());
    std::fs::write(&path, render_png(args.iter().any(|a| a == "--dark"))).expect("write png");
    println!("wrote {path}");
}
