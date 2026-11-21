<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logo/alloy-lockup-dark.svg">
    <img alt="Alloy" src="assets/logo/alloy-lockup-light.svg" width="360">
  </picture>
</p>

<p align="center">
  <strong>A native Rust desktop UI framework. Your app is a state struct and a view function.</strong><br>
  GPU-drawn by Alloy's own renderer. No webview, no HTML, no JavaScript.
</p>

<p align="center">
  <a href="docs/guide/README.md">Guide</a> ·
  <a href="BENCHMARKS.md">Benchmarks</a> ·
  <a href="docs/WHY_ALLOY.md">Why Alloy</a> ·
  <a href="ARCHITECTURE.md">Architecture</a> ·
  <a href="DESIGN.md">Design</a>
</p>

<p align="center">
  <img alt="Alloy dashboard in dark mode" src="docs/media/alloy_rich_dashboard.png" width="860">
</p>

## What it is

Alloy takes the typed, Elm-style architecture of iced and the small, quiet app shell of Tauri, and drops everything else that gets in the way: no message enum unless you want one, no ids, no wrapper types, no browser engine. Every pixel comes from our own wgpu renderer, and the same code runs on macOS, Linux (X11 and Wayland) and Windows.

```rust
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| {
        col![
            heading("Counter"),
            text(format!("{n} so far")).dim(),
            row![
                button("Subtract").act(|n| *n -= 1),
                button("Add").primary().act(|n| *n += 1),
            ],
        ]
        .gap(14.0)
        .pad(24.0)
    })
    .run("Counter")
}
```

Handlers are closures over your state and every widget is a plain function. When an app grows, switch to the `App` trait with a `Msg` enum, background `Cmd`s and subscriptions. Both styles are fully typed.

## Try it now

The biggest demo is the **workbench**: a split pane, a 100,000-row list, tabs with overflow, a context menu, Markdown and a popover, in about 100 lines.

```sh
git clone https://github.com/adswill/alloy-ui
cd alloy-ui
cargo run --release -p alloy-ui --example workbench
```

More to poke at, all under `crates/alloy/examples/`: `showcase` (the widget tour), `notes`, `admin` (a CRUD screen from `derive(Record)`), `status` (async data, table and chart), `dashboard`, `editor`, `wizard`, `windows` (multi-window) and `motion` (animation). `examples/src/gallery.rs` is the lower-level widget gallery. Run any with `cargo run --release -p alloy-ui --example <name>`. Linux needs the usual GPU and windowing libraries (Vulkan or GL, X11 or Wayland). The screenshots are real renders from these examples and tests.

## Demos

<table>
<tr>
<td width="50%"><img src="docs/media/workbench.png" alt="Workbench: split pane, 100,000 rows, tabs, Markdown"><br><sub><b>Workbench</b>: split pane, 100,000 rows, tabs, Markdown</sub></td>
<td width="50%"><img src="docs/media/alloy_rich_notes_dark.png" alt="Notes app, dark"><br><sub><b>Notes</b>: search, editor, pin, undo, command palette</sub></td>
</tr>
<tr>
<td><img src="docs/media/alloy_rich_settings.png" alt="Settings panel"><br><sub><b>Settings</b>: forms with validation that waits until you are done</sub></td>
<td><img src="docs/media/grid_dark.png" alt="Data grid"><br><sub><b>Data grid</b>: sortable, selectable rows</sub></td>
</tr>
<tr>
<td><img src="docs/media/code_dark.png" alt="Code editor"><br><sub><b>Code editor</b>: a code editor widget</sub></td>
<td><img src="docs/media/charts.png" alt="Charts"><br><sub><b>Charts</b>: built-in charts, no extra crates</sub></td>
</tr>
<tr>
<td><img src="docs/media/board.png" alt="Kanban board"><br><sub><b>Kanban board</b>: cards in columns</sub></td>
<td><img src="docs/media/graph.png" alt="Node graph"><br><sub><b>Node graph</b>: nodes and connections</sub></td>
</tr>
<tr>
<td><img src="docs/media/calendar_month.png" alt="Calendar"><br><sub><b>Calendar</b>: month view with events</sub></td>
<td><img src="docs/media/canvas3d.png" alt="3D canvas"><br><sub><b>3D canvas</b>: drawn by the same renderer</sub></td>
</tr>
<tr>
<td><img src="docs/media/media.png" alt="Media player"><br><sub><b>Media</b>: images and media</sub></td>
<td><img src="docs/media/inspector.png" alt="Inspector"><br><sub><b>Inspector</b>: developer tooling</sub></td>
</tr>
<tr>
<td><img src="docs/media/mail_en.png" alt="Mail app in English"><br><sub><b>Localized</b>: English</sub></td>
<td><img src="docs/media/mail_ar.png" alt="Mail app in Arabic, mirrored"><br><sub><b>Localized</b>: Arabic, layout mirrors on its own</sub></td>
</tr>
</table>

### Themes

Light and dark follow the system. Palette presets ship in the box.

<table>
<tr>
<td><img src="docs/media/theme_alloy_dark.png" alt="Alloy dark"><br><sub>Alloy, dark</sub></td>
<td><img src="docs/media/theme_alloy_light.png" alt="Alloy light"><br><sub>Alloy, light</sub></td>
<td><img src="docs/media/theme_ember_dark.png" alt="Ember dark"><br><sub>Ember, dark</sub></td>
</tr>
<tr>
<td><img src="docs/media/theme_glacier_light.png" alt="Glacier light"><br><sub>Glacier, light</sub></td>
<td><img src="docs/media/theme_graphite_dark.png" alt="Graphite dark"><br><sub>Graphite, dark</sub></td>
<td><img src="docs/media/theme_signal_light.png" alt="Signal light"><br><sub>Signal, light</sub></td>
</tr>
</table>

## Same app, three ways

One dashboard, one notes app and one settings panel, written in Alloy, in idiomatic iced 0.14 and in Tauri (HTML, CSS and JavaScript), counted the same way.

| | Alloy | iced | Tauri |
|---|---|---|---|
| lines for all three apps | **355** | 1,487 | 765 |
| per app | 2.2 to 3.8x fewer than iced | | |

<table>
<tr>
<td><img src="docs/media/alloy_dashboard.png" alt="Alloy"><br><sub>Alloy</sub></td>
<td><img src="docs/media/iced_dashboard.png" alt="iced"><br><sub>iced</sub></td>
<td><img src="docs/media/tauri_dashboard.png" alt="Tauri"><br><sub>Tauri</sub></td>
</tr>
</table>

Method, sources and the rich versions of these apps are in [comparison/](comparison/README.md) and [BENCHMARKS.md](BENCHMARKS.md).

## How much less code, honestly

We wrote the same apps in idiomatic iced 0.14 (compiled, never a dependency) and in Alloy, and counted non-blank, non-comment lines after `cargo fmt` (`python3 crates/alloy/loc/count.py`).

| kind of app | example | iced | Alloy | fewer lines |
|---|---|---|---|---|
| everyday apps | pomodoro | 150 | 66 | 2.3x |
| | files | 222 | 69 | 3.2x |
| | wizard | 238 | 73 | 3.3x |
| | split | 100 | 47 | 2.1x |
| | **these four together** | 710 | 255 | **2.8x** |
| apps built from Alloy's ready-made blocks | admin (CRUD with `derive(Record)`) | 338 | 27 | 12.5x |
| | status board (async, table, chart) | 267 | 30 | 8.9x |
| | notes | 377 | 26 | 14.5x |

Read this as: **about 2.9x fewer lines on everyday apps, 9 to 14x when the app is mostly a table, a form or a list that a block already covers.** It is not a flat 10x. What is left in an ordinary app is your own logic, which no framework removes. The method, the iced sources and the audit of our own claims are in [BENCHMARKS.md](BENCHMARKS.md).

## Features

* **Concise or Elm, your choice.** `sketch(state, view)` with closures, or the `App` trait with a `Msg` enum, background `Cmd`s and subscriptions. Both are fully typed.
* **Batteries in the box.** Text, buttons, inputs, editor, sliders, pickers, date picker, sortable tables, lists, charts, wizards, `derive(Record)` CRUD screens, and a sidebar shell.
* **Forms that behave.** Validation messages wait until a field is touched or a submit fails.
* **Async without ceremony.** `resource(fetch)` gives loading, error with Retry, refresh and auto-refresh.
* **Finished-app details.** Shortcuts, a `mod+k` command palette, toasts with Undo, undo and redo, persisted state, multi-window.
* **Looks designed.** One teal-green accent, cool neutrals, small radii, WCAG AA contrast, light and dark following the system. See [DESIGN.md](DESIGN.md).
* **Small and quiet.** Counter binary 7.5 MB (`.deb` and `.rpm` 3.2 MB), 0% CPU when idle, first paint 87 to 106 ms under Xvfb (Linux, measured; see BENCHMARKS.md).
* **Speaks your users' language.** `t!("items", n = 3)` with plain-text catalogs, CLDR plurals, live language switching and right-to-left layouts that mirror on their own. See the [localization guide](docs/guide/13-localization.md).
* **Errors that explain themselves.** `state!`, `at!` and `derive(Record)` mistakes are reported on the exact token, with a suggestion.
* **Accessible.** An accessibility tree with AT-SPI on Linux.
* **Tauri-style shell.** `alloy` CLI to create, run and bundle (`.app`, `.dmg`, `.deb`, `.rpm`, AppImage, `.exe`, NSIS, MSI) and a capability file that denies everything not declared.
* **Testable.** A built-in headless `Harness` clicks, types and renders PNGs, and golden-image tests run without a window.

## Platform status

| platform | status |
|---|---|
| Linux (X11, Wayland) | built, tested and run locally: tests, headless rendering, bundling, AT-SPI |
| Windows | CI passed fmt, build, clippy and tests once; not yet run as an app |
| macOS | code compiles; CI has not run it yet, so treat it as unverified |

GitHub Actions is currently blocked by an account billing issue, so the Ubuntu and macOS matrix jobs have not completed. We only call something verified when a run has actually passed.

## Install

Alloy is not on crates.io yet. Use the git repository:

```sh
cargo install --git https://github.com/adswill/alloy-ui alloy-ui-cli   # the `alloy` tool
alloy new hello --identifier com.example.hello   # or --template dashboard | editor | settings | localized
cd hello && alloy dev
```

or add it to an existing project:

```toml
[dependencies]
alloy-ui = { git = "https://github.com/adswill/alloy-ui" }
```

Linux needs the usual GPU and windowing libraries (Vulkan or GL, X11 or Wayland). The screenshots are real renders from these examples and tests. Then follow the [guide](docs/guide/README.md).

## Crates

| crate | what it is |
|---|---|
| `alloy-ui` | the facade: prelude, concise app API, shell wiring (start here) |
| `alloy-ui-core` | geometry, `Widget`/`Element`, events, draw commands, runtime loop, multi-window |
| `alloy-ui-layout` | flexbox-style layout engine (Fill/Shrink/Fixed, padding, spacing, alignment) |
| `alloy-ui-render` | wgpu renderer, CPU reference rasteriser, text (bundled IBM Plex), golden-image harness |
| `alloy-ui-theme` | light/dark palettes, type scale, spacing, motion (WCAG AA tested) |
| `alloy-ui-widgets` | text, button, inputs, editor, table, list, charts, overlays, focus, accessibility tree |
| `alloy-ui-platform` | windows, menus, tray, dialogs, clipboard, drag and drop, system theme, IME, screen readers |
| `alloy-ui-ipc` | typed background tasks and the capability/permissions config |
| `alloy-ui-macros` | `#[derive(Record)]` |
| `alloy-ui-cli` | `alloy` build and bundle tool (.app, AppImage/.deb/.rpm, .exe/.msi) |

## Docs

* [Guide](docs/guide/README.md) (the cookbook, a documentation website and a VS Code extension are in review and will be linked here when merged): getting started, state and bindings, layout, widgets, forms, async, multi-window, shortcuts and undo, theming, bundling, capabilities. Every snippet is compiled as a doc-test.
* [ARCHITECTURE.md](ARCHITECTURE.md) (design, and what we learned from iced and Tauri), [DECISIONS.md](DECISIONS.md) (every design choice with reasons), [DESIGN.md](DESIGN.md) (visual rules), [BENCHMARKS.md](BENCHMARKS.md) (measured numbers), [docs/WHY_ALLOY.md](docs/WHY_ALLOY.md).

Licensed under MIT OR Apache-2.0.
