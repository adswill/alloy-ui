# Performance and density (Steam feedback, area P)

## Layout and text measure allocate far less

A frame used to rebuild the layout node tree, re-measure every label into a fresh `Vec<String>`, and re-wrap text with a `format!` per word. Now:

* `Text` measures and draws line by line without allocating (`for_each_wrapped` hands out slices of the content).
* `arrange_widget` memoises trees of 24 or more nodes by (fingerprint of every node's style and every leaf's measured size, bounds). An unchanged page at an unchanged size returns the stored layout. Output is identical to the uncached path.
* `distribute` skips its scratch vectors for zero or one flexible child (a row with one `space()`).

On a page of 60 rows (text, caption, `space()`, button) in `crates/alloy/tests/steam_feedback_perf.rs`, one idle frame went from 3299 to about 1200 allocations; layout plus draw alone went from about 2650 to about 370 (the rest is your own `view`).

## Density

One switch for the whole window:

```rust,ignore
sketch(state, view).density(Density::Compact)   // Compact, Comfortable (default), Spacious
```

`alloy_ui::density()` returns the current value inside a view; `density().px(28.0)` scales a size of your own (a fixed row height) with it. Spacing tokens, default list/table row heights and `tree` rows follow it. `Sketch::density` wins over the density of a `Look` and over `set_dark`.

Headless phase log: see `Harness::phases` and `Harness::perf_mark` (area R).
