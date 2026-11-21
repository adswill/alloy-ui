# Lists and layout

## A virtualised grid

`vgrid(count, columns, cell)` is `vlist` in two dimensions: only the rows in view are built, so
10 000 tiles scroll like 20. Size cells with `.cell_height(px)` or `.aspect(ratio)`, space them
with `.gap(px)`, or use `.fit(min_width)` for as many columns as fit. Click, the arrow keys
(left, right, up, down), Home, End and PageUp/PageDown select; `.act(|s, i| ..)` hears about it.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default)]
struct Shop { open: Option<usize> }

fn main() {
    sketch(Shop::default(), |s: &Shop| {
        col![
            text(format!("Open: {:?}", s.open)),
            vgrid(10_000, 4, |i| text(format!("Game {i}")).pad(8.0))
                .aspect(1.5)
                .act(|s: &mut Shop, i| s.open = Some(i)),
        ]
    })
    .run("Shop")
}
```

## A chat log that stays at the bottom

`scroll_bottom(view)` is `scroll` that starts at the newest content and follows it while the
view is at the bottom. Scroll up and it stays put; scroll back to the end and it follows again.
No scroll ids to compute.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(vec!["hello".to_string()], |log: &Vec<String>| {
        col![
            scroll_bottom(col(log.iter().map(|m| text(m.clone())))),
            button("Send").act(|log: &mut Vec<String>| log.push("more".into())),
        ]
    })
    .run("Chat")
}
```

## Reading a split's size

Make a handle, give it to the split, and read `width()` anywhere in the view. It is the first
pane's size as of the last frame (its height for `.vertical()`), and the view is rebuilt when
a drag or a resize changes it.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_: &()| {
        let side = split_handle();
        let narrow = side.width() < 160.0;
        split(text(if narrow { "Menu" } else { "Main menu and shortcuts" }), text("Content"))
            .handle(side)
            .ratio(0.25)
    })
    .run("Sidebar")
}
```

## A carousel

`carousel(slides)` shows one slide at a time with a crossfade, arrows and dots (Left and Right
keys work when focused). `.auto(secs(5.0))` advances on a timer and pauses while the pointer
is over it. `.act(|s, i| ..)` reports changes, `.index(i)` pins the position, `.height(px)`
fixes the height, and `.no_arrows()` / `.no_dots()` hide the controls. Under reduced motion it
switches without the fade.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_: &()| {
        carousel([heading("Portal"), heading("Half-Life"), heading("Dota 2")])
            .auto(secs(5.0))
            .height(160.0)
    })
    .run("Featured")
}
```

## Text with a weight and a colour

`text(..)` takes both: `.weight(600)`, `.bold()`, `.semibold()` combine with `.color(..)`,
`.accent()`, `.dim()` and the roles.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_: &()| text("Online").semibold().success()).run("Status")
}
```
