//! Golden-image tests: render widgets with the CPU reference rasteriser and
//! compare with committed PNGs. Text commands are stripped (glyphs differ per
//! OS font stack), so these cover shapes, colours, borders, focus rings and state.
//! Regenerate with `UPDATE_GOLDEN=1 cargo test -p alloy-ui-widgets --test golden`.

use alloy_ui_core::*;
use alloy_ui_layout::Length;
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::path::Path;

const W: u32 = 420;
const H: u32 = 300;

fn render(
    theme: Theme,
    build: impl Fn(&Ui) -> Element<()>,
    setup: impl Fn(&Ui),
) -> alloy_ui_render::Image {
    let bg = theme.palette.background;
    let ui = Ui::new(theme);
    setup(&ui);
    let root = Root::new(&ui, build(&ui)).into_element();
    let layout = alloy_ui_widgets::layout(&root, Size::new(W as f32, H as f32));
    let mut list = DrawList::default();
    root.0.draw(&layout, &mut list);
    // Glyph shapes differ per OS font stack, so goldens only keep shapes.
    list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
    let mut r = CpuRenderer::new(W * 2, H * 2);
    r.set_background(bg);
    r.render_to_image(&list, W * 2, H * 2, 2.0)
}

fn check(name: &str, img: &alloy_ui_render::Image) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    assert_golden(&dir, name, img, Tolerance::CPU);
}

fn page(ui: &Ui, rows: Vec<Element<()>>) -> Element<()> {
    let t = ui.theme();
    Container::new(
        Column::new(rows)
            .spacing(t.spacing.md)
            .width(Length::Fill(1))
            .into_element(),
    )
    .padding(alloy_ui_core::style::Padding::all(t.spacing.lg))
    .width(Length::Fill(1))
    .height(Length::Fill(1))
    .into_element()
}

fn controls(ui: &Ui) -> Element<()> {
    page(
        ui,
        vec![
            Row::new(vec![
                Button::new(ui, "Save")
                    .variant(Variant::Primary)
                    .on_press(())
                    .id(1)
                    .into_element(),
                Button::new(ui, "Cancel").on_press(()).id(2).into_element(),
                Button::new(ui, "Skip")
                    .variant(Variant::Ghost)
                    .on_press(())
                    .into_element(),
                Button::<()>::new(ui, "Locked").into_element(),
            ])
            .spacing(ui.theme().spacing.sm)
            .into_element(),
            Row::new(vec![
                Checkbox::new(ui, "On", true)
                    .on_toggle(|_| ())
                    .id(3)
                    .into_element(),
                Checkbox::new(ui, "Off", false)
                    .on_toggle(|_| ())
                    .into_element(),
                Toggle::new(ui, "", true).on_toggle(|_| ()).into_element(),
                Toggle::new(ui, "", false).on_toggle(|_| ()).into_element(),
            ])
            .spacing(ui.theme().spacing.lg)
            .into_element(),
            Slider::new(ui, 0.0, 100.0, 35.0)
                .on_change(|_| ())
                .id(4)
                .into_element(),
            ProgressBar::new(ui, 0.0, 1.0, 0.6).into_element(),
            TextInput::new(ui, 5, "")
                .placeholder("Search")
                .on_input(|_| ())
                .into_element(),
            TextInput::new(ui, 6, "alloy")
                .on_input(|_| ())
                .into_element(),
        ],
    )
}

#[test]
fn controls_light() {
    check("controls_light", &render(Theme::light(), controls, |_| {}));
}

#[test]
fn controls_dark() {
    check("controls_dark", &render(Theme::dark(), controls, |_| {}));
}

#[test]
fn controls_focused_input_with_selection() {
    let img = render(Theme::light(), controls, |ui| {
        ui.set_focus(Some(6));
    });
    check("controls_focus_input", &img);
}

#[test]
fn list_and_scroll_light() {
    let img = render(
        Theme::light(),
        |ui| {
            let items = (0..12)
                .map(|i| Text::new(ui, format!("Item {i}")).into_element())
                .collect();
            page(
                ui,
                vec![List::new(ui, 20, items, Some(2), |_| ()).into_element()],
            )
        },
        |ui| ui.set_scroll_offset(20, 30.0),
    );
    check("list_light", &img);
}

#[test]
fn pick_list_open_light() {
    let img = render(
        Theme::light(),
        |ui| {
            let opts = vec!["System".to_string(), "Light".into(), "Dark".into()];
            page(
                ui,
                vec![PickList::new(ui, 30, opts, Some(1))
                    .on_select(|_| ())
                    .into_element()],
            )
        },
        |ui| {
            ui.set_popup(Some(30));
            ui.set_focus(Some(30));
        },
    );
    check("pick_list_open", &img);
}

#[test]
fn choices_light() {
    let img = render(
        Theme::light(),
        |ui| {
            page(
                ui,
                vec![
                    Segmented::new(ui, 1, vec!["Day".into(), "Week".into(), "Month".into()], 1)
                        .on_select(|_| ())
                        .into_element(),
                    Column::new(vec![
                        Radio::new(ui, "System", false).on_select(()).into_element(),
                        Radio::new(ui, "Light", true)
                            .on_select(())
                            .id(2)
                            .into_element(),
                        Radio::<()>::new(ui, "Dark", false).into_element(),
                    ])
                    .spacing(ui.theme().spacing.sm)
                    .into_element(),
                    Rule::horizontal(ui).into_element(),
                    Row::new(vec![
                        Tooltip::new(
                            ui,
                            Button::new(ui, "Hover me").on_press(()).into_element(),
                            "x",
                        )
                        .into_element(),
                        Space::fill().into_element(),
                        Button::new(ui, "Done")
                            .variant(Variant::Primary)
                            .on_press(())
                            .into_element(),
                    ])
                    .width(Length::Fill(1))
                    .into_element(),
                ],
            )
        },
        |ui| ui.set_focus(Some(1)),
    );
    check("choices_light", &img);
}

#[test]
fn number_and_date_inputs_light() {
    let img = render(
        Theme::light(),
        |ui| {
            page(
                ui,
                vec![
                    NumberInput::new(ui, 20, 12.0)
                        .range(0.0, 20.0)
                        .unit(" px")
                        .on_change(|_| ())
                        .into_element(),
                    DatePicker::new(ui, 21, Some(Date::new(2026, 11, 3)))
                        .today(Date::new(2026, 11, 2))
                        .on_select(|_| ())
                        .into_element(),
                ],
            )
        },
        |ui| ui.set_popup(Some(21)),
    );
    check("inputs_light", &img);
}

#[test]
fn gradient_and_elevation_light() {
    let build = |ui: &Ui| {
        let t = ui.theme();
        let p = t.palette.clone();
        let card = |c: Container<()>| {
            c.padding(alloy_ui_core::style::Padding::all(24.0))
                .width(Length::Fixed(150.0))
                .height(Length::Fixed(80.0))
                .radius(t.radii.large)
                .into_element()
        };
        page(
            ui,
            vec![Row::new(vec![
                card(
                    Container::new(Space::fixed(1.0, 1.0).into_element()).gradient(
                        p.accent,
                        p.accent_hover.with_alpha(0.55),
                        90.0,
                    ),
                ),
                card(
                    Container::new(Space::fixed(1.0, 1.0).into_element())
                        .background(p.surface_raised)
                        .border(p.border, 1.0)
                        .shadow(t.elevation.medium),
                ),
            ])
            .spacing(24.0)
            .into_element()],
        )
    };
    check(
        "gradient_elevation_light",
        &render(Theme::light(), build, |_| {}),
    );
}

fn rich(ui: &Ui) -> Element<()> {
    let series = || {
        vec![
            Series::new("A", vec![3.0, 5.0, 4.0, 8.0, 6.0]),
            Series::new("B", vec![1.0, 2.0, 2.0, 3.0, 2.0]),
        ]
    };
    page(
        ui,
        vec![
            Row::new(vec![
                Badge::new(ui, "Active")
                    .tone(Tone::Success)
                    .dot()
                    .into_element(),
                Badge::new(ui, "Failed")
                    .tone(Tone::Danger)
                    .solid()
                    .into_element(),
                Badge::new(ui, "Beta")
                    .tone(Tone::Accent)
                    .outline()
                    .into_element(),
                Swatch::<()>::new(ui, Color::hex(0x4338CA))
                    .selected(true)
                    .into_element(),
                Swatch::<()>::new(ui, Color::hex(0xC2185B)).into_element(),
                Sparkline::new(ui, vec![1.0, 3.0, 2.0, 5.0, 4.0]).into_element(),
            ])
            .spacing(10.0)
            .into_element(),
            Chart::line(ui, series())
                .x_labels(["a", "b", "c", "d", "e"])
                .area()
                .height(110.0)
                .into_element(),
            Row::new(vec![
                Tree::<()>::new(
                    ui,
                    vec![
                        TreeItem::new(1, "src").children([TreeItem::new(2, "lib.rs")]),
                        TreeItem::new(3, "Cargo.toml"),
                    ],
                )
                .expanded([1])
                .selected(Some(2))
                .into_element(),
                ColorPicker::<()>::new(ui, 9, Color::hex(0x0B6B5F)).into_element(),
            ])
            .spacing(16.0)
            .into_element(),
        ],
    )
}

#[test]
fn rich_light() {
    let img = render(Theme::light(), rich, |_| {});
    check("rich_light", &img);
}

#[test]
fn rich_dark_accent() {
    let img = render(
        Theme::dark().with_accent(Color::hex(0x4338CA)),
        rich,
        |_| {},
    );
    check("rich_dark_accent", &img);
}
