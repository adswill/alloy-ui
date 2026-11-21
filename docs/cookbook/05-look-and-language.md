# Look and language

## 21. A light and dark switch

A window follows the system appearance. To let the person choose, call `set_dark` from a handler.

```rust
use alloy_ui::prelude::*;

fn view(dark: &bool) -> impl View<Act<bool>> {
    col![
        heading("Appearance"),
        choose([("Light", false), ("Dark", true)], *dark).act(|d: &mut bool, v| {
            *d = v;
            set_dark(v);
        }),
    ]
    .gap(10.0)
    .pad(20.0)
}
# fn main() { let _ = sketch(false, view); }
```

## 22. Your brand accent

`Look` describes the whole window in one value and can be re-applied at any time. The accent is nudged until its text passes WCAG AA.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_| {
        set_look(Look::light().accent(Color::hex(0x4338CA)).density(Density::Compact));
        col![heading("Brand colour"), button("Continue").primary()].gap(12.0).pad(24.0)
    })
    .run("Brand")
}
```

## 23. Custom colours that follow the theme

Read `palette()` instead of hard-coding colours, and a custom surface works in light and dark.

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    let p = palette();
    col![text("Pinned").color(p.text), caption("Stays on top".to_string()).dim()]
        .gap(4.0)
        .pad(16.0)
        .bg(p.surface)
        .round(6.0)
}
# fn main() { let _ = sketch((), view); }
```

## 24. Progress, stats and a chart

`progress` takes a range and a value. `stats` is a row of cards. `bars` draws values between 0 and 1.

```rust
use alloy_ui::prelude::*;

fn view(done: &f32) -> impl View<Act<f32>> {
    col![
        stats([("Open", "12"), ("Closed", "148"), ("Blocked", "2")]),
        row![text("Migration"), progress(0.0..=1.0, *done)].gap(12.0).align_center(),
        bars([0.2, 0.5, 0.4, 0.8, 0.6, 0.9, 0.7]),
    ]
    .gap(16.0)
    .pad(20.0)
}
# fn main() { let _ = sketch(0.65f32, view); }
```

## 25. Translations and plurals

A catalog is `key = value` lines. Plural entries add one line per category, and the count is passed as `n`. The test switches language mid-run.

```rust
use alloy_ui::{i18n, t};

fn main() {
    i18n::add("en", "inbox.one = {n} unread message\ninbox.other = {n} unread messages");
    i18n::add("fr", "inbox.one = {n} message non lu\ninbox.other = {n} messages non lus");
    assert_eq!(t!("inbox", n = 1), "1 unread message");
    i18n::set_locale("fr");
    assert_eq!(t!("inbox", n = 3), "3 messages non lus");
    i18n::set_locale("en");
}
```
