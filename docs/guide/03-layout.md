# Layout

Layout is flexbox-style and written by hand for Alloy (no external layout engine). Two containers do almost everything: `col!` stacks children top to bottom, `row!` left to right.

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    col![
        heading("Invoice"),
        row![text("Total"), space(), text("$1,204.00")].align_center(),
        rule(),
        row![button("Cancel").ghost(), button("Pay").primary()].gap(8.0),
    ]
    .gap(14.0)
    .pad(24.0)
}
# fn main() { let _ = sketch((), view); }
```

## Spacing

* `.gap(px)` space between children. `.pad(px)` space inside the container; `.pad_xy(x, y)` for different values.
* `space()` is a flexible spacer that takes whatever room is left (`row![a, space(), b]` pushes `b` to the right). `vspace()` is the vertical twin, `gap(w, h)` a fixed gap, `rule()` a thin divider.

## Size

Every item is *shrink-wrapped* by default, takes a *fixed* size with `.w(px)` / `.h(px)`, or *fills* the remaining room with `.fill_w()`, `.fill_h()` or `.fill()`. Fill children of one container share the leftover space equally.

```rust
use alloy_ui::prelude::*;

fn view(s: &String) -> impl View<Act<String>> {
    row![
        col![text("Sidebar")].pad(12.0).w(170.0).fill_h().surface(), // fixed width, full height
        col![input(s).fill_w(), "content"].fill(),                  // takes the rest
    ]
}
# fn main() { let _ = sketch(String::new(), view); }
```

## Alignment

`.align_center()` and `.align_end()` set the cross-axis alignment (vertical in a `row!`, horizontal in a `col!`). `.center()` centers the container's own content.

## Surfaces

`.surface()` fills with the sidebar colour, `.card()` draws a bordered card, `.outline()` a hairline border, `.round(r)` sets the corner radius and `.bg(color)` paints any colour. Colours inside the theme should come from `palette()` so they follow light and dark.

## Scrolling

`scroll(content)` makes any view scrollable. Wrap a column of rows in it; give the scroll area a bounded height (a `.fill()` parent, or `.h(px)`).

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    col![heading("Log"), scroll(col((0..200).map(|i| text(format!("line {i}")))))]
        .gap(8.0)
        .pad(16.0)
        .fill()
}
# fn main() { let _ = sketch((), view); }
```

## Resizing

The window's layout is recomputed on every size change. There are no breakpoints to write: use `fill` for the parts that should grow and fixed sizes for the rest.
