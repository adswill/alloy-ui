# Text input and accessibility

## Programmatic selection

```rust,ignore
input(at!(s.q))
    .id(7)
    .select_all_on_focus()          // click or Tab selects everything
    .selection(0..s.q.len().min(3)) // char range, applied when the range value changes
```

`.selection(range)` clamps to the text; an empty range places the caret. A request is applied
once per distinct range, so the user can move the selection afterwards. From app code, use the
handle: `ui().set_focus(Some(7)); ui().select_all_text(7);` or `ui().select_text(7, 2..5)`;
read it back with `ui().text_selection(7)`.

## Escape in text inputs

```rust,ignore
input(at!(s.q)).hint("Search").escape(|s: &mut State| s.q.clear());
// or with messages: .on_escape(Msg::ClearSearch)
```

Escape is consumed and sent to your handler only when one is set (the field keeps focus; call
`ui().set_focus(None)` to blur). Without a handler Escape still blurs the field.

## Slider accessible name and value

```rust,ignore
slider(0.0..=100.0, at!(s.zoom)).label("Grid zoom").unit("%")
```

`.label(..)` also names the slider for screen readers; with `.unit(..)` the accessible value
reads `25 %` instead of `25`. The lower-level `widgets::Slider` has `.label(..)` (accessible
only) and `.value_text(..)`.
