# Decisions

1. **Crate names**: `alloy-ui-{core,layout,render,theme,widgets,platform,ipc,cli}`; all returned 404 on crates.io on 2026-11-20. Directory names are short (`crates/core`).
2. **Geometry, DrawCommand and Event live in core.** Layout, render and widgets all need them; a separate types crate adds a hop for no gain yet.
3. **Widgets draw into a `DrawList`, not a generic renderer.** Keeps the renderer swappable and tests headless. Cost: no per-widget custom GPU pipelines; revisit if needed.
4. **Zero dependencies in the contract stubs.** wgpu, winit, cosmic-text etc. are added by the crates that own them.
5. **Release profile is size-first** (opt-level z, fat LTO, panic abort, strip) to hit the 10 MB counter target.
6. **Visual direction**: follow Impeccable's AI-slop catalogue; see DESIGN.md.
7. **Facade crate `alloy-ui`** (`crates/alloy`) owns the glue: shell events -> `Runtime` -> `WindowRenderer`, background futures through `ipc::TaskRunner` whose waker is the shell proxy. Apps depend on it only. Layouter is a parameter so core stays independent of the layout crate.
8. **Platform PR #1 merged locally by the lead**, verified fmt/clippy/tests on Linux only; Windows and macOS await CI.
9. **Contract change (lead):** `DrawCommand::Text` gained `weight` and `line_height`; new `DrawCommand::Line` for strokes (renderer draws it as a chain of round dots for now; a proper SDF segment is a later optimisation). `Event::Tick(ms)` plus `Widget::animating()` give widgets a timer only while something animates; `Outcome.tick` tells the shell to schedule ~16 ms ticks, so an idle app has no timer.
10. **Weight/line_height are accepted but not yet honoured by the renderer** (ignored with `_`); tracked for the render follow-up along with real text metrics.

## Text editing follows grapheme clusters, UAX #29 words and per-OS shortcuts

- Caret and selection stay `char` indices (so the controlled-value API is unchanged) but every move, delete and hit test lands on a grapheme-cluster boundary. Depends on `unicode-segmentation` (data tables only, no GUI code), the one new dependency in `alloy-ui-widgets`.
- Words use UAX #29: `don't` and `foo_bar` are one word, and each CJK ideograph is its own stop (no dictionary segmentation yet).
- Hit testing binary-searches boundaries with prefix widths (O(log n) measurements) rather than measuring every prefix.
- Shortcut key: Cmd on macOS, Ctrl elsewhere (Super and Ctrl+Alt/AltGr never act). Word moves: Option on macOS, Ctrl elsewhere. On macOS Ctrl+A / Ctrl+E mean line start / end. Chosen at compile time with `cfg!(target_os)`.
- While an IME preedit is active the field consumes every key except Tab, so Backspace, arrows, Enter and shortcuts cannot touch committed text.
- Bidi is basic: a line whose first letter is right-to-left makes plain Left/Right arrows move logically forward/backward respectively. Word and line moves stay logical, and the caret's drawn position is still LTR-only. Full visual bidi needs `unicode-bidi` plus renderer support.
- The editor selects a word on double click and a line on triple click (500 ms, 4 px). The single-line input does not yet. A lone CR on paste is a newline.
