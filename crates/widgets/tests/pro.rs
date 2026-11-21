//! Behaviour and golden tests for the "pro" widgets: split pane, context menu, virtual list,
//! markdown, tabs and popover. Goldens drop text (glyphs differ per OS); regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p alloy-ui-widgets --test pro`.

use alloy_ui_core::*;
use alloy_ui_layout::Length;
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::{cell::Cell, path::Path, rc::Rc};

const W: u32 = 560;
const H: u32 = 340;

#[derive(Debug, Clone, PartialEq)]
enum Msg {
    Pick(usize),
    Close(usize),
    Toggle(bool),
}

/// A mounted tree: builds once, then takes events like the runtime does.
struct Scene {
    ui: Ui,
    root: Element<Msg>,
    layout: Layout,
    cursor: Point,
    msgs: Vec<Msg>,
}

impl Scene {
    fn new(theme: Theme, build: impl Fn(&Ui) -> Element<Msg>) -> Self {
        let ui = Ui::new(theme);
        let root = Root::new(&ui, build(&ui)).into_element();
        let layout = alloy_ui_widgets::layout(&root, Size::new(W as f32, H as f32));
        let mut s = Self {
            ui,
            root,
            layout,
            cursor: Point::new(-1.0, -1.0),
            msgs: Vec::new(),
        };
        s.draw();
        s
    }
    fn draw(&mut self) -> DrawList {
        self.layout = alloy_ui_widgets::layout(&self.root, Size::new(W as f32, H as f32));
        let mut list = DrawList::default();
        self.root.0.draw(&self.layout, &mut list);
        list
    }
    fn send(&mut self, e: Event) {
        if let Event::CursorMoved(p) = &e {
            self.cursor = *p;
        }
        self.root
            .0
            .on_event(&e, &self.layout, self.cursor, &mut self.msgs);
        self.draw();
    }
    fn mv(&mut self, x: f32, y: f32) {
        self.send(Event::CursorMoved(Point::new(x, y)));
    }
    fn click(&mut self, x: f32, y: f32) {
        self.mv(x, y);
        self.send(Event::MousePressed(MouseButton::Left));
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    fn right_click(&mut self, x: f32, y: f32) {
        self.mv(x, y);
        self.send(Event::MousePressed(MouseButton::Right));
        self.send(Event::MouseReleased(MouseButton::Right));
    }
    fn key(&mut self, k: Key) {
        self.send(Event::KeyPressed(k, Modifiers::default()));
    }
    fn named(&mut self, n: &str) {
        self.key(Key::Named(n.into()));
    }
    fn image(&mut self, bg: Color) -> alloy_ui_render::Image {
        let mut list = self.draw();
        list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
        let mut r = CpuRenderer::new(W * 2, H * 2);
        r.set_background(bg);
        r.render_to_image(&list, W * 2, H * 2, 2.0)
    }
}

fn check(name: &str, img: &alloy_ui_render::Image) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    assert_golden(&dir, name, img, Tolerance::CPU);
}

fn panel(ui: &Ui, label: &str) -> Element<Msg> {
    Container::new(Text::new(ui, label).into_element())
        .padding(alloy_ui_core::style::Padding::all(ui.theme().spacing.lg))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .into_element()
}

// ------------------------------------------------------------------ split pane

fn split_scene(ui: &Ui) -> Element<Msg> {
    SplitPane::new(ui, 1, panel(ui, "Files"), panel(ui, "Editor"))
        .ratio(0.3)
        .min(100.0)
        .into_element()
}

#[test]
fn split_starts_at_ratio_and_drags() {
    let mut s = Scene::new(Theme::light(), split_scene);
    let first = |s: &Scene| s.layout.children[0].children[0].bounds;
    let w0 = first(&s).size.width;
    let total = W as f32 - 9.0;
    assert!((w0 - total * 0.3).abs() < 2.0, "starts near 30%: {w0}");
    // Grab the gutter and drag right.
    let gx = first(&s).size.width + 4.0;
    s.mv(gx, 100.0);
    s.send(Event::MousePressed(MouseButton::Left));
    s.mv(gx + 150.0, 100.0);
    s.send(Event::MouseReleased(MouseButton::Left));
    let w1 = first(&s).size.width;
    assert!(
        (w1 - (w0 + 150.0)).abs() < 3.0,
        "dragged by 150: {w0} -> {w1}"
    );
    // Dragging far past the end stops at the minimum for the other pane.
    s.mv(w1 + 4.0, 100.0);
    s.send(Event::MousePressed(MouseButton::Left));
    s.mv(10_000.0, 100.0);
    s.send(Event::MouseReleased(MouseButton::Left));
    let second = s.layout.children[0].children[1].bounds.size.width;
    assert!(second >= 99.0, "second pane keeps its minimum: {second}");
    // And the other way.
    s.mv(first(&s).size.width + 4.0, 100.0);
    s.send(Event::MousePressed(MouseButton::Left));
    s.mv(-500.0, 100.0);
    s.send(Event::MouseReleased(MouseButton::Left));
    assert!(first(&s).size.width >= 99.0);
}

#[test]
fn split_keyboard_resizes() {
    let mut s = Scene::new(Theme::light(), split_scene);
    s.ui.set_focus(Some(1));
    let w0 = s.layout.children[0].children[0].bounds.size.width;
    s.named("ArrowRight");
    let w1 = s.layout.children[0].children[0].bounds.size.width;
    assert!((w1 - w0 - 24.0).abs() < 2.0);
    s.named("Home");
    assert!(s.layout.children[0].children[0].bounds.size.width >= 99.0);
}

#[test]
fn split_golden_horizontal_and_vertical() {
    let mut s = Scene::new(Theme::light(), split_scene);
    s.mv(W as f32 * 0.3 + 4.0, 80.0);
    let bg = Theme::light().palette.background;
    check("pro_split_hover", &s.image(bg));
    let mut v = Scene::new(Theme::dark(), |ui| {
        SplitPane::new(ui, 2, panel(ui, "Top"), panel(ui, "Bottom"))
            .vertical()
            .ratio(0.6)
            .into_element()
    });
    v.ui.set_focus(Some(2));
    check(
        "pro_split_vertical_dark_focus",
        &v.image(Theme::dark().palette.background),
    );
}

// ------------------------------------------------------------------ context menu

fn menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Cut").shortcut("Ctrl+X"),
        MenuItem::new("Copy").shortcut("Ctrl+C"),
        MenuItem::new("Paste").shortcut("Ctrl+V").disabled(true),
        MenuItem::separator(),
        MenuItem::new("Delete").danger(),
    ]
}

fn menu_scene(ui: &Ui) -> Element<Msg> {
    ContextMenu::new(ui, 7, panel(ui, "Right-click me"), menu_items())
        .on_select(Msg::Pick)
        .into_element()
}

#[test]
fn context_menu_right_click_keyboard_and_pick() {
    let mut s = Scene::new(Theme::light(), menu_scene);
    assert_eq!(s.ui.popup(), None);
    s.right_click(60.0, 60.0);
    assert_eq!(s.ui.popup(), Some(7), "right click opens");
    // Down skips nothing at first (Cut), Down again is Copy, Down skips disabled Paste and the rule.
    s.named("ArrowDown");
    s.named("ArrowDown");
    s.named("ArrowDown");
    s.key(Key::Enter);
    assert_eq!(
        s.msgs,
        vec![Msg::Pick(4)],
        "disabled and separator rows are skipped"
    );
    assert_eq!(s.ui.popup(), None);

    // Mouse pick: click the "Copy" row (second row).
    s.msgs.clear();
    s.right_click(60.0, 60.0);
    s.click(60.0 + 40.0, 60.0 + 4.0 + 28.0 + 14.0);
    assert_eq!(s.msgs, vec![Msg::Pick(1)]);

    // Escape closes without a message; a click elsewhere closes and is swallowed.
    s.msgs.clear();
    s.right_click(200.0, 100.0);
    s.key(Key::Escape);
    assert_eq!(s.ui.popup(), None);
    s.right_click(200.0, 100.0);
    s.click(500.0, 300.0);
    assert_eq!(s.ui.popup(), None);
    assert!(s.msgs.is_empty());

    // The menu key opens it under the pointer.
    s.mv(120.0, 120.0);
    s.named("ContextMenu");
    assert_eq!(s.ui.popup(), Some(7));
}

#[test]
fn context_menu_stays_in_window() {
    let mut s = Scene::new(Theme::light(), menu_scene);
    s.right_click(W as f32 - 30.0, H as f32 - 30.0);
    let list = s.draw();
    let panel = list
        .0
        .iter()
        .rev()
        .find_map(|c| match c {
            DrawCommand::Quad {
                bounds,
                shadow: Some(_),
                ..
            } => Some(*bounds),
            _ => None,
        })
        .expect("menu panel");
    assert!(panel.origin.x + panel.size.width <= W as f32);
    assert!(panel.origin.y + panel.size.height <= H as f32);
    assert!(panel.origin.x >= 0.0 && panel.origin.y >= 0.0);
}

#[test]
fn context_menu_golden() {
    let mut s = Scene::new(Theme::light(), menu_scene);
    s.right_click(80.0, 60.0);
    s.named("ArrowDown");
    s.named("ArrowDown");
    check(
        "pro_context_menu_open",
        &s.image(Theme::light().palette.background),
    );
}

// ------------------------------------------------------------------ virtual list

#[test]
fn virtual_list_builds_only_visible_rows() {
    let built = Rc::new(Cell::new(0usize));
    let b = built.clone();
    let mut s = Scene::new(Theme::light(), move |ui| {
        let b = b.clone();
        VirtualList::new(ui, 3, 100_000, move |i| {
            b.set(b.get() + 1);
            Text::new(&Ui::new(Theme::light()), format!("Row {i}")).into_element()
        })
        .row_height(28.0)
        .selected(Some(5))
        .on_select(Msg::Pick)
        .into_element()
    });
    built.set(0);
    s.draw();
    let n = built.get();
    assert!(
        n <= (H as f32 / 28.0).ceil() as usize + 2,
        "built {n} rows for 100k"
    );
    // Scroll far: still only a screenful.
    s.mv(100.0, 100.0);
    s.send(Event::Scrolled {
        dx: 0.0,
        dy: -28.0 * 50_000.0,
    });
    built.set(0);
    s.draw();
    assert!(
        built.get() <= 16,
        "built {} rows after scrolling",
        built.get()
    );
    assert!(s.ui.scroll_offset(3) > 1_000_000.0);
    // Clicking selects the row under the pointer, by absolute index.
    s.msgs.clear();
    s.click(100.0, 14.0);
    let expect = (s.ui.scroll_offset(3) / 28.0).floor() as usize;
    assert_eq!(s.msgs, vec![Msg::Pick(expect)]);
}

#[test]
fn virtual_list_keyboard_and_scrollbar() {
    let mut s = Scene::new(Theme::light(), |ui| {
        VirtualList::new(ui, 3, 1000, |_| Space::fill().into_element())
            .row_height(30.0)
            .selected(Some(0))
            .on_select(Msg::Pick)
            .into_element()
    });
    s.ui.set_focus(Some(3));
    s.named("PageDown");
    assert_eq!(
        s.msgs.pop(),
        Some(Msg::Pick(10)),
        "a page is rows-in-view minus one"
    );
    s.named("End");
    assert_eq!(s.msgs.pop(), Some(Msg::Pick(999)));
    assert!(
        s.ui.scroll_offset(3) > 29_000.0,
        "End scrolls the row into view"
    );
    // Dragging the thumb to the top scrolls to the top.
    s.mv(W as f32 - 6.0, H as f32 - 20.0);
    s.send(Event::MousePressed(MouseButton::Left));
    s.mv(W as f32 - 6.0, -50.0);
    s.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(s.ui.scroll_offset(3), 0.0);
}

#[test]
fn virtual_list_golden() {
    let mut s = Scene::new(Theme::light(), |ui| {
        VirtualList::new(ui, 3, 100_000, |i| {
            Row::new(vec![Text::new(
                &Ui::new(Theme::light()),
                format!("Row {i}"),
            )
            .into_element()])
            .into_element()
        })
        .row_height(32.0)
        .ruled()
        .selected(Some(4))
        .into_element()
    });
    s.ui.set_scroll_offset(3, 32.0 * 41_990.0);
    s.mv(200.0, 90.0);
    check(
        "pro_virtual_list_100k",
        &s.image(Theme::light().palette.background),
    );
}

// ------------------------------------------------------------------ tabs

fn tab_labels(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("document-{i}.rs")).collect()
}

#[test]
fn tabs_select_close_and_overflow() {
    let mut s = Scene::new(Theme::light(), |ui| {
        Tabs::new(ui, 9, tab_labels(14), 11)
            .on_select(Msg::Pick)
            .on_close(Msg::Close)
            .into_element()
    });
    // Overflow keeps the selected tab visible: Pick the last visible tab and an overflow entry.
    let list = s.draw();
    let quads = list
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Quad { .. }))
        .count();
    assert!(quads > 0);
    // Open overflow (right end of the strip), then choose the first hidden tab.
    s.click(W as f32 - 20.0, 18.0);
    assert_eq!(s.ui.popup(), Some(9), "overflow button opens the menu");
    s.click(W as f32 - 60.0, 38.0 + 4.0 + 14.0);
    assert!(
        matches!(s.msgs.last(), Some(Msg::Pick(i)) if *i < 14),
        "{:?}",
        s.msgs
    );
    assert_eq!(s.ui.popup(), None);
    // Keyboard.
    s.msgs.clear();
    s.ui.set_focus(Some(9));
    s.named("ArrowLeft");
    assert_eq!(s.msgs, vec![Msg::Pick(10)]);
}

#[test]
fn tabs_all_fit_has_no_overflow_button() {
    let mut s = Scene::new(Theme::light(), |ui| {
        Tabs::new(ui, 9, tab_labels(3), 0)
            .on_select(Msg::Pick)
            .into_element()
    });
    s.click(W as f32 - 20.0, 18.0);
    assert_eq!(s.ui.popup(), None);
    s.click(30.0, 18.0);
    assert_eq!(s.msgs, vec![Msg::Pick(0)]);
}

#[test]
fn tabs_golden_overflow_open() {
    let mut s = Scene::new(Theme::light(), |ui| {
        Column::new(vec![
            Tabs::new(ui, 9, tab_labels(14), 6)
                .on_select(Msg::Pick)
                .on_close(Msg::Close)
                .into_element(),
            panel(ui, "content"),
        ])
        .into_element()
    });
    s.mv(120.0, 18.0);
    s.click(W as f32 - 20.0, 18.0);
    check(
        "pro_tabs_overflow_open",
        &s.image(Theme::light().palette.background),
    );
}

// ------------------------------------------------------------------ popover

fn popover_scene(open: bool) -> impl Fn(&Ui) -> Element<Msg> {
    move |ui| {
        Container::new(
            Popover::new(
                ui,
                11,
                Button::new(ui, "Filters")
                    .on_press(Msg::Pick(0))
                    .into_element(),
                Column::new(vec![
                    Text::new(ui, "Show archived").into_element(),
                    Checkbox::new(ui, "Only mine", true)
                        .on_toggle(|_| Msg::Pick(1))
                        .id(12)
                        .into_element(),
                    Button::new(ui, "Apply")
                        .variant(Variant::Primary)
                        .on_press(Msg::Pick(2))
                        .into_element(),
                ])
                .spacing(8.0)
                .into_element(),
            )
            .open(open)
            .on_toggle(Msg::Toggle)
            .into_element(),
        )
        .padding(alloy_ui_core::style::Padding::all(24.0))
        .width(Length::Fill(1))
        .height(Length::Fill(1))
        .into_element()
    }
}

#[test]
fn popover_toggles_and_routes_clicks() {
    let mut s = Scene::new(Theme::light(), popover_scene(false));
    s.click(60.0, 60.0);
    assert_eq!(s.msgs, vec![Msg::Toggle(true)], "anchor click asks to open");
    let mut s = Scene::new(Theme::light(), popover_scene(true));
    // Escape and outside clicks ask to close.
    s.key(Key::Escape);
    assert_eq!(s.msgs, vec![Msg::Toggle(false)]);
    s.msgs.clear();
    s.click(500.0, 300.0);
    assert_eq!(s.msgs, vec![Msg::Toggle(false)]);
    // The Apply button inside the panel is clickable (the panel sits under the anchor).
    s.msgs.clear();
    let list = s.draw();
    let apply = list
        .0
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Quad { bounds, fill, .. }
                if fill.a > 0.9 && bounds.size.width > 40.0 && bounds.origin.y > 70.0 =>
            {
                Some(*bounds)
            }
            _ => None,
        })
        .next_back()
        .expect("apply button quad");
    s.click(apply.origin.x + 10.0, apply.origin.y + 10.0);
    assert!(s.msgs.contains(&Msg::Pick(2)), "{:?}", s.msgs);
    assert!(
        !s.msgs.contains(&Msg::Toggle(false)),
        "a click inside does not dismiss"
    );
}

#[test]
fn popover_golden() {
    let mut s = Scene::new(Theme::dark(), popover_scene(true));
    check(
        "pro_popover_open_dark",
        &s.image(Theme::dark().palette.background),
    );
}

// ------------------------------------------------------------------ markdown

const DOC: &str = "# Release notes\n\nAlloy **0.2** adds a *split pane*, a `ContextMenu` and this view. See [the guide](guide).\n\n## Highlights\n\n- Split panes you can drag\n- 100k-row lists\n  - nested item\n1. First\n2. Second\n\n> Native means every pixel is ours.\n\n```\nlet ui = Ui::new(theme);\n```\n\n---\n\nDone.";

#[test]
fn markdown_has_definite_height_and_links_click() {
    let mut s = Scene::new(Theme::light(), |ui| {
        Markdown::new(ui, DOC)
            .width(420.0)
            .on_link(|u| Msg::Pick(u.len()))
            .into_element()
    });
    let h = s.layout.children[0].bounds.size.height;
    assert!(h > 300.0 && h < 900.0, "height {h}");
    // Find the link word in the draw output and click it.
    let list = s.draw();
    let (x, y) = list
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text {
                content, origin, ..
            } if content == "guide" => Some((origin.x, origin.y)),
            _ => None,
        })
        .expect("link text drawn");
    s.click(x + 4.0, y + 6.0);
    assert_eq!(s.msgs, vec![Msg::Pick(5)]);
}

#[test]
fn markdown_golden() {
    let mut s = Scene::new(Theme::dark(), |ui| {
        Container::new(Markdown::<Msg>::new(ui, DOC).width(480.0).into_element())
            .padding(alloy_ui_core::style::Padding::all(20.0))
            .into_element()
    });
    check(
        "pro_markdown_dark",
        &s.image(Theme::dark().palette.background),
    );
}
