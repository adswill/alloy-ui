# Hover, focus and keyboard

## Hover

`.hover_bg(color)` fades a colour in under the pointer. To react in state, use `.on_hover`:

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct App1 { paused: bool }

fn main() {
    let _ = sketch(App1::default(), |s: &App1| {
        text(if s.paused { "Paused" } else { "Playing" })
            .pad(12.0)
            .on_hover(|s: &mut App1, over| s.paused = over)
    });
}
```

To read hover while building the view, make a flag with `hovered()` and attach it with `.watch_hover(flag)`; read it with `flag.get()`. Inside lists use `hovered_for(item_id)` so state follows the item, not its position. Apps with their own message type use `.on_hover_msg(|over| Msg::Hover(over))`.

## Focusable taps

`.tap(|s| ..)` makes any view clickable and reachable with Tab and Shift+Tab. It draws a focus ring, and Enter or Space press it. In a list that reorders use `.tap_for(id, ..)`. With your own messages use `.on_tap(msg)`.

## Arrow keys in grids

Wrap a grid of taps in `.arrow_nav()`: the arrow keys move focus to the nearest cell in that direction. Cells that are not drawn (off-screen rows of a virtualised grid) cannot be reached by arrows.

```rust
use alloy_ui::prelude::*;

fn main() {
    let _ = sketch(0, |_: &i32| {
        col![
            row![text("a").pad(8.0).tap(|n: &mut i32| *n += 1), text("b").pad(8.0).tap(|n: &mut i32| *n += 2)],
            row![text("c").pad(8.0).tap(|n: &mut i32| *n += 3), text("d").pad(8.0).tap(|n: &mut i32| *n += 4)],
        ]
        .arrow_nav()
    });
}
```

## Typing and shortcuts

`text_focused()` is true while a text field has focus. Shortcuts made of a plain printable key (`"n"`, `"shift+a"`) do not fire then; `mod+`, `ctrl+`, `alt+` shortcuts and keys like arrows, Enter and Escape still do. Opt a plain key back in with `.shortcut_typing("n", ..)` or `Hotkey::while_typing()`.

## Key names

One spelling everywhere: `"down"` and `"ArrowDown"`, `"esc"` and `"Escape"`, `"return"` and `"Enter"`, `"pgdn"` and `"PageDown"`, `"space"` and `" "` mean the same key in shortcuts, `Harness::key` and `Harness::chord`. `Key::from_name` and `canonical_key_name` expose the mapping.
