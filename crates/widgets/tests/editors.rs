#![allow(clippy::ptr_arg)]
//! Behaviour and golden tests for the data grid, code editor and rich-text editor.
//! Goldens drop text (glyphs differ per OS); regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p alloy-ui-widgets --test editors`.

use alloy_ui_core::*;
use alloy_ui_render::testing::{assert_golden, Tolerance};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer};
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::path::Path;

const W: u32 = 640;
const H: u32 = 360;

/// A tiny Elm loop: `view` builds from state, `update` folds messages in, and the tree is
/// rebuilt after every event, like the runtime does.
struct App<S, M> {
    ui: Ui,
    state: S,
    view: fn(&Ui, &S) -> Element<M>,
    update: fn(&mut S, M),
    root: Element<M>,
    layout: Layout,
    cursor: Point,
    log: Vec<M>,
}

impl<S, M: Clone + 'static> App<S, M> {
    fn new(theme: Theme, state: S, view: fn(&Ui, &S) -> Element<M>, update: fn(&mut S, M)) -> Self {
        let ui = Ui::new(theme);
        let root = Root::new(&ui, view(&ui, &state)).into_element();
        let layout = alloy_ui_widgets::layout(&root, Size::new(W as f32, H as f32));
        let mut a = Self {
            ui,
            state,
            view,
            update,
            root,
            layout,
            cursor: Point::new(-1.0, -1.0),
            log: Vec::new(),
        };
        a.draw();
        a
    }
    fn rebuild(&mut self) {
        self.root = Root::new(&self.ui, (self.view)(&self.ui, &self.state)).into_element();
        self.layout = alloy_ui_widgets::layout(&self.root, Size::new(W as f32, H as f32));
    }
    fn draw(&mut self) -> DrawList {
        self.rebuild();
        let mut list = DrawList::default();
        self.root.0.draw(&self.layout, &mut list);
        list
    }
    fn send(&mut self, e: Event) {
        if let Event::CursorMoved(p) = &e {
            self.cursor = *p;
        }
        let mut out = Vec::new();
        self.root
            .0
            .on_event(&e, &self.layout, self.cursor, &mut out);
        for m in out {
            self.log.push(m.clone());
            (self.update)(&mut self.state, m);
        }
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
    fn double_click(&mut self, x: f32, y: f32) {
        self.click(x, y);
        self.send(Event::MousePressed(MouseButton::Left));
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    fn key_mod(&mut self, k: Key, m: Modifiers) {
        self.send(Event::KeyPressed(k, m));
    }
    fn key(&mut self, k: Key) {
        self.key_mod(k, Modifiers::default());
    }
    fn named(&mut self, n: &str) {
        self.key(Key::Named(n.into()));
    }
    fn chord(&mut self, c: &str) {
        let m = Modifiers {
            ctrl: !cfg!(target_os = "macos"),
            logo: cfg!(target_os = "macos"),
            ..Default::default()
        };
        self.key_mod(Key::Character(c.into()), m);
    }
    fn type_text(&mut self, s: &str) {
        for ch in s.chars() {
            match ch {
                '\n' => self.key(Key::Enter),
                '\t' => self.key(Key::Tab),
                c => self.send(Event::TextInput(c.to_string())),
            }
        }
    }
    fn image(&mut self, bg: Color) -> alloy_ui_render::Image {
        let mut list = self.draw();
        list.0.retain(|c| !matches!(c, DrawCommand::Text { .. }));
        let mut r = CpuRenderer::new(W * 2, H * 2);
        r.set_background(bg);
        r.render_to_image(&list, W * 2, H * 2, 2.0)
    }
    fn texts(&mut self) -> Vec<String> {
        self.draw()
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect()
    }
}

fn check(name: &str, img: &alloy_ui_render::Image) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    assert_golden(&dir, name, img, Tolerance::CPU);
}

// ------------------------------------------------------------------ code editor

#[derive(Clone, Debug, PartialEq)]
enum CMsg {
    Text(String),
}
fn code_view(ui: &Ui, s: &String) -> Element<CMsg> {
    CodeEditor::new(ui, 1, s.clone())
        .lang(Lang::Rust)
        .on_input(CMsg::Text)
        .into_element()
}
fn code_update(s: &mut String, m: CMsg) {
    let CMsg::Text(t) = m;
    *s = t;
}
fn code(initial: &str) -> App<String, CMsg> {
    let mut a = App::new(Theme::light(), initial.to_string(), code_view, code_update);
    a.click(200.0, 100.0); // focus
    a.chord("a");
    a
}

#[test]
fn code_typing_replaces_selection_and_pairs_brackets() {
    let mut a = code("");
    a.type_text("fn main(");
    assert_eq!(a.state, "fn main()", "an opening bracket closes itself");
    a.type_text(")");
    assert_eq!(a.state, "fn main()", "typing the closer steps over it");
    a.type_text(" {");
    assert_eq!(a.state, "fn main() {}");
}

#[test]
fn code_enter_keeps_indent_and_opens_blocks() {
    let mut a = code("");
    a.type_text("fn f() {\nlet x = 1;\nif x > 0 {\nx\n");
    assert_eq!(
        a.state,
        "fn f() {\n    let x = 1;\n    if x > 0 {\n        x\n        \n    }\n}"
    );
}

#[test]
fn code_tab_indents_and_shift_tab_dedents_selection() {
    let mut a = code("a\nb\nc");
    a.chord("a");
    a.key(Key::Tab);
    assert_eq!(a.state, "    a\n    b\n    c");
    a.key_mod(
        Key::Tab,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(a.state, "a\nb\nc");
}

#[test]
fn code_comment_toggle_round_trips() {
    let mut a = code("fn a() {\n    1\n}");
    a.chord("a");
    a.chord("/");
    assert_eq!(a.state, "// fn a() {\n//     1\n// }");
    a.chord("/");
    assert_eq!(a.state, "fn a() {\n    1\n}");
}

#[test]
fn code_backspace_removes_empty_pair_and_undo_restores() {
    let mut a = code("");
    a.type_text("(");
    assert_eq!(a.state, "()");
    a.key(Key::Backspace);
    assert_eq!(a.state, "");
    a.chord("z");
    assert_eq!(a.state, "()");
}

#[test]
fn code_find_steps_through_matches_and_closes() {
    let mut a = code("let a = 1;\nlet b = a;\nlet c = a + a;");
    a.chord("f");
    a.type_text("a");
    let t = a.texts();
    assert!(t.iter().any(|s| s == "1 of 4"), "{t:?}");
    a.key(Key::Enter);
    assert!(a.texts().iter().any(|s| s == "2 of 4"));
    a.key_mod(
        Key::Enter,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    a.key_mod(
        Key::Enter,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert!(a.texts().iter().any(|s| s == "4 of 4"), "wraps backwards");
    a.type_text("zzz");
    assert!(a.texts().iter().any(|s| s == "No results"));
    a.key(Key::Escape);
    assert!(!a.texts().iter().any(|s| s == "No results"), "bar closed");
}

#[test]
fn code_duplicate_line_and_home_end() {
    let mut a = code("one\ntwo");
    a.click(300.0, 60.0);
    a.chord("d");
    assert_eq!(a.state.lines().count(), 3);
    assert!(a.state.contains("one\none") || a.state.contains("two\ntwo"));
}

#[test]
fn code_large_file_draws_only_what_is_visible() {
    let big: String = (0..100_000).map(|i| format!("let v{i} = {i};\n")).collect();
    let mut a = App::new(Theme::light(), big, code_view, code_update);
    let t0 = std::time::Instant::now();
    let list = a.draw();
    let texts = list
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { .. }))
        .count();
    assert!(texts < 600, "{texts} text runs for a screenful");
    a.click(200.0, 100.0);
    a.chord("End".to_lowercase().as_str());
    for _ in 0..5 {
        a.draw();
    }
    eprintln!("100k-line draws: {:?} for 6 frames", t0.elapsed());
}

const SAMPLE: &str = "use std::collections::HashMap;\n\n/// A tiny cache.\n#[derive(Debug, Default)]\npub struct Cache<'a> {\n    hits: u64,\n    items: HashMap<&'a str, f64>,\n}\n\nimpl<'a> Cache<'a> {\n    pub fn get(&mut self, key: &str) -> Option<f64> {\n        // Count every lookup.\n        self.hits += 1;\n        let n = 42;\n        println!(\"{key} -> {}\", n);\n        self.items.get(key).copied()\n    }\n}\n";

#[test]
fn code_editor_goldens() {
    for (name, theme) in [("code_light", Theme::light()), ("code_dark", Theme::dark())] {
        let bg = theme.palette.background;
        let mut a = App::new(theme, SAMPLE.to_string(), code_view, code_update);
        a.click(300.0, 140.0);
        a.chord("f");
        a.type_text("get");
        check(name, &a.image(bg));
    }
}

// ------------------------------------------------------------------ data grid

use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
enum GMsg {
    Sort(usize, bool),
    Edit(usize, usize, String),
    Select(Vec<usize>),
}
struct GState {
    rows: Rc<Vec<[String; 3]>>,
    sort: Option<(usize, bool)>,
    selected: usize,
}
fn grid_data(n: usize) -> Rc<Vec<[String; 3]>> {
    Rc::new(
        (0..n)
            .map(|i| {
                [
                    format!("Item {i}"),
                    format!("{}", (i * 37) % 1000),
                    if i % 3 == 0 {
                        "Up".into()
                    } else {
                        "Down".into()
                    },
                ]
            })
            .collect(),
    )
}
fn grid_view(ui: &Ui, s: &GState) -> Element<GMsg> {
    let rows = s.rows.clone();
    DataGrid::new(
        ui,
        2,
        vec![
            GridColumn::new("Name", 160.0).editable(),
            GridColumn::new("Score", 100.0).right().editable(),
            GridColumn::new("Status", 120.0).flex(),
        ],
        rows.len(),
        move |r, c| {
            if c == 2 {
                let up = rows[r][2] == "Up";
                GridCell::Status(
                    if up { Tone::Success } else { Tone::Danger },
                    rows[r][2].clone(),
                )
            } else {
                GridCell::Text(rows[r][c].clone())
            }
        },
    )
    .sorted(s.sort)
    .on_sort(GMsg::Sort)
    .on_edit(GMsg::Edit)
    .on_select(|sel| GMsg::Select(sel.rows().take(1000).collect()))
    .into_element()
}
fn grid_update(s: &mut GState, m: GMsg) {
    match m {
        GMsg::Sort(c, asc) => s.sort = Some((c, asc)),
        GMsg::Edit(r, c, t) => {
            let mut v = (*s.rows).clone();
            v[r][c] = t;
            s.rows = Rc::new(v);
        }
        GMsg::Select(v) => s.selected = v.len(),
    }
}
fn grid(n: usize) -> App<GState, GMsg> {
    App::new(
        Theme::light(),
        GState {
            rows: grid_data(n),
            sort: None,
            selected: 0,
        },
        grid_view,
        grid_update,
    )
}
const BODY_Y: f32 = 34.0 + 18.0; // header + half a row
fn row_y(r: usize) -> f32 {
    BODY_Y + r as f32 * 36.0 - 18.0 + 18.0
}
fn mods(a: &mut App<GState, GMsg>, m: Modifiers) {
    a.send(Event::KeyReleased(Key::Named("Modifiers".into()), m));
}

#[test]
fn grid_100k_rows_ask_for_visible_cells_only() {
    let calls = Rc::new(std::cell::Cell::new(0usize));
    let c2 = calls.clone();
    let ui = Ui::new(Theme::light());
    let el = DataGrid::<GMsg>::new(
        &ui,
        1,
        vec![GridColumn::new("A", 100.0), GridColumn::new("B", 100.0)],
        100_000,
        move |r, c| {
            c2.set(c2.get() + 1);
            format!("{r}:{c}")
        },
    )
    .into_element();
    let root = Root::new(&ui, el).into_element();
    let l = alloy_ui_widgets::layout(&root, Size::new(W as f32, H as f32));
    let mut list = DrawList::default();
    root.0.draw(&l, &mut list);
    assert!(
        calls.get() <= 2 * 12,
        "{} cell calls for one screen of 100k rows",
        calls.get()
    );
}

#[test]
fn grid_click_shift_ctrl_select_rows() {
    let mut a = grid(1000);
    a.click(60.0, row_y(2));
    assert_eq!(a.log.last(), Some(&GMsg::Select(vec![2])));
    mods(
        &mut a,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    a.click(60.0, row_y(5));
    assert_eq!(a.log.last(), Some(&GMsg::Select(vec![2, 3, 4, 5])));
    mods(
        &mut a,
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    a.click(60.0, row_y(3));
    assert_eq!(a.log.last(), Some(&GMsg::Select(vec![2, 4, 5])));
    mods(&mut a, Modifiers::default());
}

#[test]
fn grid_keyboard_moves_selects_and_copies() {
    let mut a = grid(1000);
    a.click(60.0, row_y(0));
    a.named("ArrowDown");
    a.named("ArrowDown");
    assert_eq!(a.log.last(), Some(&GMsg::Select(vec![2])));
    a.key_mod(
        Key::Named("ArrowDown".into()),
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(a.log.last(), Some(&GMsg::Select(vec![2, 3])));
    a.chord("c");
    assert_eq!(
        a.ui.clipboard().get().unwrap(),
        "Item 2\t74\tDown\nItem 3\t111\tUp"
    );
    a.chord("a");
    assert_eq!(a.state.selected, 1000);
    a.named("End");
    assert_eq!(
        a.state.selected, 1,
        "End jumps to the last column of the row, selecting just that row"
    );
}

#[test]
fn grid_header_click_sorts_and_flips() {
    let mut a = grid(100);
    a.click(60.0, 17.0);
    assert_eq!(a.log.last(), Some(&GMsg::Sort(0, true)));
    a.click(60.0, 17.0);
    assert_eq!(a.log.last(), Some(&GMsg::Sort(0, false)));
    a.click(200.0, 17.0);
    assert_eq!(a.log.last(), Some(&GMsg::Sort(1, true)));
}

fn col_x(a: &mut App<GState, GMsg>, text: &str) -> f32 {
    a.draw()
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text {
                content, origin, ..
            } if content == text => Some(origin.x),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text {text}"))
}

#[test]
fn grid_resize_by_dragging_header_edge_and_autofit() {
    let mut a = grid(100);
    let x0 = col_x(&mut a, "Status");
    // Drag the Name column's right edge (x = 160) 60 px right.
    a.mv(160.0, 17.0);
    a.send(Event::MousePressed(MouseButton::Left));
    a.mv(220.0, 17.0);
    a.send(Event::MouseReleased(MouseButton::Left));
    assert!(
        (col_x(&mut a, "Status") - x0 - 60.0).abs() < 1.5,
        "later columns shift"
    );
    // Dragging far left stops at the minimum.
    a.mv(220.0, 17.0);
    a.send(Event::MousePressed(MouseButton::Left));
    a.mv(-500.0, 17.0);
    a.send(Event::MouseReleased(MouseButton::Left));
    assert!(
        (col_x(&mut a, "Status") - x0 + 112.0).abs() < 1.5,
        "stops at the 48 px minimum"
    );
    // Double-click the edge fits the content: "Item 99" is much narrower than 160.
    let edge = col_x(&mut a, "Status") - 12.0;
    a.double_click(edge, 17.0);
    assert!(col_x(&mut a, "Status") < x0, "autofit shrinks the column");
}

#[test]
fn grid_edit_commits_on_enter_and_cancels_on_escape() {
    let mut a = grid(100);
    a.double_click(60.0, row_y(1));
    a.double_click(60.0, row_y(1)); // first pair selects, second pair edits
    a.chord("a");
    a.type_text("Renamed");
    a.key(Key::Enter);
    assert_eq!(
        a.log.last(),
        Some(&GMsg::Select(vec![2])),
        "Enter moves down after committing"
    );
    assert!(a.log.contains(&GMsg::Edit(1, 0, "Renamed".into())));
    assert_eq!(a.state.rows[1][0], "Renamed");
    // Typing on the active cell starts an edit that replaces its content; Escape cancels.
    a.click(60.0, row_y(3));
    a.type_text("zz");
    a.key(Key::Escape);
    assert_eq!(a.state.rows[3][0], "Item 3");
    // Tab commits and moves right.
    a.click(60.0, row_y(4));
    a.type_text("tab");
    a.key(Key::Tab);
    assert!(a.log.contains(&GMsg::Edit(4, 0, "tab".into())));
    // The status column is not editable.
    a.click(400.0, row_y(5));
    a.type_text("x");
    a.key(Key::Enter);
    assert!(!a.log.iter().any(|m| matches!(m, GMsg::Edit(5, 2, _))));
}

#[test]
fn grid_scroll_reaches_last_row_and_clamps() {
    let mut a = grid(100_000);
    a.mv(200.0, 200.0);
    a.send(Event::Scrolled { dx: 0.0, dy: 1.0e9 });
    assert_eq!(
        a.ui.scroll_offset(2),
        0.0,
        "cannot scroll above the first row"
    );
    a.send(Event::Scrolled {
        dx: 0.0,
        dy: -1.0e9,
    });
    let t = a.texts();
    assert!(t.iter().any(|s| s == "Item 99999"), "last row visible");
}

#[test]
fn grid_goldens() {
    for (name, theme) in [("grid_light", Theme::light()), ("grid_dark", Theme::dark())] {
        let bg = theme.palette.background;
        let mut a = App::new(
            theme,
            GState {
                rows: grid_data(500),
                sort: Some((1, true)),
                selected: 0,
            },
            grid_view,
            grid_update,
        );
        a.click(60.0, row_y(2));
        mods(
            &mut a,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        a.click(60.0, row_y(4));
        mods(&mut a, Modifiers::default());
        a.mv(300.0, row_y(7));
        check(name, &a.image(bg));
    }
}

// ------------------------------------------------------------------ rich text

#[derive(Clone, Debug, PartialEq)]
enum RMsg {
    Doc(RichText),
    Link(String),
}
struct RState {
    doc: RichText,
    opened: Vec<String>,
}
fn rich_view(ui: &Ui, s: &RState) -> Element<RMsg> {
    RichEditor::new(ui, 3, &s.doc)
        .placeholder("Start writing")
        .on_change(RMsg::Doc)
        .on_link(RMsg::Link)
        .into_element()
}
fn rich_update(s: &mut RState, m: RMsg) {
    match m {
        RMsg::Doc(d) => s.doc = d,
        RMsg::Link(u) => s.opened.push(u),
    }
}
fn rich(md: &str) -> App<RState, RMsg> {
    let mut a = App::new(
        Theme::light(),
        RState {
            doc: RichText::from_markdown(md),
            opened: vec![],
        },
        rich_view,
        rich_update,
    );
    a.click(40.0, 75.0);
    a
}
fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Default::default()
    }
}

#[test]
fn rich_typing_and_enter_build_blocks() {
    let mut a = rich("");
    a.type_text("Hello\nworld");
    assert_eq!(a.state.doc.plain(), "Hello\nworld");
    assert_eq!(a.state.doc.blocks.len(), 2);
}

#[test]
fn rich_bold_via_shortcut_selection_and_pending_caret() {
    let mut a = rich("hello world");
    a.key_mod(Key::Named("Home".into()), Modifiers::default());
    for _ in 0..5 {
        a.key_mod(Key::Named("ArrowRight".into()), shift());
    }
    a.chord("b");
    assert_eq!(a.state.doc.to_markdown(), "**hello** world");
    // At a bare caret the toggle applies to what is typed next.
    a.named("End");
    a.chord("i");
    a.type_text("!");
    assert_eq!(a.state.doc.to_markdown(), "**hello** world*!*");
    a.chord("i");
    a.type_text("?");
    assert_eq!(a.state.doc.to_markdown(), "**hello** world*!*?");
}

#[test]
fn rich_toolbar_buttons_toggle_marks_and_blocks() {
    let mut a = rich("one two");
    a.chord("a");
    a.click(25.0, 21.0); // Bold
    assert_eq!(a.state.doc.to_markdown(), "**one two**");
    a.click(25.0, 21.0);
    assert_eq!(a.state.doc.to_markdown(), "one two");
    // Heading 1 and bullet buttons.
    let h1 = 228.0; // centre of the H1 button
    a.click(h1, 21.0);
    assert_eq!(a.state.doc.blocks[0].kind, BlockKind::Heading(1));
    a.click(h1, 21.0);
    assert_eq!(a.state.doc.blocks[0].kind, BlockKind::Paragraph);
}

#[test]
fn rich_markdown_shortcuts_and_list_exit() {
    let mut a = rich("");
    a.type_text("- ");
    assert_eq!(a.state.doc.blocks[0].kind, BlockKind::Bullet);
    assert_eq!(a.state.doc.plain(), "");
    a.type_text("milk\neggs\n");
    assert_eq!(a.state.doc.to_markdown(), "- milk\n- eggs\n- ");
    a.key(Key::Enter); // Enter on the empty item ends the list
    assert_eq!(
        a.state.doc.blocks.last().unwrap().kind,
        BlockKind::Paragraph
    );
    a.type_text("## Plans");
    assert_eq!(
        a.state.doc.blocks.last().unwrap().kind,
        BlockKind::Heading(2)
    );
    // Backspace at the start of a heading makes it a paragraph first.
    a.key_mod(Key::Named("Home".into()), Modifiers::default());
    a.key(Key::Backspace);
    assert_eq!(
        a.state.doc.blocks.last().unwrap().kind,
        BlockKind::Paragraph
    );
    assert_eq!(a.state.doc.blocks.last().unwrap().text(), "Plans");
}

#[test]
fn rich_links_apply_open_and_remove() {
    let mut a = rich("read the docs now");
    a.key_mod(Key::Named("Home".into()), Modifiers::default());
    for _ in 0..9 {
        a.key(Key::Named("ArrowRight".into()));
    }
    for _ in 0..4 {
        a.key_mod(Key::Named("ArrowRight".into()), shift());
    }
    a.chord("k");
    a.type_text("https://alloy.dev");
    a.key(Key::Enter);
    assert_eq!(
        a.state.doc.to_markdown(),
        "read the [docs](https://alloy.dev) now"
    );
    // Plain click only moves the caret; Ctrl-click opens.
    let x = a.draw().0.iter().find_map(|c| match c {
        DrawCommand::Text {
            content, origin, ..
        } if content == "docs" => Some(origin.x + 6.0),
        _ => None,
    });
    if let Some(x) = x {
        a.click(x, 75.0);
        assert!(a.state.opened.is_empty());
        a.send(Event::KeyReleased(
            Key::Named("Modifiers".into()),
            Modifiers {
                ctrl: true,
                ..Default::default()
            },
        ));
        a.click(x, 75.0);
        assert_eq!(a.state.opened, ["https://alloy.dev"]);
    }
    // Reselect and remove it with an empty URL.
    a.send(Event::KeyReleased(
        Key::Named("Modifiers".into()),
        Modifiers::default(),
    ));
    a.chord("a");
    a.chord("k");
    a.key(Key::Escape);
    assert!(
        a.state.doc.to_markdown().contains("[docs]"),
        "Escape leaves the link alone"
    );
}

#[test]
fn rich_undo_redo_and_cross_block_delete() {
    let mut a = rich("one\n\ntwo");
    a.chord("a");
    a.key(Key::Backspace);
    assert!(a.state.doc.is_empty());
    a.chord("z");
    assert_eq!(a.state.doc.plain(), "one\ntwo");
    a.key_mod(
        Key::Character("z".into()),
        Modifiers {
            shift: true,
            ctrl: !cfg!(target_os = "macos"),
            logo: cfg!(target_os = "macos"),
            ..Default::default()
        },
    );
    assert!(a.state.doc.is_empty());
    a.chord("z");
    a.type_text("abc");
    a.chord("z");
    assert_eq!(
        a.state.doc.plain(),
        "one\ntwo",
        "typing within a second is one undo step"
    );
}

#[test]
fn rich_vertical_movement_and_selection_copy() {
    let mut a = rich("first line\n\nsecond line");
    a.key_mod(Key::Named("Home".into()), Modifiers::default());
    a.named("ArrowDown");
    a.type_text("X");
    assert!(
        a.state.doc.blocks[1].text().contains('X'),
        "{:?}",
        a.state.doc.plain()
    );
    a.chord("a");
    a.chord("c");
    assert_eq!(a.ui.clipboard().get().unwrap().lines().count(), 2);
}

#[test]
fn rich_long_document_lays_out_and_scrolls() {
    let md: String = (0..300).map(|i| format!("Paragraph {i} with a fair amount of words to make it wrap onto a couple of lines.\n\n")).collect();
    let mut a = App::new(
        Theme::light(),
        RState {
            doc: RichText::from_markdown(&md),
            opened: vec![],
        },
        rich_view,
        rich_update,
    );
    a.mv(300.0, 200.0);
    a.send(Event::Scrolled {
        dx: 0.0,
        dy: -1.0e9,
    });
    assert!(a.ui.scroll_offset(3) > 10_000.0);
    let t = a.texts();
    assert!(t.iter().any(|s| s.contains("Paragraph 299")));
    assert!(t.len() < 200, "only visible lines are drawn: {}", t.len());
}

const NOTE: &str = "# Launch notes\n\nThe **beta** ships on *Friday*. Read the [release plan](https://alloy.dev/plan) and keep `cargo test` green.\n\n## Checklist\n\n- Freeze the API\n- Write the changelog\n- Tag **v0.4**\n\n1. Smoke test on three platforms\n2. Publish the docs\n\n> Small, sharp tools win.";

#[test]
fn rich_goldens() {
    for (name, theme) in [
        ("richedit_light", Theme::light()),
        ("richedit_dark", Theme::dark()),
    ] {
        let bg = theme.palette.background;
        let mut a = App::new(
            theme,
            RState {
                doc: RichText::from_markdown(NOTE),
                opened: vec![],
            },
            rich_view,
            rich_update,
        );
        a.click(60.0, 150.0);
        a.key_mod(Key::Named("ArrowRight".into()), shift());
        for _ in 0..11 {
            a.key_mod(Key::Named("ArrowRight".into()), shift());
        }
        check(name, &a.image(bg));
    }
}
