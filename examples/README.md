# Alloy examples

`counter`, `todo`, `text_editor`, `settings`, `dashboard`. Run one with
`cargo run -p alloy-ui-examples --bin counter -- <out-dir>`; it drives a scripted
session through the real runtime and writes `<name>.png`.

UI is built from `src/kit.rs`, a stand-in for `alloy-ui-widgets` (not landed
yet). It has no hover states, slider dragging or shaped text measurement.
Swap in the real widgets and the app logic stays the same.
