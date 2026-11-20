# Alloy examples

`counter`, `todo`, `text_editor`, `settings`, `dashboard`, each a few dozen lines against
`alloy_ui::prelude` (closure-style `sketch`, no message enums).

```sh
cargo run -p alloy-ui-examples --bin dashboard                 # opens a native window
cargo run -p alloy-ui-examples --bin dashboard -- --headless out.png --click "12 days"   # PNG, no window
cargo test -p alloy-ui-examples                                # scripted input + golden images
UPDATE_GOLDEN=1 cargo test -p alloy-ui-examples                # regenerate goldens after a visual change
```

The dashboard uses the widgets' `BarChart` and the facade's `table`; the editor is the real
`TextEditor`. Goldens live in `tests/golden` and double as the visual review surface.
