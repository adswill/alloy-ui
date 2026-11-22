# Steam-style looks

## Square avatars with a status border

`avatar(name).square()` draws a slightly rounded square. With a presence set it gets a 2px
border in the status colour instead of a dot. `Presence::InGame` is Steam green.

```rust
use alloy_ui::prelude::*;

fn friend() -> impl View<()> {
    row![
        avatar("Mira Okafor").square().size(40.0).presence(Presence::InGame),
        avatar("Jon Hale").square().size(40.0).presence(Presence::Online),
    ]
    .gap(8.0)
}
```

## Counter as one string

A resting `counter(..)` is drawn as one string including prefix and suffix, so
`counter(3).suffix(" of 8")` is found as `"3 of 8"` in tests (and read that way by
screen readers). While the digits roll, the per-digit wheels are drawn as before.
