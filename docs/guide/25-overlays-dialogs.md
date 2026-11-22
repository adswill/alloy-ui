# Menus, popovers and dialogs

## Menus and popovers work inside scroll areas and list rows

A `context_menu` or `popover` can sit inside `scroll(..)` or in a `vlist` row. Its panel is
drawn above everything, and mouse clicks on it reach it (they used to be dropped, so such
menus only worked from the keyboard). A `vlist` row also receives right and middle clicks, so
`context_menu(row_view, [..])` as a row is the way to give each row a menu.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Files { trashed: u32 }

let _ = sketch(Files::default(), |s: &Files| {
    col![
        text(format!("{} trashed", s.trashed)),
        vlist(500, |i| context_menu(
            text(format!("file {i}")),
            [menu_item("Move to trash").act(|s: &mut Files| s.trashed += 1)],
        ))
        .fill_h(),
    ]
});
```

## `modal`: a dialog

`modal(open, content)` shows a dimmed scrim and a centred card. Escape or a click on the
scrim runs `.close(..)`; Tab stays inside the card; focus moves into it on open and back to
where it was on close; it fades and rises in and out (jumping when reduced motion is on).
You keep `open` in your state. It is drawn on the overlay layer, so you can write it right
next to the button that opens it, even inside a scroll area. Clicks and the mouse wheel never
reach the page under it.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Page { confirm: bool, deleted: bool }

let _ = sketch(Page::default(), |s: &Page| {
    col![
        button("Delete").act(|s: &mut Page| s.confirm = true),
        modal(s.confirm, col![
            heading("Delete this?"),
            row![
                button("Cancel").act(|s: &mut Page| s.confirm = false),
                button("Delete").primary().act(|s: &mut Page| {
                    s.deleted = true;
                    s.confirm = false;
                }),
            ].gap(8.0),
        ].gap(12.0))
        .width(360.0)
        .close(|s: &mut Page| s.confirm = false),
    ]
});
```

With a view-local flag use `.local(&open)` instead of `.close(..)`. Limit: the page under the
scrim still draws hover highlights from the pointer position.

## `portal`: anything above the window

`portal(open, content)` hosts any view on the overlay layer, laid out over the whole window,
whatever clips or scrolls around it. Use it for a `sheet`, a lightbox or a custom panel.
While open it takes every press and the wheel; there is no scrim and no Escape handling, so
give the content its own close button.

```rust
use alloy_ui::prelude::*;

let _ = sketch(false, |open: &bool| {
    col![
        button("Preview").act(|s: &mut bool| *s = true),
        portal(*open, col![
            text("Full window preview"),
            button("Close").act(|s: &mut bool| *s = false),
        ].pad(24.0).fill()),
    ]
});
```
