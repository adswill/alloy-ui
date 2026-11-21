# Coming from iced or egui

Alloy keeps iced's architecture (state, messages, view, native GPU drawing) and drops most of its ceremony. If you know egui, the biggest change is that the view is a value you return, not a series of calls on a `Ui`. This page maps what you already know. It is a guide to getting started, not a claim that the others are wrong: iced and egui are excellent and have things Alloy does not (see "What you give up").

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

## What you give up

* iced's and egui's ecosystems: many third-party widgets, a game-engine or wgpu-callback integration story, and (egui) web/WASM output. Alloy is desktop only (macOS, Linux X11 and Wayland, Windows) with no webview and no browser target.
* Maturity: iced and egui have years of use. Alloy's cross-platform behaviour is verified locally on Linux and by earlier CI runs on Windows; see the README for what has and has not run on each system.
* Direct `wgpu` access from a widget: custom widgets implement Alloy's `Widget` trait and draw with its commands.

## A porting checklist

1. Start with `sketch` and move one screen at a time; keep your domain structs unchanged.
2. Replace each `Message` that only sets a field with `at!(s.field)` bindings; keep messages for real events.
3. Move UI-only state (search text, expanded flags) into `local`.
4. Replace hand-rolled forms, tables and pagination with `field`, `table` and `crud`.
5. Turn spawned tasks that load data for a view into `resource(..)`; keep `Cmd::perform` for one-off actions.
6. Re-create tests with `Harness` and `snapshot`.
