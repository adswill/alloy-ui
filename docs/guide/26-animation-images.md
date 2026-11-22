# Animation and images

## Fade pictures

`image(id, w, h)` draws a bitmap registered with the renderer. `.opacity(0.4)` draws it
translucent. `fade`, `reveal` and `enter` around an image fade the picture like any other
content (the draw list carries `DrawCommand::ImageAlpha`).

```rust,ignore
fade(s.show_art, image(art_id, 120.0, 120.0))
```

## Keyed scope for list rows

Animation, hover and `local` state is keyed by call site and order. In a list that reorders or
recycles rows, wrap each row in `keyed(id, || ..)` so the state follows the item:

```rust,ignore
col(s.items.iter().map(|it| keyed(it.id, || reveal(it.open, row(it)))))
```

## Animate on first build

`reveal` and `fade` show already-visible content at once. `reveal_appear(show, child)` and
`fade_appear(show, child)` also ease in the first time they are built.

## Exit animations in lists

`presence_list(items, |t| t.id, |t| row(t))` keeps removed rows mounted until their exit has
played, eases in rows added later, and shows the first build at once. Under reduced motion
rows vanish immediately. Items are `Clone + Send`.

## Stagger in `vlist`

`stagger` and `enter` work inside `vlist` rows: a row plays its entrance when it scrolls into
view and settles fully opaque.
