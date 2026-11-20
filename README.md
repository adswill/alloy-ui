# Alloy

A native, cross-platform Rust desktop UI framework. Elm-style architecture, our own wgpu renderer and widget set, a small Tauri-style app shell. No webview, no HTML, CSS or JavaScript, and no existing GUI toolkit underneath.

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

[ARCHITECTURE.md](ARCHITECTURE.md) (design and what was learned from iced and Tauri), [DECISIONS.md](DECISIONS.md) (every design choice with reasons), [DESIGN.md](DESIGN.md) (visual rules), [BENCHMARKS.md](BENCHMARKS.md) (measured numbers), [docs/WHY_ALLOY.md](docs/WHY_ALLOY.md).

## Verification status

Everything above is built and tested on Linux. macOS and Windows code compiles and is covered by the CI matrix, but CI runs have not completed yet, so treat those platforms as unverified until they do. Screen readers: AT-SPI is tested end to end on Linux; VoiceOver and Narrator support is compiled only.

Licensed under MIT OR Apache-2.0.
