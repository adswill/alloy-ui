//! Behaviour and performance tests for the pro widgets: data grid pinning, grouping,
//! filtering and paste; code editor multi-cursor and bracket matching; rich-text blocks;
//! calendar spans. Goldens for them live in `golden_pro.rs`.

use alloy_ui_core::*;
use alloy_ui_theme::Theme;
use alloy_ui_widgets::*;
use std::{rc::Rc, time::Instant};

const W: f32 = 640.0;
const H: f32 = 360.0;

/// A tiny Elm loop, like the runtime's: rebuild after every event.
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
    fn new(state: S, view: fn(&Ui, &S) -> Element<M>, update: fn(&mut S, M)) -> Self {
        let ui = Ui::new(Theme::light());
        let root = Root::new(&ui, view(&ui, &state)).into_element();
        let layout = alloy_ui_widgets::layout(&root, Size::new(W, H));
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
    fn draw(&mut self) -> DrawList {
        self.root = Root::new(&self.ui, (self.view)(&self.ui, &self.state)).into_element();
        self.layout = alloy_ui_widgets::layout(&self.root, Size::new(W, H));
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
    fn click(&mut self, x: f32, y: f32) {
        self.send(Event::CursorMoved(Point::new(x, y)));
        self.send(Event::MousePressed(MouseButton::Left));
        self.send(Event::MouseReleased(MouseButton::Left));
    }
    fn key_mod(&mut self, k: Key, m: Modifiers) {
        self.send(Event::KeyPressed(k, m));
    }
    fn named(&mut self, n: &str) {
        self.key_mod(Key::Named(n.into()), Modifiers::default());
    }
    fn chord(&mut self, c: &str) {
        let m = Modifiers {
            ctrl: !cfg!(target_os = "macos"),
            logo: cfg!(target_os = "macos"),
            ..Default::default()
        };
        self.key_mod(Key::Character(c.into()), m);
    }
    fn texts(&mut self) -> Vec<(String, Point)> {
        self.draw()
            .0
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Text {
                    content, origin, ..
                } => Some((content.clone(), *origin)),
                _ => None,
            })
            .collect()
    }
    fn text_x(&mut self, t: &str) -> Option<f32> {
        self.texts().into_iter().find(|x| x.0 == t).map(|x| x.1.x)
    }
}

// ------------------------------------------------------------------ data grid

#[derive(Clone, Debug, PartialEq)]
enum G {
    Toggle(String),
    Edit(usize, usize, String),
    Sort(usize, bool),
}

struct GS {
    data: Rc<Vec<[String; 3]>>,
    model: RowModel,
}

fn people() -> Rc<Vec<[String; 3]>> {
    let teams = ["eng", "ops", "eng", "sales", "ops", "eng"];
    Rc::new(
        (0..6)
            .map(|i| {
                [
                    format!("Person {i}"),
                    teams[i].to_string(),
                    format!("{}", 100 + i * 10),
                ]
            })
            .collect(),
    )
}

fn gs() -> GS {
    let data = people();
    let mut model = RowModel::new(data.len());
    let d = data.clone();
    model.rebuild(&|r, c| d[r][c].clone());
    GS { data, model }
}

fn g_view(ui: &Ui, s: &GS) -> Element<G> {
    let d = s.data.clone();
    DataGrid::from_model(
        ui,
        7,
        vec![
            GridColumn::new("Name", 140.0).editable().pinned(),
            GridColumn::new("Team", 140.0).editable(),
            GridColumn::new("Pay", 140.0).right().editable(),
        ],
        &s.model,
        move |r, c| d[r][c].clone(),
    )
    .on_toggle_group(G::Toggle)
    .on_edit(G::Edit)
    .on_sort(G::Sort)
    .into_element()
}

fn g_update(s: &mut GS, m: G) {
    match m {
        G::Toggle(k) => s.model.toggle_group(&k),
        G::Edit(r, c, t) => {
            let mut v = (*s.data).clone();
            v[r][c] = t;
            s.data = Rc::new(v);
        }
        G::Sort(c, asc) => s.model.sort(Some((c, asc))),
    }
    let d = s.data.clone();
    s.model.rebuild(&|r, c| d[r][c].clone());
}

fn grid() -> App<GS, G> {
    App::new(gs(), g_view, g_update)
}

fn row_center(r: usize) -> f32 {
    34.0 + r as f32 * 36.0 + 18.0
}

#[test]
fn grid_groups_toggle_by_click_and_keyboard() {
    let mut a = grid();
    a.state.model.group_by(Some(1));
    let d = a.state.data.clone();
    a.state.model.rebuild(&|r, c| d[r][c].clone());
    a.draw();
    assert_eq!(
        a.state.model.len(),
        3 + 6,
        "three group headers and six rows"
    );
    // Click the "eng" header (row 0) to collapse it.
    a.click(300.0, row_center(0));
    assert_eq!(a.log.last(), Some(&G::Toggle("eng".into())));
    assert_eq!(a.state.model.len(), 3 + 3);
    // Keyboard: the header row is active; Right expands, Left collapses again.
    a.named("ArrowRight");
    assert_eq!(
        a.state.model.len(),
        3 + 6,
        "Right expands a collapsed group"
    );
    a.named("ArrowLeft");
    assert_eq!(a.state.model.len(), 3 + 3, "Left collapses an expanded one");
    a.key_mod(Key::Enter, Modifiers::default());
    assert_eq!(a.state.model.len(), 3 + 6, "Enter toggles");
    let t: Vec<String> = a.texts().into_iter().map(|x| x.0).collect();
    assert!(t.contains(&"eng".to_string()) && t.contains(&"sales".to_string()));
}

#[test]
fn grid_pinned_column_stays_while_others_scroll() {
    let mut a = App::new(
        gs(),
        |ui, s| {
            let d = s.data.clone();
            DataGrid::<G>::from_model(
                ui,
                8,
                vec![
                    GridColumn::new("Name", 120.0).pinned(),
                    GridColumn::new("Team", 300.0),
                    GridColumn::new("Pay", 300.0),
                    GridColumn::new("Extra", 300.0),
                ],
                &s.model,
                move |r, c| {
                    if c == 3 {
                        "x".to_string()
                    } else {
                        d[r][c].clone()
                    }
                },
            )
            .into_element()
        },
        g_update,
    );
    let (n0, t0) = (a.text_x("Name").unwrap(), a.text_x("Team").unwrap());
    a.send(Event::CursorMoved(Point::new(300.0, 120.0)));
    a.send(Event::Scrolled {
        dx: -250.0,
        dy: 0.0,
    });
    let (n1, t1) = (a.text_x("Name").unwrap(), a.text_x("Team"));
    assert_eq!(n0, n1, "the pinned header does not move");
    assert!(
        t1.is_none_or(|t| t < t0 - 100.0),
        "the scrolling header does"
    );
    // A pinned cell is still drawn at the left edge.
    let cell = a
        .texts()
        .into_iter()
        .find(|x| x.0 == "Person 0")
        .map(|x| x.1.x);
    assert!(
        cell.is_some_and(|x| x < 50.0),
        "pinned cells stay at the left: {cell:?}"
    );
}

#[test]
fn grid_paste_writes_a_block_of_cells_from_the_active_cell() {
    let mut a = grid();
    a.click(200.0, row_center(1));
    a.ui.clipboard().set("A1\tB1\nA2\tB2".to_string());
    // Active cell is (1, 1): Team. Paste lands on Team and Pay of rows 1 and 2.
    a.chord("v");
    let edits: Vec<&G> = a.log.iter().filter(|m| matches!(m, G::Edit(..))).collect();
    assert_eq!(edits.len(), 4, "{edits:?}");
    assert_eq!(a.state.data[1][1], "A1");
    assert_eq!(a.state.data[1][2], "B1");
    assert_eq!(a.state.data[2][1], "A2");
}

#[test]
fn grid_ctrl_arrows_jump_to_the_edges() {
    let mut a = grid();
    a.click(300.0, row_center(2));
    a.key_mod(
        Key::Named("ArrowDown".into()),
        Modifiers {
            ctrl: !cfg!(target_os = "macos"),
            logo: cfg!(target_os = "macos"),
            ..Default::default()
        },
    );
    a.chord("c");
    assert!(
        a.ui.clipboard().get().unwrap().starts_with("Person 5"),
        "Ctrl+Down lands on the last row"
    );
}

#[test]
fn row_model_handles_a_million_rows() {
    // A debug build is an order of magnitude slower; the release numbers are in BENCHMARKS.md.
    let n = if cfg!(debug_assertions) {
        200_000
    } else {
        1_000_000
    };
    let cell = |r: usize, c: usize| match c {
        0 => format!("Row {r}"),
        1 => format!("g{}", r % 50),
        _ => format!("{}", (r * 7919) % 100_000),
    };
    let mut m = RowModel::new(n);
    let t = Instant::now();
    m.filter(2, ">50000");
    m.sort(Some((2, false)));
    m.rebuild(&cell);
    let rebuild = t.elapsed();
    assert!(m.matched() > n / 3 && m.matched() < n);
    let top = m.source(0).unwrap();
    assert!((top * 7919) % 100_000 >= 99_000, "descending by pay");
    let t = Instant::now();
    m.group_by(Some(1));
    m.rebuild(&cell);
    let grouped = t.elapsed();
    assert_eq!(m.len(), m.matched() + 50);
    // Drawing one screen of the grid over the model touches only what is visible.
    let ui = Ui::new(Theme::light());
    let calls = Rc::new(std::cell::Cell::new(0usize));
    let c2 = calls.clone();
    let el = DataGrid::<()>::from_model(
        &ui,
        3,
        vec![
            GridColumn::new("Name", 140.0),
            GridColumn::new("Group", 140.0),
            GridColumn::new("Pay", 140.0),
        ],
        &m,
        move |r, c| {
            c2.set(c2.get() + 1);
            cell(r, c)
        },
    )
    .into_element();
    let root = Root::new(&ui, el).into_element();
    let l = alloy_ui_widgets::layout(&root, Size::new(W, H));
    let t = Instant::now();
    let mut list = DrawList::default();
    root.0.draw(&l, &mut list);
    let draw = t.elapsed();
    println!(
        "rows={n} rebuild(filter+sort)={rebuild:?} regroup={grouped:?} draw={draw:?} cells={}",
        calls.get()
    );
    assert!(
        calls.get() <= 3 * 14,
        "{} cell reads for one screen",
        calls.get()
    );
    assert!(
        draw.as_millis() < 100,
        "one frame of a million-row grid took {draw:?}"
    );
    assert!(rebuild.as_secs() < 20 && grouped.as_secs() < 20);
}

// ------------------------------------------------------------------ code editor

fn code(initial: &str) -> App<String, String> {
    let mut a = App::new(
        initial.to_string(),
        |ui, s| {
            CodeEditor::new(ui, 1, s.clone())
                .lang(Lang::Rust)
                .on_input(|v| v)
                .into_element()
        },
        |s, m| *s = m,
    );
    a.click(100.0, 20.0);
    a
}

fn shortcut(a: &mut App<String, String>, k: Key, shift: bool, alt: bool) {
    let m = Modifiers {
        ctrl: !cfg!(target_os = "macos"),
        logo: cfg!(target_os = "macos"),
        shift,
        alt,
    };
    a.key_mod(k, m);
}

fn type_str(a: &mut App<String, String>, s: &str) {
    for ch in s.chars() {
        a.send(Event::TextInput(ch.to_string()));
    }
}

#[test]
fn code_next_occurrence_selects_each_match_and_typing_edits_all() {
    let mut a = code("foo bar foo baz foo");
    shortcut(&mut a, Key::Named("Home".into()), false, false);
    a.named("ArrowRight");
    for _ in 0..3 {
        shortcut(&mut a, Key::Character("d".into()), true, false);
    }
    type_str(&mut a, "qux");
    assert_eq!(a.state, "qux bar qux baz qux");
    // Typing over the selections is one step and the burst after it another, as with one caret.
    a.chord("z");
    assert_eq!(a.state, "q bar q baz q");
    a.chord("z");
    assert_eq!(a.state, "foo bar foo baz foo");
}

#[test]
fn code_select_all_occurrences_and_escape_returns_to_one_caret() {
    let mut a = code("let a = 1;\nlet b = a;\nlet c = a;");
    shortcut(&mut a, Key::Named("Home".into()), false, false);
    for _ in 0..4 {
        a.named("ArrowRight");
    }
    shortcut(&mut a, Key::Character("l".into()), true, false);
    type_str(&mut a, "mut");
    assert_eq!(a.state, "let mut = 1;\nlet b = mut;\nlet c = mut;");
    a.key_mod(Key::Escape, Modifiers::default());
    type_str(&mut a, "!");
    assert_eq!(a.state.matches('!').count(), 1, "Escape leaves one caret");
}

#[test]
fn code_carets_added_below_type_backspace_and_split_lines_together() {
    let mut a = code("a\nb\nc");
    a.chord("a");
    a.named("ArrowLeft"); // caret at the start of the text
    let alt_shift = Modifiers {
        alt: true,
        shift: true,
        ..Default::default()
    };
    a.key_mod(Key::Named("ArrowDown".into()), alt_shift);
    a.key_mod(Key::Named("ArrowDown".into()), alt_shift);
    type_str(&mut a, "-");
    assert_eq!(a.state, "-a\n-b\n-c", "one caret per line");
    a.key_mod(Key::Backspace, Modifiers::default());
    assert_eq!(a.state, "a\nb\nc");
    a.named("End");
    a.key_mod(Key::Enter, Modifiers::default());
    assert_eq!(a.state, "a\n\nb\n\nc\n", "Enter at each caret");
    // Escape drops the extra carets: typing now affects only one.
    a.key_mod(Key::Escape, Modifiers::default());
    type_str(&mut a, "!");
    assert_eq!(a.state.matches('!').count(), 1);
}

#[test]
fn code_alt_click_adds_a_caret() {
    let mut a = code("one\ntwo\nthree");
    a.named("Home");
    a.send(Event::KeyReleased(
        Key::Named("Modifiers".into()),
        Modifiers {
            alt: true,
            ..Default::default()
        },
    ));
    // Alt+click on the third line (gutter is about 46 px wide, lines are ~20 px tall).
    a.click(120.0, 8.0 + 2.5 * 20.0);
    a.send(Event::KeyReleased(
        Key::Named("Modifiers".into()),
        Modifiers::default(),
    ));
    type_str(&mut a, "#");
    assert_eq!(
        a.state, "#one\ntwo\nthree#",
        "one caret at the start, one added at the end of line 3"
    );
}

#[test]
fn code_editor_stays_fast_on_a_huge_file_with_many_carets() {
    // Debug builds are far slower than release; the release numbers are in BENCHMARKS.md.
    let lines = if cfg!(debug_assertions) {
        50_000
    } else {
        200_000
    };
    let text: String = (0..lines)
        .map(|i| format!("    let value_{i} = compute({i}, \"item\"); // line {i}\n"))
        .collect();
    let ui = Ui::new(Theme::light());
    let mk = |t: &str| {
        Root::new(
            &ui,
            CodeEditor::<()>::new(&ui, 1, t.to_string())
                .lang(Lang::Rust)
                .into_element(),
        )
        .into_element()
    };
    let root = mk(&text);
    let l = alloy_ui_widgets::layout(&root, Size::new(W, H));
    let t = Instant::now();
    let mut list = DrawList::default();
    root.0.draw(&l, &mut list); // first draw indexes the file
    let first = t.elapsed();
    let t = Instant::now();
    let mut list = DrawList::default();
    root.0.draw(&l, &mut list);
    let again = t.elapsed();
    let texts = list
        .0
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { .. }))
        .count();
    // Every occurrence of `compute` gets a caret, then one keystroke edits them all.
    let mut a = App::new(
        text.clone(),
        |ui, s| {
            CodeEditor::new(ui, 1, s.clone())
                .lang(Lang::Rust)
                .on_input(|v| v)
                .into_element()
        },
        |s, m| *s = m,
    );
    a.click(100.0, 20.0);
    shortcut(&mut a, Key::Named("Home".into()), false, false);
    for _ in 0..(4 + "let value_0 = ".len()) {
        a.named("ArrowRight");
    }
    let t = Instant::now();
    shortcut(&mut a, Key::Character("l".into()), true, false);
    let select_all = t.elapsed();
    let t = Instant::now();
    a.send(Event::TextInput("X".into()));
    let type_at_all = t.elapsed();
    let edited = a.state.matches("Xcompute").count() + a.state.matches("X(").count();
    let again = {
        let _ = edited;
        again
    };
    println!(
        "lines={lines} first_draw={first:?} second_draw={again:?} select_all_occurrences={select_all:?} type_at_every_caret={type_at_all:?}"
    );
    assert!(
        a.state.matches("X(").count() == lines,
        "one edit per line: {}",
        a.state.matches("X(").count()
    );
    assert!(texts < 400, "{texts} text commands for one screen");
    assert!(
        again.as_millis() < 100,
        "a frame of a {lines}-line file took {again:?}"
    );
    assert!(first.as_secs() < 10);
}

// ------------------------------------------------------------------ rich text

fn rich_app(md: &str) -> App<RichText, RichText> {
    let mut a = App::new(
        RichText::from_markdown(md),
        |ui, d| {
            RichEditor::new(ui, 3, d)
                .on_change(|d| d)
                .images(|src| (src == "known.png").then_some(77))
                .into_element()
        },
        |s, m| *s = m,
    );
    a.click(40.0, 75.0);
    a
}

#[test]
fn rich_code_shortcut_enter_continues_and_empty_enter_exits() {
    let mut a = rich_app("");
    for ch in "``` ".chars() {
        a.send(Event::TextInput(ch.to_string()));
    }
    assert_eq!(
        a.state.blocks[0].kind,
        BlockKind::Code,
        "``` starts a code block"
    );
    for ch in "let x = 1;".chars() {
        a.send(Event::TextInput(ch.to_string()));
    }
    a.key_mod(Key::Enter, Modifiers::default());
    assert_eq!(a.state.blocks.len(), 2);
    assert_eq!(
        a.state.blocks[1].kind,
        BlockKind::Code,
        "enter stays in the block"
    );
    a.key_mod(Key::Enter, Modifiers::default());
    assert_eq!(
        a.state.blocks[1].kind,
        BlockKind::Paragraph,
        "empty line leaves it"
    );
    assert_eq!(a.state.to_markdown(), "```\nlet x = 1;\n```\n\n");
}

#[test]
fn rich_deep_heading_shortcut() {
    let mut a = rich_app("");
    for ch in "#### ".chars() {
        a.send(Event::TextInput(ch.to_string()));
    }
    assert_eq!(a.state.blocks[0].kind, BlockKind::Heading(4));
}

#[test]
fn rich_images_draw_through_the_resolver_or_as_a_placeholder() {
    let mut a = rich_app("hello\n\n![one](known.png)\n\n![two](gone.png)");
    let cmds = a.draw().0;
    let imgs: Vec<_> = cmds
        .iter()
        .filter_map(|c| match c {
            DrawCommand::Image { id, bounds } => Some((*id, bounds.size.width)),
            _ => None,
        })
        .collect();
    assert_eq!(imgs.len(), 1, "only the resolvable image is a bitmap");
    assert_eq!(imgs[0].0, 77);
    assert!(imgs[0].1 > 100.0);
    let t = a.texts();
    assert!(
        t.iter().any(|x| x.0 == "gone.png"),
        "placeholder names its source"
    );
    assert!(t.iter().any(|x| x.0 == "one"), "alt text is the caption");
}

#[test]
fn rich_backspace_at_image_start_turns_it_into_text() {
    let mut a = rich_app("![cap](known.png)");
    a.send(Event::KeyPressed(
        Key::Named("Home".into()),
        Modifiers::default(),
    ));
    a.key_mod(Key::Backspace, Modifiers::default());
    assert_eq!(a.state.blocks[0].kind, BlockKind::Paragraph);
    assert_eq!(a.state.blocks[0].src, None);
    assert_eq!(a.state.blocks[0].text(), "cap");
}

// ------------------------------------------------------------------ calendar

fn cal_app(
    view: CalView,
    anchor: Date,
    entries: Vec<Entry>,
) -> App<(CalView, Date, Vec<Entry>), CalAction> {
    App::new(
        (view, anchor, entries),
        |ui, s| {
            Calendar::new(ui, 5, s.1, s.0, s.2.clone())
                .hours(8, 16)
                .on_action(|a| a)
                .into_element()
        },
        |s, a| {
            a.apply(&mut s.2);
        },
    )
}

fn week_of(d: Date) -> Date {
    Date::from_days(d.to_days() - d.weekday() as i64)
}

/// Column `c` of the week view: centre x. Gutter 52, seven columns in a 640 wide window.
fn col_x(c: f32) -> f32 {
    52.0 + (c + 0.5) * (640.0 - 52.0) / 7.0
}

#[test]
fn calendar_all_day_entries_sit_in_a_band_and_drag_across_days() {
    let anchor = Date::new(2025, 3, 12);
    let ws = week_of(anchor);
    let day = |n: i64| Date::from_days(ws.to_days() + n);
    let trip = Entry::all_day(1, "Offsite", day(1), 3);
    let mut a = cal_app(
        CalView::Week,
        anchor,
        vec![trip, Entry::new(2, "Standup", day(2), 9 * 60, 10 * 60)],
    );
    let cmds = a.draw().0;
    let title_y =
        |a: &mut App<_, _>, t: &str| a.texts().into_iter().find(|x| x.0 == t).map(|x| x.1.y);
    let band_y = title_y(&mut a, "Offsite").expect("banner title drawn");
    let timed_y = title_y(&mut a, "Standup").expect("timed title drawn");
    assert!(band_y < timed_y, "banner above the time grid");
    assert!(cmds
        .iter()
        .any(|c| matches!(c, DrawCommand::Text { content, .. } if content == "all-day")));
    // Drag from the middle day (column 2) to column 4: the entry moves two days.
    a.send(Event::CursorMoved(Point::new(col_x(2.0), 118.0)));
    a.send(Event::MousePressed(MouseButton::Left));
    a.send(Event::CursorMoved(Point::new(col_x(3.0), 118.0)));
    a.send(Event::CursorMoved(Point::new(col_x(4.0), 118.0)));
    a.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(
        a.log.last(),
        Some(&CalAction::Move {
            id: 1,
            date: day(3),
            start: 0
        }),
        "grab point is kept: day 2 -> day 4 moves the start from 1 to 3"
    );
    assert_eq!(a.state.2[0].date, day(3));
    assert_eq!(a.state.2[0].days, 3, "length is kept");
}

#[test]
fn calendar_right_edge_changes_the_span_and_empty_band_creates() {
    let anchor = Date::new(2025, 3, 12);
    let ws = week_of(anchor);
    let day = |n: i64| Date::from_days(ws.to_days() + n);
    let mut a = cal_app(
        CalView::Week,
        anchor,
        vec![Entry::all_day(1, "Offsite", day(1), 3)],
    );
    // Right edge of column 3 (the last covered day).
    let edge = 52.0 + 4.0 * (640.0 - 52.0) / 7.0 - 4.0;
    a.send(Event::CursorMoved(Point::new(edge, 118.0)));
    a.send(Event::MousePressed(MouseButton::Left));
    a.send(Event::CursorMoved(Point::new(col_x(5.0), 118.0)));
    a.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(a.log.last(), Some(&CalAction::Span { id: 1, days: 5 }));
    assert_eq!(a.state.2[0].days, 5);
    // Dragging the edge back past the start never goes below one day.
    let edge = 52.0 + 6.0 * (640.0 - 52.0) / 7.0 - 4.0;
    a.send(Event::CursorMoved(Point::new(edge, 118.0)));
    a.send(Event::MousePressed(MouseButton::Left));
    a.send(Event::CursorMoved(Point::new(col_x(0.0), 118.0)));
    a.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(a.state.2[0].days, 1);
    // A click on an empty band spot makes an all-day entry there.
    a.send(Event::CursorMoved(Point::new(col_x(6.0), 118.0)));
    a.send(Event::MousePressed(MouseButton::Left));
    a.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(
        a.log.last(),
        Some(&CalAction::CreateAllDay { date: day(6) })
    );
    assert!(a.state.2.iter().any(|e| e.all_day && e.date == day(6)));
}

#[test]
fn calendar_month_bars_span_cells_stack_and_drag_by_the_grabbed_day() {
    // February 2027 starts on a Monday and fits four rows, so each cell has room for chips.
    let anchor = Date::new(2027, 2, 10);
    let d = |n: u32| Date::new(2027, 2, n);
    let mut a = cal_app(
        CalView::Month,
        anchor,
        vec![
            Entry::all_day(1, "Trip", d(3), 3),
            Entry::all_day(2, "Hack", d(4), 1),
        ],
    );
    let ty = |a: &mut App<_, _>, t: &str| {
        a.texts()
            .into_iter()
            .find(|x| x.0.ends_with(t))
            .map(|x| x.1)
    };
    let trip = ty(&mut a, "Trip").expect("bar title");
    let hack = ty(&mut a, "Hack").expect("second bar");
    assert!(hack.y > trip.y, "overlapping bars take separate lanes");
    let mut b = cal_app(
        CalView::Month,
        anchor,
        vec![
            Entry::all_day(1, "Trip", d(3), 3),
            Entry::new(3, "Lunch", d(3), 12 * 60, 13 * 60),
        ],
    );
    let (trip_b, lunch) = (
        ty(&mut b, "Trip").unwrap(),
        ty(&mut b, "Lunch").expect("chip"),
    );
    assert!(lunch.y > trip_b.y, "timed chips start below the bars");
    // The bar spans three cells: its title starts in Wednesday's cell.
    let cw = 640.0 / 7.0;
    assert!(trip.x > 2.0 * cw && trip.x < 3.0 * cw);
    // Grab the bar on Thursday (second day) in row 0 and drop it on Thursday of row 1:
    // the start moves to Wednesday the 10th.
    let bar_y = trip.y + 6.0;
    a.send(Event::CursorMoved(Point::new(3.5 * cw, bar_y)));
    a.send(Event::MousePressed(MouseButton::Left));
    a.send(Event::CursorMoved(Point::new(3.5 * cw, bar_y + 40.0)));
    a.send(Event::CursorMoved(Point::new(3.5 * cw, bar_y + 76.0)));
    a.send(Event::MouseReleased(MouseButton::Left));
    assert_eq!(
        a.log.last(),
        Some(&CalAction::Move {
            id: 1,
            date: d(10),
            start: 0
        })
    );
}

#[test]
fn calendar_bars_continue_across_week_rows() {
    let anchor = Date::new(2027, 2, 10);
    let mut a = cal_app(
        CalView::Month,
        anchor,
        vec![Entry::all_day(1, "Sprint", Date::new(2027, 2, 5), 5)], // Fri 5th to Tue 9th
    );
    let cmds = a.draw().0;
    let arrows = cmds
        .iter()
        .filter(|c| matches!(c, DrawCommand::Text { content, .. } if content == "›"))
        .count();
    assert_eq!(
        arrows, 2,
        "toolbar next, plus the first week's bar running on"
    );
    assert!(
        cmds.iter()
            .any(|c| matches!(c, DrawCommand::Text { content, .. } if content == "‹ Sprint")),
        "the second week's piece is marked as a continuation"
    );
}

#[test]
fn calendar_apply_edits_the_entry_list() {
    let d = Date::new(2025, 3, 12);
    let mut es = vec![Entry::new(1, "A", d, 600, 660)];
    assert!(CalAction::Move {
        id: 1,
        date: Date::new(2025, 3, 14),
        start: 720
    }
    .apply(&mut es));
    assert_eq!((es[0].date.day, es[0].start, es[0].end), (14, 720, 780));
    assert!(CalAction::Span { id: 1, days: 0 }.apply(&mut es));
    assert_eq!(es[0].days, 1, "never below one day");
    assert!(CalAction::CreateAllDay { date: d }.apply(&mut es));
    assert_eq!(es[1].id, 2);
    assert!(es[1].all_day);
    assert!(
        !CalAction::Open(1).apply(&mut es),
        "navigation is the app's"
    );
    assert!(!CalAction::Move {
        id: 9,
        date: d,
        start: 0
    }
    .apply(&mut es));
}
