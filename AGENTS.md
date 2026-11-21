# Using Alloy: instructions for coding agents

Read this before writing or changing code that uses `alloy-ui`. It is short on purpose. Every Rust snippet below is compiled by this repository's doc-tests, so it matches the current API. When something here and your memory of another GUI library disagree, trust this file, then the compiler.

## What Alloy is

A native, GPU-drawn Rust desktop UI framework (macOS, Linux X11 and Wayland, Windows). It has iced's shape (state, view, messages) with far less ceremony, and Tauri's shell ideas (tiny binaries, tray, menus, dialogs, a capability file). **There is no webview, no HTML, no CSS and no JavaScript, and none must be added.** Everything is Rust. The crate is `alloy-ui`; the tool is `alloy` (`alloy-ui-cli`).

```toml
# Cargo.toml of an app that uses Alloy
[dependencies]
alloy-ui = { git = "https://github.com/adswill/alloy-ui", branch = "main" }
```

Never depend on iced, egui, Slint, taffy, wry or Tauri alongside it. Do not write `use alloy_ui::*;` in app code; use the prelude.

## The shape of every app

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default)]
struct Counter { n: i64, name: String }

fn main() {
    sketch(Counter::default(), |s: &Counter| {
        col![
            heading("Counter"),
            text(format!("{} so far", s.n)).dim(),
            input(at!(s.name)).hint("Your name"),
            row![
                button("Subtract").act(|s: &mut Counter| s.n -= 1),
                button("Add").primary().act(|s: &mut Counter| s.n += 1),
            ],
        ]
        .gap(14.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

Rules of thumb:

1. **State is a plain struct.** `sketch(state, view)` is the default. The view is a function from `&State` to a view; it runs again after every handler and Alloy draws the difference. There is no `update` function and no message enum to write.
2. **Handlers are closures over `&mut State`** attached with `.act(|s| ..)`. Annotate the parameter (`|s: &mut Counter|`) when inference needs help. Extract repeated logic into `impl State { fn add(&mut self) }` and pass `State::add`.
3. **Fields bind two-way with `at!(s.field)`**: `input(at!(s.name))`, `check("On", at!(s.on))`, `slider(0.0..=1.0, at!(s.v))`, `number(at!(s.n))`, `options(s.filter).bind(..)`. Do not invent a message per field. `at!` takes a path starting at the `s` your view received: `at!(s.tasks[i].1)` works.
4. **Use the `App` trait only when you want messages**: a message log, `update_cmd` returning a `Cmd`, or several windows. `type Msg: Clone + Send`; `fn update(&mut self, m)` or `fn update_cmd(&mut self, m) -> Cmd<Msg>`; `fn view(&self) -> impl View<Msg>`; widgets then use `.on(Msg::X)` instead of `.act(..)`.
5. **UI-only state lives in the view**: `let q = local(String::new());` keeps a search box, a sort order or an open panel without a field. `store(..)` is the saved version.
6. **Widget identity follows the call site.** Do not add ids. Inside a loop where rows can reorder, add `.id(n)`.
7. **Pixel sizes are `f32`**: `.gap(14.0)`, `.pad(24.0)`, `.w(170.0)`. Passing an integer is a compile error.

## Layout

`col![..]` stacks top to bottom, `row![..]` left to right. `.gap(px)` between children, `.pad(px)` inside, `.align_center()`, `.fill_w()` / `.fill_h()` / `.fill()` to take the remaining room, `.w(px)` / `.h(px)` for a fixed size; items shrink to fit by default. `space()` is a flexible spacer (`row![a, space(), b]` pushes `b` right), `rule()` a divider, `scroll(view)` a scroll area, `col(iter)` / `row(iter)` build from an iterator.

## Widgets you will reach for

| you want | use |
|---|---|
| text | `text(s)`, `heading(s)`, `caption(s)`, `.dim()`, `.accent()`, `.danger()`, `.success()` |
| button | `button("Go")`, `.primary()`, `.ghost()`, `.enabled(bool)`, `.confirm("Sure?")` |
| inputs | `input(at!(s.x))`, `.hint(..)`, `.secure()`, `.submit(f)`; `editor(..)` multi-line; `number(..)` |
| choices | `check`, `radios`, `options(s.filter)`, `pick` (dropdown), `tabs`, `choose` |
| numbers | `slider`, `stepper`, `progress`, `stat`, `bars`, `bar_chart`, `line_chart`, `sparkline` |
| collections | `table(&rows).col("Name", |r| ..)`, `data_grid(&rows)` (100k+ rows), `vlist`, `list`, `tree` |
| screens | `app("Title").page("Name", || ..)` (sidebar app), `router(home)` (drill-down), `wizard(..)` |
| forms and CRUD | `#[derive(Record)]` plus `crud(..)`, `field(..)`, validation |
| floating | `popover`, `context_menu`, toasts, `palette` |
| anything custom | implement `Widget` and wrap it with `custom(element)` |

Enums for radio groups and sidebars: `choices! { enum Filter { All, Open, Done = "Finished" } }`.

## Background work

* Data a view shows: `resource(fetch)` (an `async fn() -> Result<T, E>`) gives loading, error with Retry, refresh and `.auto(secs(10.0))`. `resource_for(key, fetch)` refetches when `key` changes and cancels the stale fetch.
* A result that must reach your state (the `App` trait): return `Cmd::perform(async { Msg::Done(..) })` from `update_cmd`. For search-as-you-type use **`Cmd::perform_keyed("search", fut)`**: a newer future with the same key cancels the running one, and `Cmd::cancel("search")` stops it. Never write your own "ignore stale result" counter.
* Futures run on a small built-in executor; `sleep(secs(0.3)).await` works with no runtime. A future needing tokio must be wrapped by the app.
* Timers: `.tick(period, |s| ..)` on a sketch, or `every(period, || msg)` in `subscriptions()`. Never read a clock inside `view`.

## Navigation and deep links

```rust
use alloy_ui::prelude::*;

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
enum Route { Inbox, Mail(u32) }

fn view(_: &()) -> impl View<Act<()>> {
    // Remembers the stack across restarts; `myapp://mail/7` opens message 7.
    let nav = router_saved("my-mail", Route::Inbox).links(|url| match link_path(url).as_slice() {
        [kind, id] if kind == "mail" => id.parse().ok().map(Route::Mail),
        _ => None,
    });
    nav.show(|route| match route {
        Route::Inbox => button("Open 7").act(move |_| nav.push(Route::Mail(7))).into(),
        Route::Mail(id) => col![nav.back_button(), heading(format!("Message {id}"))].into(),
    })
}
# fn main() { let _ = sketch((), view); }
```

`router(home)` is the non-saving version. `push`, `replace`, `back`, `forward`, `reset`, `breadcrumbs` exist; `show` also binds alt+left and alt+right. Register the scheme with `.url_scheme("myapp")` on a sketch.

## Look

`set_look(Look::dark().accent(..).density(..))`, `set_dark(true)`, `theme_switch()`. Pick a `.primary()` / `.ghost()` / `.danger()` variant instead of per-widget colours. Text uses the bundled Alloy Sans and Alloy Mono; do not load fonts yourself. Translated text is `t!("key", name = value)` with plain-text catalogs (`i18n::add("en", ..)`). Avoid the usual "AI-made UI" look: no gratuitous gradients, no beige, consistent spacing, a real hierarchy of heading, body and caption.

## Test it, headless

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Form { name: String }

let app = sketch(Form::default(), |s: &Form| {
    col![
        input(at!(s.name)).hint("Name"),
        text(format!("Hello {}", s.name)),
        button("Clear").act(|s: &mut Form| s.name.clear()),
    ]
});
let mut ui = Harness::new(app, 360.0, 200.0);
ui.fill("Name", "Ada").expect("Hello Ada");
ui.click("Clear").expect_no("Hello Ada");
```

`Harness` runs the real runtime with no window. Everything is found by the text on screen: `click`, `fill`, `type_text`, `key`, `chord("mod+s")`, `hover`, `scroll`, `advance(ms)`, `settle()` (waits for resources and `Cmd` futures), `open_url("myapp://..")`, `expect`, `expect_no`, `expect_containing`, `count`, `snapshot("name")` (golden image in `tests/snapshots/`). Write a test for every behaviour you add, and run it:

```sh
cargo test                  # or:
alloy test                  # failing snapshots become expected | actual | diff images in target/alloy-diff
alloy test --update         # accept new snapshots after you looked at the diff
```

Snapshots depend on fonts, so only generate them on the OS CI uses; assert on text for anything that must hold everywhere.

## Commands

| command | does |
|---|---|
| `alloy new NAME --template counter\|dashboard\|editor\|settings\|localized` | a starter app |
| `alloy dev` | build, run and restart on every save; `.persist("name")` keeps state; F12 opens the inspector |
| `alloy check` | validate `[package.metadata.alloy]`, icons and the capability file |
| `alloy test` / `cargo alloy test` | tests with snapshot diffs |
| `alloy bundle [--format app,dmg,deb,rpm,appimage,exe,nsis,msi,msix,flatpak]` | installers |
| `alloy lsp` | language server for `.alloy` live views (the VS Code extension uses it) |

Live views: a screen's layout, copy and theme can live in `ui/home.alloy` (indented markup, `live!("ui/home.alloy").var(..).on("name", ..)`) and reload on save without a restart. Rust behaviour stays in Rust.

## Translating from other toolkits

* **iced**: `Message` plus `update` for a field edit becomes `at!(s.field)`; `Task::perform` becomes `Cmd::perform` / `Cmd::perform_keyed` or `resource`; `.spacing/.padding` become `.gap/.pad`; `Length::Fill` becomes `.fill()`.
* **egui**: no `ui`, no polling `.clicked()`; the handler is attached with `.act`. `ctx.request_repaint()` is unnecessary.
* **Slint**: a `property` is a struct field, a `callback` is a `.act` closure, `for x in model` is `col(iter)`, `<=>` is `at!`.

The full tables are in `docs/guide/16-coming-from-iced-and-egui.md`.

## Mistakes agents make here

* Adding a message enum and `update` for something `at!` or `.act` already does.
* Writing `button("x").on_press(..)`, `.spacing(..)`, `.padding(..)`, `Column::new()`, `ui.label(..)`. Those are other libraries; use `.act`, `.gap`, `.pad`, `col!`, `text`.
* Integer pixel sizes (`.gap(8)`); use `8.0`.
* Storing search text or an "expanded" flag in the state struct instead of `local(..)`.
* Spawning threads for data a view shows instead of `resource(..)`.
* Calling `block_on` or `std::thread::sleep` inside a handler: it freezes the UI. Use a `Cmd` or `resource`.
* Reading the clock or a random number inside `view`: the view must be a function of state.
* Claiming something works on Windows or Linux because it works on your machine. CI runs all three; say what ran.

## Working on the Alloy repository itself

* Layout: `crates/alloy` is the facade you use; `core` (runtime, `.alloy` parser), `layout`, `render` (CPU and wgpu), `widgets`, `platform` (one module per OS behind traits), `ipc` (tasks, capabilities), `ship` (updates, crash reports), `cli` (`alloy`). Examples are in `crates/alloy/examples`, guides in `docs/guide` (compiled as doc-tests), recipes in `docs/cookbook`.
* Before a pull request: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. Public items need rustdoc (`missing_docs` is on), `unsafe` needs a comment saying why.
* Hard rules: no webview and no web tech, no dependency on another GUI toolkit or layout engine, platform code only inside platform modules, and honest line counts in `BENCHMARKS.md`. Record design decisions in `DECISIONS.md`.
