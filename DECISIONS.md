# Decisions

1. **Crate names**: `alloy-ui-{core,layout,render,theme,widgets,platform,ipc,cli}`; all returned 404 on crates.io on 2026-11-20. Directory names are short (`crates/core`).
2. **Geometry, DrawCommand and Event live in core.** Layout, render and widgets all need them; a separate types crate adds a hop for no gain yet.
3. **Widgets draw into a `DrawList`, not a generic renderer.** Keeps the renderer swappable and tests headless. Cost: no per-widget custom GPU pipelines; revisit if needed.
4. **Zero dependencies in the contract stubs.** wgpu, winit, cosmic-text etc. are added by the crates that own them.
5. **Release profile is size-first** (opt-level z, fat LTO, panic abort, strip) to hit the 10 MB counter target.
6. **Visual direction**: follow Impeccable's AI-slop catalogue; see DESIGN.md.
7. **Facade crate `alloy-ui`** (`crates/alloy`) owns the glue: shell events -> `Runtime` -> `WindowRenderer`, background futures through `ipc::TaskRunner` whose waker is the shell proxy. Apps depend on it only. Layouter is a parameter so core stays independent of the layout crate.
8. **Platform PR #1 merged locally by the lead**, verified fmt/clippy/tests on Linux only; Windows and macOS await CI.
