//! Behavioural tests for the widget library: layout, events, focus, drawing.
use alloy_ui_core::*;
use alloy_ui_layout::Length;
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Clicked,
    Toggled(bool),
    Value(f32),
    Text(String),
    Picked(usize),
    Submit,
}

struct Harness {
    root: Element<Msg>,
    layout: Layout,
    cursor: Point,
}

impl Harness {
    fn new(ui: &Ui, child: Element<Msg>, w: f32, h: f32) -> Self {
        let root = Root::new(ui, child).into_element();
        let layout = root
            .0
            .arrange(Rectangle::new(Point::default(), Size::new(w, h)));
        Self {
            root,
            layout,
            cursor: Point::default(),
        }
    }
    fn send(&mut self, e: Event) -> Vec<Msg> {
        if let Event::CursorMoved(p) = &e {
            self.cursor = *p;
        }
        let mut out = Vec::new();
        self.root
            .0
            .on_event(&e, &self.layout, self.cursor, &mut out);
        out
    }
    fn mv(&mut self, x: f32, y: f32) -> Vec<Msg> {
        self.send(Event::CursorMoved(Point::new(x, y)))
    }
    fn click(&mut self, x: f32, y: f32) -> Vec<Msg> {
        self.mv(x, y);
        let mut m = self.send(Event::MousePressed(MouseButton::Left));
        m.extend(self.send(Event::MouseReleased(MouseButton::Left)));
        m
    }
    fn key(&mut self, k: Key) -> Vec<Msg> {
        self.send(Event::KeyPressed(k, Modifiers::default()))
    }
    fn tick(&mut self, ms: u64) {
        self.send(Event::Tick(ms));
    }
    fn animating(&self) -> bool {
        self.root.0.animating()
    }
    fn draw(&self) -> DrawList {
        let mut l = DrawList::default();
        self.root.0.draw(&self.layout, &mut l);
        l
    }
    fn texts(&self) -> Vec<String> {
        self.draw()
            .0
            .into_iter()
            .filter_map(|c| match c {
                DrawCommand::Text { content, .. } => Some(content),
                _ => None,
            })
            .collect()
    }
}

fn ui() -> Ui {
    Ui::new(Theme::light())
}

#[test]
fn column_stacks_children_with_spacing() {
    let ui = ui();
    let col = Column::new(vec![
        Text::new(&ui, "one").into_element(),
        Text::new(&ui, "two").into_element(),
    ])
    .spacing(10.0);
    let h = Harness::new(&ui, col.into_element(), 300.0, 300.0);
    let kids = &h.layout.children[0].children;
    assert_eq!(kids.len(), 2);
    let lh = ui.theme().typography.body.line_height;
    assert_eq!(kids[0].bounds.size.height, lh);
    assert_eq!(kids[1].bounds.origin.y, lh + 10.0);
}

#[test]
fn row_fill_shares_space() {
    let ui = ui();
    let row = Row::new(vec![
        Container::new(Text::new(&ui, "a").into_element())
            .width(Length::Fill(1))
            .into_element(),
        Container::new(Text::new(&ui, "b").into_element())
            .width(Length::Fill(3))
            .into_element(),
    ])
    .width(Length::Fill(1));
    let h = Harness::new(&ui, row.into_element(), 400.0, 100.0);
    let k = &h.layout.children[0].children;
    assert_eq!(k[0].bounds.size.width, 100.0);
    assert_eq!(k[1].bounds.size.width, 300.0);
}

#[test]
fn text_wraps_in_narrow_column() {
    let ui = ui();
    let t = Text::new(&ui, "the quick brown fox jumps over the lazy dog").width(Length::Fill(1));
    let h = Harness::new(
        &ui,
        Column::new(vec![t.into_element()])
            .width(Length::Fill(1))
            .into_element(),
        100.0,
        400.0,
    );
    assert!(h.texts().len() > 1, "{:?}", h.texts());
    assert_eq!(
        h.texts().join(" "),
        "the quick brown fox jumps over the lazy dog"
            .replace("  ", " ")
            .split(' ')
            .collect::<Vec<_>>()
            .join(" ")
            .trim_end()
    );
}

#[test]
fn container_draws_background_only_when_set() {
    let ui = ui();
    let plain = Container::new(Text::new(&ui, "x").into_element());
    let h = Harness::new(&ui, plain.into_element(), 100.0, 100.0);
    assert!(!h
        .draw()
        .0
        .iter()
        .any(|c| matches!(c, DrawCommand::Quad { .. })));
    let styled = Container::new(Text::new(&ui, "x").into_element())
        .background(Color::hex(0xffffff))
        .radius(4.0);
    let h = Harness::new(&ui, styled.into_element(), 100.0, 100.0);
    assert!(h
        .draw()
        .0
        .iter()
        .any(|c| matches!(c, DrawCommand::Quad { .. })));
}

#[test]
fn button_click_emits_message_on_release_inside() {
    let ui = ui();
    let b = Button::new(&ui, "Save").on_press(Msg::Clicked);
    let mut h = Harness::new(&ui, b.into_element(), 200.0, 100.0);
    assert_eq!(h.click(5.0, 5.0), vec![Msg::Clicked]);
    // press inside, release outside: no click
    h.mv(5.0, 5.0);
    h.send(Event::MousePressed(MouseButton::Left));
    h.mv(150.0, 90.0);
    assert!(h.send(Event::MouseReleased(MouseButton::Left)).is_empty());
}

#[test]
fn press_survives_view_rebuild() {
    let ui = ui();
    let mk = |ui: &Ui| {
        Harness::new(
            ui,
            Button::new(ui, "Save")
                .on_press(Msg::Clicked)
                .into_element(),
            200.0,
            100.0,
        )
    };
    let mut h = mk(&ui);
    h.mv(5.0, 5.0);
    h.send(Event::MousePressed(MouseButton::Left));
    let mut h2 = mk(&ui); // view rebuilt between press and release
    h2.cursor = h.cursor;
    assert_eq!(
        h2.send(Event::MouseReleased(MouseButton::Left)),
        vec![Msg::Clicked]
    );
}

#[test]
fn disabled_button_does_nothing() {
    let ui = ui();
    let mut h = Harness::new(
        &ui,
        Button::<Msg>::new(&ui, "Nope").into_element(),
        200.0,
        100.0,
    );
    assert!(h.click(5.0, 5.0).is_empty());
}

#[test]
fn tab_cycles_focus_and_enter_activates() {
    let ui = ui();
    let col = Column::new(vec![
        Button::new(&ui, "A")
            .id(1)
            .on_press(Msg::Clicked)
            .into_element(),
        Button::new(&ui, "B")
            .id(2)
            .on_press(Msg::Submit)
            .into_element(),
    ])
    .spacing(8.0);
    let mut h = Harness::new(&ui, col.into_element(), 300.0, 300.0);
    h.draw(); // registers tab order
    assert_eq!(ui.focused(), None);
    h.key(Key::Tab);
    assert_eq!(ui.focused(), Some(1));
    h.key(Key::Tab);
    assert_eq!(ui.focused(), Some(2));
    assert_eq!(h.key(Key::Enter), vec![Msg::Submit]);
    h.key(Key::Tab);
    assert_eq!(ui.focused(), Some(1), "wraps");
    h.send(Event::KeyPressed(
        Key::Tab,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    ));
    assert_eq!(ui.focused(), Some(2), "shift-tab goes back");
}

#[test]
fn clicking_empty_space_clears_focus() {
    let ui = ui();
    let mut h = Harness::new(
        &ui,
        Button::new(&ui, "A")
            .id(1)
            .on_press(Msg::Clicked)
            .into_element(),
        300.0,
        300.0,
    );
    h.click(5.0, 5.0);
    assert_eq!(ui.focused(), Some(1));
    h.click(250.0, 250.0);
    assert_eq!(ui.focused(), None);
}

#[test]
fn focused_button_draws_ring_and_space_activates() {
    let ui = ui();
    let mut h = Harness::new(
        &ui,
        Button::new(&ui, "A")
            .id(1)
            .on_press(Msg::Clicked)
            .into_element(),
        300.0,
        300.0,
    );
    h.draw();
    h.key(Key::Tab);
    let quads = h
        .draw()
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Quad { .. }))
        .count();
    assert_eq!(quads, 2, "ring + body");
    assert_eq!(h.key(Key::Character(" ".into())), vec![Msg::Clicked]);
}

#[test]
fn checkbox_toggles_by_click_and_space() {
    let ui = ui();
    let cb = Checkbox::new(&ui, "Notify", false)
        .id(3)
        .on_toggle(Msg::Toggled);
    let mut h = Harness::new(&ui, cb.into_element(), 300.0, 100.0);
    assert_eq!(h.click(4.0, 4.0), vec![Msg::Toggled(true)]);
    assert_eq!(h.key(Key::Character(" ".into())), vec![Msg::Toggled(true)]);
    let on = Checkbox::new(&ui, "Notify", true).on_toggle(Msg::Toggled);
    let mut h = Harness::new(&ui, on.into_element(), 300.0, 100.0);
    assert_eq!(h.click(4.0, 4.0), vec![Msg::Toggled(false)]);
}

#[test]
fn slider_drag_and_keys() {
    let ui = ui();
    let mk = |ui: &Ui, v: f32| {
        Slider::new(ui, 0.0, 100.0, v)
            .step(1.0)
            .id(4)
            .on_change(Msg::Value)
            .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, 10.0), 216.0, 24.0);
    // track spans x in [8, 208]; click at the middle
    assert_eq!(h.mv(108.0, 12.0), vec![]);
    assert_eq!(
        h.send(Event::MousePressed(MouseButton::Left)),
        vec![Msg::Value(50.0)]
    );
    assert_eq!(h.mv(208.0, 12.0), vec![Msg::Value(100.0)]);
    assert_eq!(h.mv(58.0, 12.0), vec![Msg::Value(25.0)]);
    assert_eq!(
        h.mv(-50.0, 12.0),
        vec![Msg::Value(0.0)],
        "drag continues outside, clamped"
    );
    h.send(Event::MouseReleased(MouseButton::Left));
    assert!(h.mv(100.0, 12.0).is_empty(), "no drag after release");
    let mut h = Harness::new(&ui, mk(&ui, 10.0), 216.0, 24.0);
    assert_eq!(
        h.key(Key::Named("ArrowRight".into())),
        vec![Msg::Value(11.0)]
    );
    assert_eq!(h.key(Key::Named("End".into())), vec![Msg::Value(100.0)]);
}

fn typing(h: &mut Harness, s: &str) -> Vec<Msg> {
    h.send(Event::TextInput(s.into()))
}

#[test]
fn text_input_typing_selection_and_clipboard() {
    let ui = ui();
    let mk = |ui: &Ui, v: &str| {
        TextInput::new(ui, 7, v)
            .on_input(Msg::Text)
            .on_submit(|| Msg::Submit)
            .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, "hello"), 300.0, 40.0);
    h.click(290.0, 20.0); // click right of the text: caret at end
    assert_eq!(ui.focused(), Some(7));
    assert_eq!(typing(&mut h, "!"), vec![Msg::Text("hello!".into())]);
    // app accepted: rebuild with new value
    let mut h = Harness::new(&ui, mk(&ui, "hello!"), 300.0, 40.0);
    assert_eq!(h.key(Key::Backspace), vec![Msg::Text("hello".into())]);
    // select all + replace
    let m = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    h.send(Event::KeyPressed(Key::Character("a".into()), m));
    assert_eq!(typing(&mut h, "x"), vec![Msg::Text("x".into())]);
    // copy / paste
    let mut h = Harness::new(&ui, mk(&ui, "abc"), 300.0, 40.0);
    h.send(Event::KeyPressed(Key::Character("a".into()), m));
    h.send(Event::KeyPressed(Key::Character("c".into()), m));
    h.send(Event::KeyPressed(
        Key::Named("End".into()),
        Modifiers::default(),
    ));
    assert_eq!(
        h.send(Event::KeyPressed(Key::Character("v".into()), m)),
        vec![Msg::Text("abcabc".into())]
    );
    assert_eq!(h.key(Key::Enter), vec![Msg::Submit]);
}

#[test]
fn text_input_shift_arrows_select_and_cut() {
    let ui = ui();
    let mk = |ui: &Ui| {
        TextInput::new(ui, 7, "abcdef")
            .on_input(Msg::Text)
            .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui), 300.0, 40.0);
    h.click(290.0, 20.0);
    let shift = Modifiers {
        shift: true,
        ..Default::default()
    };
    for _ in 0..2 {
        h.send(Event::KeyPressed(Key::Named("ArrowLeft".into()), shift));
    }
    let cut = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    assert_eq!(
        h.send(Event::KeyPressed(Key::Character("x".into()), cut)),
        vec![Msg::Text("abcd".into())]
    );
    assert_eq!(ui.clipboard().get().as_deref(), Some("ef"));
}

#[test]
fn text_input_ignores_input_when_unfocused_and_readonly() {
    let ui = ui();
    let mut h = Harness::new(
        &ui,
        TextInput::new(&ui, 7, "a")
            .on_input(Msg::Text)
            .into_element(),
        300.0,
        40.0,
    );
    assert!(typing(&mut h, "z").is_empty(), "not focused");
    let mut ro = Harness::new(
        &ui,
        TextInput::<Msg>::new(&ui, 8, "a").into_element(),
        300.0,
        40.0,
    );
    ro.click(290.0, 20.0);
    assert!(typing(&mut ro, "z").is_empty(), "read-only");
}

#[test]
fn text_input_ime_preedit_is_drawn_not_committed() {
    let ui = ui();
    let mut h = Harness::new(
        &ui,
        TextInput::new(&ui, 7, "ab")
            .on_input(Msg::Text)
            .into_element(),
        300.0,
        40.0,
    );
    h.click(290.0, 20.0);
    assert!(h.send(Event::ImePreedit("にほ".into())).is_empty());
    assert!(h.texts().contains(&"にほ".to_string()));
    assert_eq!(typing(&mut h, "日本"), vec![Msg::Text("ab日本".into())]);
}

#[test]
fn text_input_placeholder_when_empty() {
    let ui = ui();
    let h = Harness::new(
        &ui,
        TextInput::<Msg>::new(&ui, 7, "")
            .placeholder("Search")
            .into_element(),
        300.0,
        40.0,
    );
    assert_eq!(h.texts(), vec!["Search".to_string()]);
}

fn tall_list(ui: &Ui) -> Element<Msg> {
    let rows = (0..50)
        .map(|i| Text::new(ui, format!("item {i}")).into_element())
        .collect();
    Scrollable::new(
        ui,
        9,
        Column::new(rows).width(Length::Fill(1)).into_element(),
    )
    .into_element()
}

#[test]
fn scrollable_scrolls_clamps_and_clips() {
    let ui = ui();
    let mut h = Harness::new(&ui, tall_list(&ui), 200.0, 100.0);
    let d = h.draw().0;
    assert!(matches!(d.first(), Some(DrawCommand::PushClip(_))));
    assert!(d.iter().any(|c| matches!(c, DrawCommand::PopClip)));
    h.mv(10.0, 10.0);
    h.send(Event::Scrolled { dx: 0.0, dy: -40.0 });
    assert_eq!(ui.scroll_offset(9), 40.0);
    h.send(Event::Scrolled {
        dx: 0.0,
        dy: 10_000.0,
    });
    assert_eq!(ui.scroll_offset(9), 0.0, "clamped at top");
    h.send(Event::Scrolled {
        dx: 0.0,
        dy: -1.0e6,
    });
    let lh = ui.theme().typography.body.line_height;
    assert_eq!(ui.scroll_offset(9), 50.0 * lh - 100.0, "clamped at bottom");
}

#[test]
fn scrollable_offsets_child_content() {
    let ui = ui();
    let mut h = Harness::new(&ui, tall_list(&ui), 200.0, 100.0);
    let first_y = |h: &Harness| {
        h.draw().0.iter().find_map(|c| match c {
            DrawCommand::Text { origin, .. } => Some(origin.y),
            _ => None,
        })
    };
    assert_eq!(first_y(&h), Some(0.0));
    h.mv(10.0, 10.0);
    h.send(Event::Scrolled { dx: 0.0, dy: -30.0 });
    // Smooth scrolling: the offset target moved, the view eases toward it.
    assert_eq!(ui.scroll_offset(9), 30.0);
    assert_eq!(first_y(&h), Some(0.0));
    assert!(h.animating());
    h.tick(1000);
    let mid = first_y(&h).unwrap();
    assert!((-30.0..=0.0).contains(&mid));
    h.tick(1000 + 1000);
    assert_eq!(first_y(&h), Some(-30.0));
    assert!(!h.animating(), "idle once settled");
}

#[test]
fn scroll_ignored_outside_viewport() {
    let ui = ui();
    let mut h = Harness::new(&ui, tall_list(&ui), 200.0, 100.0);
    h.mv(500.0, 500.0);
    h.send(Event::Scrolled { dx: 0.0, dy: -40.0 });
    assert_eq!(ui.scroll_offset(9), 0.0);
}

#[test]
fn list_select_by_click_and_keys() {
    let ui = ui();
    let mk = |ui: &Ui, sel: Option<usize>| {
        let items = (0..5)
            .map(|i| Text::new(ui, format!("row {i}")).into_element())
            .collect();
        List::new(ui, 11, items, sel, Msg::Picked).into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, None), 200.0, 300.0);
    let row_h = ui.theme().typography.body.line_height + 2.0 * ui.theme().spacing.xs;
    assert_eq!(h.click(20.0, row_h * 2.0 + 2.0), vec![Msg::Picked(2)]);
    assert_eq!(ui.focused(), Some(11));
    let mut h = Harness::new(&ui, mk(&ui, Some(2)), 200.0, 300.0);
    assert_eq!(h.key(Key::Named("ArrowDown".into())), vec![Msg::Picked(3)]);
    assert_eq!(h.key(Key::Named("ArrowUp".into())), vec![Msg::Picked(1)]);
    let mut h = Harness::new(&ui, mk(&ui, Some(4)), 200.0, 300.0);
    assert_eq!(
        h.key(Key::Named("ArrowDown".into())),
        vec![Msg::Picked(4)],
        "clamped at the end"
    );
}

#[test]
fn list_selected_row_is_highlighted() {
    let ui = ui();
    let items = (0..3)
        .map(|i| Text::new(&ui, format!("r{i}")).into_element())
        .collect();
    let h = Harness::new(
        &ui,
        List::new(&ui, 11, items, Some(1), Msg::Picked).into_element(),
        200.0,
        200.0,
    );
    let sel = ui.theme().palette.selection;
    let n = h
        .draw()
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Quad { fill, .. } if *fill == sel))
        .count();
    assert_eq!(n, 1);
}

#[test]
fn no_idle_work_without_events() {
    // Drawing twice yields identical lists: widgets hold no hidden animation state.
    let ui = ui();
    let h = Harness::new(
        &ui,
        Button::new(&ui, "A").on_press(Msg::Clicked).into_element(),
        100.0,
        50.0,
    );
    assert_eq!(h.draw(), h.draw());
}

#[test]
fn toggle_flips_on_click_and_space() {
    let ui = ui();
    let t = Toggle::new(&ui, "Dark mode", false)
        .id(5)
        .on_toggle(Msg::Toggled);
    let mut h = Harness::new(&ui, t.into_element(), 200.0, 40.0);
    assert_eq!(h.click(4.0, 4.0), vec![Msg::Toggled(true)]);
    assert_eq!(h.key(Key::Character(" ".into())), vec![Msg::Toggled(true)]);
}

#[test]
fn pick_list_opens_selects_and_closes() {
    let ui = ui();
    let mk = |ui: &Ui, sel| {
        PickList::new(ui, 12, vec!["a".into(), "b".into(), "c".into()], sel)
            .on_select(Msg::Picked)
            .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, Some(0)), 200.0, 300.0);
    assert!(h.click(5.0, 5.0).is_empty(), "opening emits nothing");
    assert_eq!(ui.popup(), Some(12));
    let field_h = h.layout.children[0].bounds.size.height;
    let row_h = ui.theme().typography.body.line_height + 2.0 * ui.theme().spacing.xs;
    // third option
    let y = field_h + 4.0 + 4.0 + row_h * 2.5;
    assert_eq!(h.click(5.0, y), vec![Msg::Picked(2)]);
    assert_eq!(ui.popup(), None);
    // click elsewhere closes without selecting
    h.click(5.0, 5.0);
    assert!(h.click(190.0, 290.0).is_empty());
    assert_eq!(ui.popup(), None);
    // keyboard
    let mut h = Harness::new(&ui, mk(&ui, Some(0)), 200.0, 300.0);
    h.click(5.0, 5.0);
    h.key(Key::Escape);
    assert_eq!(ui.popup(), None);
    ui.set_focus(Some(12));
    assert_eq!(h.key(Key::Named("ArrowDown".into())), vec![Msg::Picked(1)]);
}

#[test]
fn progress_bar_fraction_and_bad_input() {
    let ui = ui();
    let quad_widths = |v: f32| {
        let h = Harness::new(
            &ui,
            ProgressBar::new(&ui, 0.0, 10.0, v).into_element(),
            100.0,
            20.0,
        );
        h.draw()
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Quad { bounds, .. } => Some(bounds.size.width),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(quad_widths(5.0), vec![100.0, 50.0]);
    assert_eq!(quad_widths(-3.0), vec![100.0], "empty draws only the track");
    assert_eq!(quad_widths(f32::NAN), vec![100.0]);
    assert_eq!(quad_widths(99.0), vec![100.0, 100.0]);
}

#[test]
fn tooltip_waits_for_dwell_then_shows_and_hides_at_once() {
    let ui = ui();
    let tip = Tooltip::new(
        &ui,
        Button::new(&ui, "?").on_press(Msg::Clicked).into_element(),
        "Help",
    );
    let mut h = Harness::new(&ui, tip.into_element(), 200.0, 100.0);
    let shown = |h: &Harness| h.texts().contains(&"Help".to_string());
    assert!(!shown(&h));
    h.mv(5.0, 5.0);
    h.draw();
    h.tick(1000);
    assert!(!shown(&h), "not yet: the pointer has only just arrived");
    assert!(h.animating(), "the dwell timer needs ticks");
    h.tick(1600);
    assert!(shown(&h));
    h.mv(190.0, 90.0);
    assert!(!shown(&h), "hides immediately");
}

#[test]
fn image_emits_fixed_size_draw() {
    let ui = ui();
    let h = Harness::new(
        &ui,
        Image::new(7, Size::new(32.0, 24.0)).into_element(),
        200.0,
        100.0,
    );
    assert!(matches!(
        h.draw().0.as_slice(),
        [DrawCommand::Image { id: 7, bounds }] if bounds.size == Size::new(32.0, 24.0)
    ));
}

#[test]
fn layout_fn_matches_runtime_layouter_signature() {
    let ui = ui();
    let root: Element<Msg> = Root::new(&ui, Text::new(&ui, "hi").into_element()).into_element();
    let f: Layouter<Msg> = alloy_ui_widgets::layout::<Msg>;
    let l = f(&root, Size::new(300.0, 200.0));
    assert_eq!(l.bounds.size, Size::new(300.0, 200.0));
}

#[test]
fn idle_ui_does_not_animate() {
    let ui = ui();
    let col = Column::new(vec![
        Button::new(&ui, "A").on_press(Msg::Clicked).into_element(),
        Checkbox::new(&ui, "B", true)
            .on_toggle(Msg::Toggled)
            .into_element(),
        TextInput::new(&ui, 1, "x")
            .on_input(Msg::Text)
            .into_element(),
    ]);
    let mut h = Harness::new(&ui, col.into_element(), 300.0, 200.0);
    h.draw();
    h.tick(10);
    assert!(
        !h.animating(),
        "nothing focused or moving: zero ticks needed"
    );
}

#[test]
fn button_hover_fades_over_the_fast_duration() {
    let ui = ui();
    let mk = |ui: &Ui| {
        Harness::new(
            ui,
            Button::new(ui, "A")
                .variant(Variant::Primary)
                .on_press(Msg::Clicked)
                .into_element(),
            200.0,
            80.0,
        )
    };
    let mut h = mk(&ui);
    let fill = |h: &Harness| match &h.draw().0[0] {
        DrawCommand::Quad { fill, .. } => fill.r,
        c => panic!("{c:?}"),
    };
    let rest = fill(&h);
    h.mv(5.0, 5.0);
    assert_eq!(fill(&h), rest, "transition starts on the next tick");
    h.tick(100);
    let fast = ui.theme().motion.fast_ms as u64;
    h.tick(100 + fast / 2);
    let mid = fill(&h);
    h.tick(100 + fast);
    let end = fill(&h);
    assert_eq!(end, ui.theme().palette.accent_hover.r);
    assert!(
        (mid - rest).abs() > 0.0 && (end - mid).abs() > 0.0 || rest == end,
        "mid {mid} rest {rest} end {end}"
    );
    assert!(!h.animating());
}

#[test]
fn text_caret_blinks_only_while_focused() {
    let ui = ui();
    let mk = |ui: &Ui| {
        Harness::new(
            ui,
            TextInput::new(ui, 7, "ab")
                .on_input(Msg::Text)
                .into_element(),
            300.0,
            40.0,
        )
    };
    let mut h = mk(&ui);
    assert!(!h.animating());
    h.click(290.0, 20.0);
    assert!(h.animating(), "focused field needs ticks");
    let carets = |h: &Harness| {
        h.draw()
            .0
            .iter()
            .filter(|c| matches!(c, DrawCommand::Quad { bounds, .. } if bounds.size.width == 1.5))
            .count()
    };
    h.tick(0);
    assert_eq!(carets(&h), 1);
    h.tick(600);
    assert_eq!(carets(&h), 0, "off phase");
    h.tick(1100);
    assert_eq!(carets(&h), 1);
}

#[test]
fn overlay_click_reaches_pick_list_through_a_covering_sibling() {
    let ui = ui();
    let mk = |ui: &Ui| {
        Column::new(vec![
            PickList::new(ui, 12, vec!["a".into(), "b".into(), "c".into()], Some(0))
                .on_select(Msg::Picked)
                .into_element(),
            // A later sibling that would normally take clicks beneath the popup.
            Button::new(ui, "Underneath")
                .width(Length::Fill(1))
                .on_press(Msg::Clicked)
                .into_element(),
        ])
        .spacing(4.0)
        .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui), 300.0, 300.0);
    h.click(5.0, 5.0); // open
    h.draw(); // overlay regions are recorded while drawing
    let field_h = h.layout.children[0].children[0].bounds.size.height;
    let row_h = ui.theme().typography.body.line_height + 2.0 * ui.theme().spacing.xs;
    let y = field_h + 4.0 + 4.0 + row_h * 1.5;
    let msgs = h.click(5.0, y);
    assert_eq!(
        msgs,
        vec![Msg::Picked(1)],
        "button underneath must not fire"
    );
}

#[test]
fn popup_is_drawn_last() {
    let ui = ui();
    let col = Column::new(vec![
        PickList::new(&ui, 12, vec!["a".into(), "b".into()], Some(0))
            .on_select(Msg::Picked)
            .into_element(),
        Button::new(&ui, "Below")
            .on_press(Msg::Clicked)
            .into_element(),
    ]);
    let h = Harness::new(&ui, col.into_element(), 300.0, 300.0);
    ui.set_popup(Some(12));
    let list = h.draw().0;
    let last_text = list.iter().rev().find_map(|c| match c {
        DrawCommand::Text { content, .. } => Some(content.clone()),
        _ => None,
    });
    assert_eq!(
        last_text.as_deref(),
        Some("b"),
        "popup text comes after the sibling button"
    );
}

fn editor(ui: &Ui, v: &str) -> Element<Msg> {
    TextEditor::new(ui, 40, v)
        .on_input(Msg::Text)
        .into_element()
}

#[test]
fn editor_enter_inserts_newline_and_vertical_moves_keep_column() {
    let ui = ui();
    let mut h = Harness::new(&ui, editor(&ui, "abcd\nxy\nhello"), 300.0, 200.0);
    h.click(290.0, 8.0); // focus; caret at end of line 0 (right of the text)
    assert_eq!(ui.focused(), Some(40));
    assert_eq!(
        h.key(Key::Enter),
        vec![Msg::Text("abcd\n\nxy\nhello".into())]
    );
    // rebuild with the app's accepted value; caret is now at the start of line 1
    let mut h = Harness::new(&ui, editor(&ui, "abcd\n\nxy\nhello"), 300.0, 200.0);
    h.key(Key::Named("ArrowDown".into()));
    h.key(Key::Named("End".into())); // end of "xy" (col 2)
    h.key(Key::Named("ArrowDown".into())); // "hello": col 2
    assert_eq!(
        typing(&mut h, "!"),
        vec![Msg::Text("abcd\n\nxy\nhe!llo".into())]
    );
}

#[test]
fn editor_goal_column_survives_short_lines() {
    let ui = ui();
    let mut h = Harness::new(&ui, editor(&ui, "abcdef\nx\nabcdef"), 300.0, 200.0);
    h.click(290.0, 8.0);
    h.key(Key::Named("End".into())); // col 6
    h.key(Key::Named("ArrowDown".into())); // clamps to col 1
    h.key(Key::Named("ArrowDown".into())); // back to col 6
    assert_eq!(
        typing(&mut h, "!"),
        vec![Msg::Text("abcdef\nx\nabcdef!".into())]
    );
}

#[test]
fn editor_backspace_joins_lines_and_home_end_are_per_line() {
    let ui = ui();
    let mut h = Harness::new(&ui, editor(&ui, "ab\ncd"), 300.0, 200.0);
    h.click(290.0, 8.0);
    h.key(Key::Named("ArrowDown".into()));
    h.key(Key::Named("Home".into()));
    assert_eq!(h.key(Key::Backspace), vec![Msg::Text("abcd".into())]);
}

#[test]
fn editor_select_all_and_paste_keeps_newlines() {
    let ui = ui();
    ui.clipboard().set("one\ntwo".into());
    let mut h = Harness::new(&ui, editor(&ui, "zz"), 300.0, 200.0);
    h.click(290.0, 8.0);
    let m = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    h.send(Event::KeyPressed(Key::Character("a".into()), m));
    assert_eq!(
        h.send(Event::KeyPressed(Key::Character("v".into()), m)),
        vec![Msg::Text("one\ntwo".into())]
    );
}

#[test]
fn editor_scrolls_with_wheel_and_follows_caret() {
    let ui = ui();
    let text = (0..100)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut h = Harness::new(&ui, editor(&ui, &text), 300.0, 120.0);
    h.mv(10.0, 10.0);
    h.send(Event::Scrolled { dx: 0.0, dy: -50.0 });
    assert_eq!(ui.scroll_offset(40), 50.0);
    // Ctrl+End moves the caret to the last line; drawing scrolls it into view.
    h.click(10.0, 10.0);
    h.send(Event::KeyPressed(
        Key::Named("End".into()),
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    ));
    h.draw();
    let lh = ui.theme().typography.code.line_height;
    let max = 100.0 * lh - (120.0 - 2.0 * ui.theme().spacing.md);
    assert!(
        (ui.scroll_offset(40) - max).abs() < 0.01,
        "{} vs {max}",
        ui.scroll_offset(40)
    );
}

#[test]
fn space_pushes_siblings_apart_and_rule_is_one_pixel() {
    let ui = ui();
    let row = Row::new(vec![
        Text::new(&ui, "left").into_element(),
        Space::fill().into_element(),
        Text::new(&ui, "right").into_element(),
    ])
    .width(Length::Fill(1));
    let h = Harness::new(&ui, row.into_element(), 400.0, 40.0);
    let k = &h.layout.children[0].children;
    assert_eq!(k[2].bounds.origin.x + k[2].bounds.size.width, 400.0);
    let col = Column::new(vec![Rule::horizontal(&ui).into_element()]).width(Length::Fill(1));
    let h = Harness::new(&ui, col.into_element(), 400.0, 40.0);
    assert_eq!(
        h.layout.children[0].children[0].bounds.size,
        Size::new(400.0, 1.0)
    );
}

#[test]
fn radio_selects_once_and_ignores_already_selected() {
    let ui = ui();
    let mk = |ui: &Ui, sel| {
        Radio::new(ui, "Dark", sel)
            .id(50)
            .on_select(Msg::Clicked)
            .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, false), 200.0, 40.0);
    assert_eq!(h.click(4.0, 4.0), vec![Msg::Clicked]);
    assert_eq!(h.key(Key::Character(" ".into())), vec![Msg::Clicked]);
    let mut h = Harness::new(&ui, mk(&ui, true), 200.0, 40.0);
    assert!(h.click(4.0, 4.0).is_empty(), "already selected");
}

#[test]
fn segmented_click_and_arrow_keys() {
    let ui = ui();
    let mk = |ui: &Ui, sel| {
        Segmented::new(
            ui,
            60,
            vec!["Day".into(), "Week".into(), "Month".into()],
            sel,
        )
        .on_select(Msg::Picked)
        .into_element()
    };
    let mut h = Harness::new(&ui, mk(&ui, 0), 400.0, 60.0);
    let w = h.layout.children[0].bounds.size.width;
    assert_eq!(h.click(w - 6.0, 10.0), vec![Msg::Picked(2)], "last segment");
    assert!(h.click(6.0, 10.0).is_empty(), "already selected");
    let mut h = Harness::new(&ui, mk(&ui, 1), 400.0, 60.0);
    ui.set_focus(Some(60));
    assert_eq!(h.key(Key::Named("ArrowRight".into())), vec![Msg::Picked(2)]);
    assert_eq!(h.key(Key::Named("ArrowLeft".into())), vec![Msg::Picked(0)]);
}

#[test]
fn offscreen_list_rows_are_not_drawn() {
    let ui = ui();
    let items = (0..500)
        .map(|i| Text::new(&ui, format!("r{i}")).into_element())
        .collect();
    let h = Harness::new(
        &ui,
        List::new(&ui, 11, items, None, Msg::Picked).into_element(),
        200.0,
        100.0,
    );
    let n = h.draw().0.len();
    assert!(n < 40, "drew {n} commands for a 100px viewport");
}

#[test]
fn text_input_undo_redo_round_trip() {
    let ui = ui();
    let mk = |ui: &Ui, v: &str| TextInput::new(ui, 7, v).on_input(Msg::Text).into_element();
    let mut h = Harness::new(&ui, mk(&ui, "ab"), 300.0, 40.0);
    h.click(290.0, 20.0);
    assert_eq!(typing(&mut h, "c"), vec![Msg::Text("abc".into())]);
    let mut h = Harness::new(&ui, mk(&ui, "abc"), 300.0, 40.0);
    assert_eq!(h.key(Key::Backspace), vec![Msg::Text("ab".into())]);
    let ctrl = Modifiers {
        ctrl: true,
        ..Default::default()
    };
    let z = Key::Character("z".into());
    // app accepted "ab"; undo restores "abc", redo goes back
    let mut h = Harness::new(&ui, mk(&ui, "ab"), 300.0, 40.0);
    assert_eq!(
        h.send(Event::KeyPressed(z.clone(), ctrl)),
        vec![Msg::Text("abc".into())]
    );
    let mut h = Harness::new(&ui, mk(&ui, "abc"), 300.0, 40.0);
    let shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    assert_eq!(
        h.send(Event::KeyPressed(z, shift)),
        vec![Msg::Text("ab".into())]
    );
}

#[test]
fn editor_undo_restores_deleted_line() {
    let ui = ui();
    let mut h = Harness::new(&ui, editor(&ui, "ab\ncd"), 300.0, 200.0);
    h.click(290.0, 8.0);
    h.key(Key::Named("ArrowDown".into()));
    h.key(Key::Named("Home".into()));
    assert_eq!(h.key(Key::Backspace), vec![Msg::Text("abcd".into())]);
    let mut h = Harness::new(&ui, editor(&ui, "abcd"), 300.0, 200.0);
    let m = Modifiers {
        logo: true,
        ..Default::default()
    };
    assert_eq!(
        h.send(Event::KeyPressed(Key::Character("z".into()), m)),
        vec![Msg::Text("ab\ncd".into())]
    );
}
