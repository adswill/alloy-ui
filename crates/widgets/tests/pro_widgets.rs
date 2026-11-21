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
        self.root.0.on_event(&e, &self.layout, self.cursor, &mut out);
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
    assert_eq!(a.state.model.len(), 3 + 6, "three group headers and six rows");
    // Click the "eng" header (row 0) to collapse it.
    a.click(300.0, row_center(0));
    assert_eq!(a.log.last(), Some(&G::Toggle("eng".into())));
    assert_eq!(a.state.model.len(), 3 + 3);
    // Keyboard: the header row is active; Right expands, Left collapses again.
    a.named("ArrowRight");
    assert_eq!(a.state.model.len(), 3 + 6, "Right expands a collapsed group");
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
                move |r, c| if c == 3 { "x".to_string() } else { d[r][c].clone() },
            )
            .into_element()
        },
        g_update,
    );
    let (n0, t0) = (a.text_x("Name").unwrap(), a.text_x("Team").unwrap());
    a.send(Event::CursorMoved(Point::new(300.0, 120.0)));
    a.send(Event::Scrolled { dx: -250.0, dy: 0.0 });
    let (n1, t1) = (a.text_x("Name").unwrap(), a.text_x("Team"));
    assert_eq!(n0, n1, "the pinned header does not move");
    assert!(t1.is_none_or(|t| t < t0 - 100.0), "the scrolling header does");
    // A pinned cell is still drawn at the left edge.
    let cell = a.texts().into_iter().find(|x| x.0 == "Person 0").map(|x| x.1.x);
    assert!(cell.is_some_and(|x| x < 50.0), "pinned cells stay at the left: {cell:?}");
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
    let n = if cfg!(debug_assertions) { 200_000 } else { 1_000_000 };
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
    println!("rows={n} rebuild(filter+sort)={rebuild:?} regroup={grouped:?} draw={draw:?} cells={}", calls.get());
    assert!(calls.get() <= 3 * 14, "{} cell reads for one screen", calls.get());
    assert!(draw.as_millis() < 100, "one frame of a million-row grid took {draw:?}");
    assert!(rebuild.as_secs() < 20 && grouped.as_secs() < 20);
}
