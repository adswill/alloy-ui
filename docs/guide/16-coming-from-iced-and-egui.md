# Coming from iced, egui or Slint

Alloy keeps iced's architecture (state, messages, view, native GPU drawing) and drops most of its ceremony. If you know egui, the biggest change is that the view is a value you return, not a series of calls on a `Ui`. If you know Slint, the biggest change is that there is no second language: the markup is Rust. This page maps what you already know, with the same app written side by side. It is a guide to getting started, not a claim that the others are wrong: iced, egui and Slint are excellent and have things Alloy does not (see "What you give up").

Jump to: [iced](#from-iced) · [egui](#from-egui) · [Slint](#from-slint) · [what you give up](#what-you-give-up) · [a porting checklist](#a-porting-checklist)

## One page of vocabulary

| Idea | iced | egui | Slint | Alloy |
|---|---|---|---|---|
| App state | struct + `Message` | your struct, mutated in `update` | `property <T>` in markup, plus Rust structs | your struct (or `sketch(state, view)`) |
| Change state | `update(msg)` | assign inside `if ui.button().clicked()` | `callback` + handler `=> { .. }` | `.act(\|s\| ..)`, or `App::update(msg)` |
| Describe the UI | `view() -> Element` | calls on `ui` every frame | `.slint` component tree | `view() -> impl View` |
| Layout | `column![]`, `row![]` | `ui.horizontal`, `ui.vertical` | `VerticalLayout`, `HorizontalLayout` | `col![]`, `row![]` |
| Two-way field | `on_input` + message | `&mut String` | `<=>` binding | `at!(s.field)` |
| Lists | `column(iter)` | `for` loop | `for x in model :` | `col(iter)`, `vlist`, `table` |
| Background work | `Task::perform` | your own thread + channel | `slint::spawn_local`, `invoke_from_event_loop` | `Cmd::perform`, `resource` |
| Cancel stale work | `Task::abortable` | drop your channel | drop the task handle | `Cmd::perform_keyed` / `Cmd::cancel`, `resource_for` |
| Navigation | none built in | none built in | `if page == ..` | `router` |
| UI tests | `iced_test` | `egui_kittest` | `i-slint-backend-testing` | `Harness` |

## From iced

The same counter in both. Alloy has two styles; the first is closest to what you know.

```rust,ignore
// iced
use iced::widget::{button, column, row, text};

#[derive(Default)]
struct Counter { value: i64 }

#[derive(Debug, Clone, Copy)]
enum Message { Inc, Dec }

fn update(c: &mut Counter, m: Message) {
    match m { Message::Inc => c.value += 1, Message::Dec => c.value -= 1 }
}

fn view(c: &Counter) -> iced::Element<'_, Message> {
    column![
        text(format!("{} so far", c.value)),
        row![button("-").on_press(Message::Dec), button("+").on_press(Message::Inc)].spacing(8),
    ].spacing(14).padding(24).into()
}

fn main() -> iced::Result {
    iced::application(Counter::default, update, view).run()
}
```

```rust
// Alloy, same shape (the App trait)
use alloy_ui::prelude::*;

#[derive(Default)]
struct Counter(i64);

#[derive(Clone)]
enum Msg { Inc, Dec }

impl App for Counter {
    type Msg = Msg;
    fn update(&mut self, m: Msg) {
        match m { Msg::Inc => self.0 += 1, Msg::Dec => self.0 -= 1 }
    }
    fn view(&self) -> impl View<Msg> {
        col![
            text(format!("{} so far", self.0)),
            row![button("-").on(Msg::Dec), button("+").on(Msg::Inc)],
        ]
        .gap(14.0)
        .pad(24.0)
    }
}
# fn main() { let _ = Counter(0); }
```

```rust,no_run
// Alloy, closure style: no message enum, no update function
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![
            text(format!("{n} so far")),
            row![button("-").act(|n| *n -= 1), button("+").act(|n| *n += 1)],
        ]
        .gap(14.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

| iced | Alloy |
|---|---|
| `Message` enum + `update` | `sketch` closures (`.act(\|s\| ..)`), or the `App` trait when you want a message log |
| `column![]`, `row![]`, `.spacing(8)`, `.padding(24)` | `col![]`, `row![]`, `.gap(8.0)`, `.pad(24.0)` |
| `button(..).on_press(m)` | `button(..).on(m)` or `.act(\|s\| ..)`; `.primary()`, `.ghost()` for styles |
| `text_input(placeholder, &value).on_input(Msg::Edit)` | `input(at!(s.field)).hint(..)`: two-way, no message |
| `checkbox`, `slider`, `pick_list` | `check(label, at!(..))`, `slider`, `pick`/`choose` |
| `Length::Fill`, `Fill` | `.fill()`, `.fill_w()`, `.fill_h()` |
| `Task::perform(fut, Msg::Done)` | `Cmd::perform(async { Msg::Done(..) })` from `update_cmd`, or `resource(fetch)` for data a view shows |
| `Subscription::run`, `time::every` | `subscriptions()` with `every(period, \|\| msg)`, or `.tick(..)` in `sketch` |
| `iced::window::open` | `Cmd::open_window(id, title, w, h)` and `view_window(id)` |
| custom `Theme` / `Palette` / `Catalog` styling | `set_look(Look::dark().accent(..).density(..))`; the accent is nudged until it passes WCAG AA |
| local widget state in your struct | `local(value)` in the view, `store(..)` if it should be saved |
| no built-in router | `router(home)` ([navigation](14-navigation.md)) |
| `iced_test` / simulator | `Harness` ([testing](15-testing.md)) |

Things that behave differently:

* **View-local state.** In iced a search box, a sort order or an open panel is a field in your state plus a message. In Alloy `let q = local(String::new())` in the view keeps it with the widget tree.
* **Widget identity.** Focus, scroll and caret state follow the call site, so you rarely write `.id(..)`. Inside loops, widgets are keyed by position; use `.id(n)` when rows can reorder.
* **Layout is built in.** Forms (`field`), tables (`table`), CRUD pages (`crud`) and wizards are ready-made blocks. An admin screen that is a few hundred lines in iced is tens of lines here; see BENCHMARKS.md for honest, measured line counts (everyday apps are about 3x shorter, apps made of the ready-made blocks 9 to 14x).
* **Lifetimes.** `view` returns an owned `impl View<Msg>`; there is no `Element<'a, Message>` to thread through helper functions. A helper is a plain `fn x(s: &State) -> impl View<Act<State>>`.

### A list with a filter: the todo app

The thing that surprises people most is how much of an iced todo app is plumbing: one message per edit, a `Vec` you map by hand, a `text_input` that needs `on_input` and `on_submit`. In Alloy the field is bound, the checkboxes edit the tasks in place, and "Clear done" is a one-liner.

```rust,ignore
// iced: ~65 lines of state, messages, update and view for the same screen
#[derive(Debug, Clone)]
enum Message {
    Draft(String),
    Add,
    Toggle(usize, bool),
    Filter(Filter),
    ClearDone,
}

fn update(s: &mut Todo, m: Message) {
    match m {
        Message::Draft(d) => s.draft = d,
        Message::Add => {
            let title = s.draft.trim().to_string();
            if !title.is_empty() { s.tasks.push((title, false)); s.draft.clear(); }
        }
        Message::Toggle(i, on) => s.tasks[i].1 = on,
        Message::Filter(f) => s.filter = f,
        Message::ClearDone => s.tasks.retain(|t| !t.1),
    }
}

fn view(s: &Todo) -> Element<'_, Message> {
    let open = s.tasks.iter().filter(|t| !t.1).count();
    let list = column(s.tasks.iter().enumerate().filter(|(_, t)| s.filter.keeps(t)).map(|(i, t)| {
        checkbox(&t.0, t.1).on_toggle(move |on| Message::Toggle(i, on)).into()
    }));
    column![
        text("Tasks").size(28), text(format!("{open} open")),
        row![
            text_input("What needs doing?", &s.draft).on_input(Message::Draft).on_submit(Message::Add),
            button("Add task").on_press(Message::Add),
        ].spacing(8),
        scrollable(list),
        row![pick_list(Filter::ALL, Some(s.filter), Message::Filter), button("Clear done").on_press(Message::ClearDone)],
    ].spacing(18).padding(24).into()
}
```

```rust,no_run
// Alloy: the whole screen
use alloy_ui::prelude::*;

alloy_ui::choices! { enum Filter { All, Open, Done } }

struct Todo { tasks: Vec<(String, bool)>, draft: String, filter: Filter }

impl Todo {
    fn add(&mut self) {
        let title = self.draft.trim().to_string();
        if !title.is_empty() {
            self.tasks.push((title, false));
            self.draft.clear();
        }
    }
}

fn view(s: &Todo) -> impl View<Act<Todo>> {
    let open = s.tasks.iter().filter(|t| !t.1).count();
    let shown = s.tasks.iter().enumerate().filter(|(_, t)| match s.filter {
        Filter::All => true,
        Filter::Open => !t.1,
        Filter::Done => t.1,
    });
    col![
        col![heading("Tasks"), caption(format!("{open} open")).dim()].gap(2.0),
        row![
            input(at!(s.draft)).hint("What needs doing?").submit(Todo::add).fill_w(),
            button("Add task").primary().act(Todo::add),
        ]
        .align_center()
        .fill_w(),
        scroll(col(shown.map(|(i, t)| check(&t.0, at!(s.tasks[i].1))))),
        row![
            options(s.filter).bind(|s: &mut Todo| &mut s.filter),
            button("Clear done").ghost().keep(at!(s.tasks), |t| !t.1),
        ],
    ]
    .gap(18.0)
    .pad(24.0)
    .fill()
}

fn main() {
    sketch(Todo { tasks: vec![("Write release notes".into(), false)], draft: String::new(), filter: Filter::All }, view)
        .run_sized("Todo", 420.0, 460.0)
}
```

`at!(s.tasks[i].1)` is a binding into the state: the checkbox reads and writes that field, so there is no `Toggle` message to write. Keep a real message when the event matters beyond "set this field" (an undo log, a network call, analytics).

### Background work and cancelling it

In iced a search-as-you-type box is a `Task::perform` plus bookkeeping so an old answer cannot overwrite a newer one. Alloy has two ways, depending on whether the result feeds a view or your state.

```rust,ignore
// iced
Message::Query(q) => {
    let (task, handle) = Task::perform(search(q), Message::Found).abortable();
    self.pending = Some(handle.abort_on_drop()); // dropping the old handle aborts the old task
    task
}
```

```rust,no_run
// Alloy, result goes to your state: a keyed command. A newer one with the same key cancels
// the running one, so only the latest answer ever arrives.
use alloy_ui::prelude::*;

#[derive(Default)]
struct Search { query: String, found: String }

#[derive(Clone)]
enum Msg { Query(String), Found(String), Stop }

async fn search(q: String) -> String {
    sleep(secs(0.3)).await; // stands in for a request; the cancelled one stops here
    format!("results for {q}")
}

impl App for Search {
    type Msg = Msg;
    fn update_cmd(&mut self, m: Msg) -> Cmd<Msg> {
        match m {
            Msg::Query(q) => {
                self.query = q.clone();
                Cmd::perform_keyed("search", async move { Msg::Found(search(q).await) })
            }
            Msg::Found(r) => { self.found = r; Cmd::none() }
            Msg::Stop => Cmd::cancel("search"), // its message never arrives
        }
    }
    fn view(&self) -> impl View<Msg> {
        col![
            row![button("rust").on(Msg::Query("rust".into())), button("iced").on(Msg::Query("iced".into())), button("Stop").on(Msg::Stop)],
            text(self.found.clone()),
        ]
        .gap(8.0)
        .pad(24.0)
    }
}
fn main() { Search::default().run("Search") }
```

When the data is only shown (a list, a detail pane), skip the messages: `resource_for(key, fetch)` refetches when the key changes and cancels the stale fetch, and `resource(fetch)` gives loading, error and retry for free ([async resources](06-async-resources.md)).

### Saved, restorable navigation

iced leaves routing to you. In Alloy `router(home)` is a stack of screens, `router_saved("my-app", home)` reopens where the person left off, and `.links(parse)` turns a deep link such as `myapp://note/3` into a screen ([navigation](14-navigation.md)).

### Things that do not map one to one

* iced's `Subscription` for window events or keyboard shortcuts: use `.shortcut("mod+k", ..)` on any node, or `every(period, ..)` for timers.
* iced's `Canvas` programs: use the `canvas` widget with its drawing API (see the widgets guide), or implement `Widget` for something fully custom.
* iced's per-widget `Style` closures: pick a `Look` and `.primary()`, `.ghost()`, `.danger()` variants; override a single node with `.bg(..)`/`.fg(..)` only when you must.

## From egui

egui redraws by running your UI code each frame; Alloy builds a tree from your state and keeps widget state for you. There is no `ctx`, no `ui` and no `.clicked()` to poll.

```rust,ignore
// egui
impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Counter");
            ui.label(format!("{} so far", self.n));
            ui.horizontal(|ui| {
                if ui.button("-").clicked() { self.n -= 1; }
                if ui.button("+").clicked() { self.n += 1; }
            });
            ui.text_edit_singleline(&mut self.name);
        });
    }
}
```

```rust,no_run
// Alloy
use alloy_ui::prelude::*;

#[derive(Default)]
struct MyApp { n: i64, name: String }

fn main() {
    sketch(MyApp::default(), |s| {
        col![
            heading("Counter"),
            text(format!("{} so far", s.n)),
            row![button("-").act(|s: &mut MyApp| s.n -= 1), button("+").act(|s: &mut MyApp| s.n += 1)],
            input(at!(s.name)),
        ]
        .gap(8.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

| egui | Alloy |
|---|---|
| `if ui.button("+").clicked() { .. }` | `button("+").act(\|s\| ..)`: the handler is attached, not polled |
| `ui.text_edit_singleline(&mut self.name)` | `input(at!(s.name))` |
| `ui.checkbox(&mut self.on, "label")` | `check("label", at!(s.on))` |
| `ui.add(egui::Slider::new(&mut v, 0.0..=1.0))` | `slider(0.0..=1.0, at!(s.v))` |
| `ui.horizontal(\|ui\| ..)`, `ui.vertical(..)` | `row![..]`, `col![..]` |
| `egui::SidePanel`, `CentralPanel` | `shell` / `app(..).page(..)` (sidebar and pages) |
| `egui::ScrollArea::vertical()` | `scroll(content)` |
| `ui.collapsing`, `ComboBox` | `reveal(show, ..)`, `pick`/`choose` |
| `egui::Window` | `Cmd::open_window` for an OS window, `popover`/`context_menu` for floating panels |
| `ctx.request_repaint()` | nothing: Alloy redraws after events, animations and [resources](06-async-resources.md), and is 0% CPU idle |
| `egui_extras::Table` | `table(..)` with sorting and selection; `vlist` for long lists |
| plot / persistence crates | `bar_chart`, `line_chart`, `sparkline`; `sketch(..).persist("name")` |
| egui's immediate state in `ui.memory` | `local(..)` / `store(..)` |

Things that behave differently:

* **No frame loop in your code.** Your view function runs after an event changes state, not 60 times a second. Do not read a clock inside it; use `.tick(period, ..)` or `animated(..)`.
* **Retained text input.** Text fields have real selection, undo, IME and right-to-left shaping instead of being redrawn from a `&mut String` each frame.
* **Native look by default.** Alloy draws everything itself like egui does, but with a consistent, themed design (light, dark, high contrast, density) rather than the egui style.

### A table and saved state

egui's `Table` needs a builder, a row closure and your own sorting; persistence needs the `persistence` feature and `eframe::Storage`. In Alloy the table is a call, and saving is `.persist("name")` on any `sketch` whose state derives `Serialize` and `Deserialize`:

```rust,ignore
// egui + egui_extras: columns, header, body, rows(...), manual sort, eframe storage
TableBuilder::new(ui).column(Column::auto()).column(Column::remainder())
    .header(20.0, |mut h| { h.col(|ui| { ui.strong("Name"); }); h.col(|ui| { ui.strong("Total"); }); })
    .body(|body| body.rows(18.0, self.rows.len(), |mut row| {
        let r = &self.rows[row.index()];
        row.col(|ui| { ui.label(&r.name); });
        row.col(|ui| { ui.label(format!("{:.2}", r.total)); });
    }));
```

```rust,no_run
// Alloy: sortable headers, selection and keyboard navigation come with it
use alloy_ui::prelude::*;
use std::sync::Arc;

#[derive(Clone)]
struct Order { name: String, total: f64 }

struct Orders { rows: Arc<Vec<Order>> }

fn main() {
    let rows = vec![Order { name: "Ada".into(), total: 12.5 }, Order { name: "Grace".into(), total: 99.0 }];
    sketch(Orders { rows: Arc::new(rows) }, |s: &Orders| {
        data_grid(&s.rows)
            .col("Name", |o: &Order| o.name.clone())
            .col("Total", |o: &Order| format!("{:.2}", o.total)).right()
            .fill()
    })
    .run("Orders")
}
```

### Habits to drop

* **Polling for clicks.** `if ui.button(..).clicked()` has no equivalent; the handler is attached to the button.
* **`ctx.request_repaint()` and `ctx.request_repaint_after`.** A timer is `every(period, ..)` or `.tick(..)`; animation is `animated(..)` and springs.
* **Storing UI state in your struct "because egui forgets it".** `local(..)` in the view keeps scroll, expanded flags and text drafts for you.
* **Ids for every widget.** Identity follows the call site; add `.id(n)` only for reorderable rows.

## From Slint

Slint splits an app in two: a `.slint` file (or `slint!` macro) with properties, callbacks and layout, and Rust code that fills the properties and handles the callbacks. Alloy keeps both halves in one Rust file: the property is a field, the callback is a closure, and the layout is the function that returns the view.

```text
// Slint (.slint)
import { Button, VerticalBox, HorizontalBox } from "std-widgets.slint";

export component Counter inherits Window {
    in-out property <int> value: 0;
    VerticalBox {
        Text { text: "\(root.value) so far"; }
        HorizontalBox {
            Button { text: "-"; clicked => { root.value -= 1; } }
            Button { text: "+"; clicked => { root.value += 1; } }
        }
    }
}
// Rust: slint::include_modules!(); let ui = Counter::new()?; ui.run()?;
```

```rust,no_run
// Alloy: one file, one language
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![
            text(format!("{n} so far")),
            row![button("-").act(|n| *n -= 1), button("+").act(|n| *n += 1)],
        ]
        .gap(14.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

A form with a list, where Slint needs a `ModelRc<VecModel<..>>` on the Rust side and a `for` loop in markup:

```text
// Slint
export component Tasks inherits Window {
    in-out property <[{title: string, done: bool}]> tasks;
    in-out property <string> draft;
    callback add(string);
    VerticalLayout {
        HorizontalLayout {
            LineEdit { text <=> root.draft; accepted(t) => { root.add(t); } }
            Button { text: "Add task"; clicked => { root.add(root.draft); } }
        }
        for t[i] in root.tasks : CheckBox { text: t.title; checked <=> t.done; }
    }
}
// Rust: let model = Rc::new(VecModel::from(vec![..])); ui.set_tasks(model.clone().into());
//       ui.on_add(move |t| model.push(TaskData { title: t, done: false }));
```

```rust,no_run
// Alloy
use alloy_ui::prelude::*;

#[derive(Default)]
struct Tasks { tasks: Vec<(String, bool)>, draft: String }

impl Tasks {
    fn add(&mut self) {
        let title = std::mem::take(&mut self.draft);
        if !title.trim().is_empty() { self.tasks.push((title, false)); }
    }
}

fn main() {
    sketch(Tasks::default(), |s: &Tasks| {
        col![
            row![
                input(at!(s.draft)).hint("What needs doing?").submit(Tasks::add).fill_w(),
                button("Add task").primary().act(Tasks::add),
            ],
            scroll(col(s.tasks.iter().enumerate().map(|(i, t)| check(&t.0, at!(s.tasks[i].1))))),
        ]
        .gap(12.0)
        .pad(24.0)
    })
    .run("Tasks")
}
```

| Slint | Alloy |
|---|---|
| `in-out property <T> x` | a field on your state; read it in the view |
| `<=>` two-way binding | `at!(s.x)` |
| `callback foo()` + `foo => { .. }` | `.act(\|s\| ..)` or a `Msg` variant handled in `update` |
| `ModelRc<VecModel<T>>`, `set_x(model)` | a plain `Vec<T>` (or `Arc<Vec<T>>` for big tables) |
| `for item[i] in model : Widget` | `col(items.iter().enumerate().map(..))`; `vlist(..)` when it is long |
| `if cond : Widget` | a Rust `if` in the view, or `reveal(cond, ..)` |
| `VerticalLayout`, `HorizontalLayout`, `GridLayout` | `col![]`, `row![]`, `stack(..)`; `.gap`, `.pad`, `.fill()` |
| `Text`, `Button`, `LineEdit`, `CheckBox`, `ComboBox` | `text`, `button`, `input`, `check`, `pick`/`choose` |
| `animate x { duration: 200ms }` | `animated(..)`, springs and `fade`/`reveal` |
| `@tr("..")` | `t!("key")` and plain-text catalogs ([localization](13-localization.md)) |
| `slint::invoke_from_event_loop`, `spawn_local` | `Cmd::perform(..)` or `resource(..)`; results arrive as messages on the UI thread |
| `Timer` | `every(period, ..)` or `.tick(..)` |
| `slint-viewer` live preview | `alloy dev` rebuilds on save and keeps `.persist()` state ([dev loop](12-dev-loop.md)) |
| Material/Fluent/Cupertino styles | `Look` presets and themes; light, dark, high contrast, density |
| `i-slint-backend-testing` | `Harness` finds things by their text ([testing](15-testing.md)) |

Things that behave differently:

* **No second language, no code generation.** Rust errors point at your view directly, rust-analyzer completes widget methods, and there is no build script. The cost: no separate designer-facing file. If you want one, Alloy's `.alloy` live views reload a markup-like file while the app runs (see the dev loop guide), and the VS Code extension highlights it.
* **State lives in Rust.** Slint properties are the source of truth and Rust pushes values in; in Alloy your struct is the source of truth and the view is recomputed from it.
* **Rust only.** Slint also targets C++, JavaScript and Python and runs on embedded devices and the web. Alloy is Rust and desktop (plus a mobile preview).
* **Licensing.** Slint is GPL, royalty-free with conditions, or commercial; Alloy is MIT or Apache-2.0.

## What you give up

* iced's, egui's and Slint's ecosystems: many third-party widgets, a game-engine or wgpu-callback integration story, (egui, Slint) web/WASM output and (Slint) embedded and MCU targets and other host languages. Alloy is desktop only (macOS, Linux X11 and Wayland, Windows) with a mobile preview, and no webview and no browser target.
* Maturity: iced and egui have years of use. Alloy's cross-platform behaviour is verified locally on Linux and by earlier CI runs on Windows; see the README for what has and has not run on each system.
* Direct `wgpu` access from a widget: custom widgets implement Alloy's `Widget` trait and draw with its commands.

## A porting checklist

1. Start with `sketch` and move one screen at a time; keep your domain structs unchanged.
2. Replace each `Message` that only sets a field with `at!(s.field)` bindings; keep messages for real events.
3. Move UI-only state (search text, expanded flags) into `local`.
4. Replace hand-rolled forms, tables and pagination with `field`, `table` and `crud`.
5. Turn spawned tasks that load data for a view into `resource(..)`; keep `Cmd::perform` for one-off actions.
6. Re-create tests with `Harness` and `snapshot`; run them with `alloy test` to get side-by-side images when a snapshot fails ([testing](15-testing.md)).
7. Replace your own "latest search wins" bookkeeping with `Cmd::perform_keyed` or `resource_for`.
8. If the app has several screens, move them into a `router` and add `router_saved` for restore and `.links(..)` for deep links.

Stuck on a pattern that is not here? Ask a coding agent: the repo's `AGENTS.md` teaches it Alloy's API and the common iced, egui and Slint translations.
