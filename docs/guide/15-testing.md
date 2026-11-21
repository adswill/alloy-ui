# Testing your app

`Harness` runs your app through the real runtime with no window and no GPU. You find things by the text on screen, the way a person does, so a test survives a layout change and fails with the screen's contents when it breaks.

```rust
use alloy_ui::{prelude::*, testing::Harness};

#[derive(Default)]
struct Form { name: String }

let app = sketch(Form::default(), |s: &Form| {
    col![
        input(at!(s.name)).hint("Name"),
        text(format!("Hello {}", s.name)),
        button("Clear").act(|s: &mut Form| s.name.clear()),
    ]
});
let mut ui = Harness::new(app, 360.0, 200.0);
ui.fill("Name", "Ada").expect("Hello Ada");
ui.click("Clear").expect_no("Hello Ada");
```

Every step returns `&mut Harness`, so a flow reads as a chain.

| Do | With |
|---|---|
| click, right-click, hover | `click(text)`, `right_click(text)`, `hover(text)` |
| type | `fill(field, value)` (replaces the contents), `type_text(s)` |
| keys | `key("Enter")`, `key("ArrowDown")`, `chord("mod+s")`, `chord("alt+ArrowLeft")` |
| scroll, resize | `scroll(over_text, dy)`, `resize(w, h)` |
| time | `advance(ms)` runs animations and toasts |
| async data | `settle()` waits for [resources](06-async-resources.md); `wait_for(text)` waits for the text |
| assert | `expect(text)`, `expect_containing(part)`, `expect_no(text)`, `count(text)`, `texts()` |
| pictures | `snapshot("name")` compares with `tests/snapshots/name.png`; `save_png(path)` |

When an `expect` fails, the message lists every piece of text that *was* on screen, which is usually enough to see what went wrong without a debugger.

## Snapshots

`snapshot("checkout")` renders the frame at 2x and compares it with `tests/snapshots/checkout.png` in your crate, allowing a tiny anti-aliasing difference. Create or update the pictures with `UPDATE_GOLDEN=1 cargo test`. On a mismatch the new picture is written next to the temp directory and its path is in the failure message, so you can open both. Fonts differ between operating systems, so generate and check snapshots on the system your CI uses; use text assertions for anything that must hold everywhere.

## Testing against fake data

Pair `Harness` with `Record::mock(..)` (see the `status` example) for a backend that fails on schedule, then `settle()` and `expect("Could not load")` and `click("Retry")`, or use `resource_for` with a key you change from a test to check that a stale answer never shows.
