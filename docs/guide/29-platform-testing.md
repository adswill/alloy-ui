# Platform and testing

## Timers a page starts itself

`.tick(..)` and `subscriptions()` are fixed at app set-up. A page can also ask for a timer while
it is on screen: call `tick_while_shown` (or `subscribe(every(..))` with the `App` trait) from the
view. The timer starts the first time a build asks for it and stops the first rebuild that does
not, so a page that is not shown costs nothing.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Default)]
struct Feed { live: bool, n: u32 }

fn main() {
    sketch(Feed::default(), |s: &Feed| {
        if s.live {
            tick_while_shown(secs(5.0), |s: &mut Feed| s.n += 1);
        }
        col![
            text(format!("refreshed {} times", s.n)),
            button("Go live").act(|s: &mut Feed| s.live = true),
        ]
    })
    .run("Feed")
}
```

Each call site is one timer; several calls from one loop iteration share an id, so use one call
per timer.

## Harness additions

* `ui.advance(ms)` now also fires due timers (`.tick`, `every`, `tick_while_shown`).
* `right_click_at(x, y)`, `middle_click(text)`, `middle_click_at(x, y)` join `right_click(text)`.
* `settle_motion()` advances the clock in frames until nothing animates (route transitions,
  fades, toasts). It gives up after about ten seconds of scripted time, so an endless spinner
  does not hang a test.
* `reduce_motion(true)` turns reduced motion on for the window, so animations jump to the end.
* `window_effect(WindowEffect::Blur)` pretends the OS applied a material (`None` undoes it).

```rust
use alloy_ui::{prelude::*, testing::Harness};

let app = sketch(0, |p: &i32| col![
    button("Next").act(|p: &mut i32| *p = 1),
    route(*p as usize, text(format!("page {p}"))),
]);
let mut ui = Harness::new(app, 300.0, 200.0);
ui.click("Next").settle_motion();
ui.expect("page 1").expect_no("page 0");
```

## Window material behind chrome

`.window_effect(WindowEffect::Mica | Acrylic | Blur | Transparent)` makes the page background
see-through, but opaque side bars and headers hid it. `Boxed::surface()` (the panel colour) now
takes the same translucency as the page while an effect is active, so the material shows behind a
sidebar or header built with `.surface()`. `.card()` and `.bg(colour)` stay solid, which is what
you want for content that must stay readable. Without an effect nothing changes.

Limit: only regions built with `Boxed::surface()` follow the effect; widgets that paint their own
palette surface (tables, tab bars) stay opaque.
