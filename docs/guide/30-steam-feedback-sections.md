# Sections and tooltips

## `fold_section`: a collapsible section that remembers

`fold_section(id, title, body)` draws a chevron and a title over a body that opens and closes
with a size animation. The open state is saved under `id` (like `store_as`), so it survives
rebuilds and page changes, is shared by every section using the same `id`, and is persisted
with the app's other `store`s. `fold_section_open` starts open. `accordion` is unchanged (its
state is local to the call site).

```rust,ignore
fold_section("advanced", "Advanced", col![check("Verbose", at!(s.verbose))])
```

## Placeable tooltips

`.tip(text)` still works. `.tooltip(text)` returns a builder with a side, an offset and a
delay, so a tooltip need not cover the next control. The bubble is clamped to the window.

```rust,ignore
button("Library")
    .tooltip("Your games")
    .tooltip_at(TipSide::Right, 8.0) // Below (default), Above, Left, Right
    .tooltip_delay(200)              // ms of resting pointer; default 500
```

The vgrid "focusable tap steals arrow focus" item belongs to the lists area and is not handled here.
