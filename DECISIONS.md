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
10. **Weight, line height and family are honoured by the renderer** (CPU and GPU share one shaping path). `DrawCommand::Text` carries `weight`, `line_height` and `family` (Sans or Mono); `TextMeasurer` measures with the same shaper so layout matches drawing.
11. **Licence: MIT OR Apache-2.0** (the Rust norm; permissive, unlike a GPL/commercial split). `LICENSE-MIT` and `LICENSE-APACHE` at the root; every crate inherits the workspace licence field.
12. **Bundled fonts**: IBM Plex Sans (400/500/600/700) and Plex Mono (400/500), OFL 1.1, so text is identical on every OS. Inter and Geist are avoided on purpose (listed as AI-look tells in DESIGN.md).
13. **CJK and emoji fallback uses system fonts**, not bundled ones: a CJK font is 5-15 MB and would break the 10 MB binary target. cosmic-text falls back per glyph; if no system font covers a script the glyph is missing. Revisit with an optional `cjk-fonts` feature.
14. **CI**: integration-branch runs are never cancelled, and `push` triggers only on main and the lead branch, because runners start 10-15 minutes late and cancel-on-push meant nothing finished.
