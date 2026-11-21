# Alloy guide

Read in order, or jump to what you need. Every code block here is compiled as a doc-test by `cargo test -p alloy-ui --doc`, so the snippets stay true.

1. [Getting started](01-getting-started.md): install the tool, your first app, testing without a screen
2. [State and bindings](02-state-and-bindings.md): handlers, `at!`, `local`, persistence
3. [Layout](03-layout.md): `col!`, `row!`, gaps, fill, scroll
4. [Widgets](04-widgets.md): the catalogue
5. [Records and forms](05-records-and-forms.md): `derive(Record)`, `crud`, validation, wizards
6. [Async resources](06-async-resources.md): loading, errors, retry, refresh, keyed data and cancellation
7. [Multi-window](07-multi-window.md)
8. [Shortcuts, palette, toasts, undo](08-shortcuts-palette-toasts-undo.md)
9. [Theming](09-theming.md)
10. [Bundling](10-bundling.md)
11. [Capabilities and IPC](11-capabilities-and-ipc.md)
12. [The dev loop](12-dev-loop.md): `alloy dev`, live views that reload on save, the F12 inspector
13. [Localization](13-localization.md): `t!`, plurals, switching language, right-to-left
14. [Navigation](14-navigation.md): routes, back and forward, breadcrumbs
15. [Testing](15-testing.md): `Harness`, fill, expect, snapshots
16. [Coming from iced or egui](16-coming-from-iced-and-egui.md): side-by-side code and a porting checklist
17. [Shipping](17-shipping.md): signing, notarisation, MSIX and Flatpak, signed auto-update, crash reports

Design background: [ARCHITECTURE.md](../../ARCHITECTURE.md), [DECISIONS.md](../../DECISIONS.md), [DESIGN.md](../../DESIGN.md). API reference: `cargo doc --open -p alloy-ui`.
