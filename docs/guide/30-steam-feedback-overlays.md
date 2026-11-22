# Overlays and toasts

Small additions from the Steam Oxide feedback. Snippets are marked `ignore`; the behaviour is
covered by `crates/alloy/tests/steam_feedback_overlays.rs`.

## Toasts: inset and count

```rust,ignore
sketch(state, view)
    .toast_inset(40.0) // keep the stack 40 px above the window bottom (default 24)
    .toast_max(3)      // show at most 3 at once, oldest dropped first (default 4)
```

Hand-written `App`s override `fn toast_layout(&self) -> (f32, usize)`.

## Modal

```rust,ignore
modal(s.open, col![heading("Library"), scroll(list).fill_h(), button("Done").act(done)])
    .max_height(520.0)            // card never taller than this; a Fill scroll inside shrinks
    .keep_content_on_exit()       // the exit fade shows the last content, not an empty card
    .default_action(|s: &mut S| s.install()) // Enter; text fields and open menus keep Enter
    .close(|s: &mut S| s.open = false)
```

The enter ease now starts from closed on every open, including a modal that is only built
while open. Under `Harness`, `settle()` does not move the clock: use `advance(ms)` to watch it.

## Portal

`portal(open, content).close(|s| ..)` (or `.on_close(msg)`) runs on Escape once nothing inside
the portal (a menu, a popover) used it. Without it, Escape now reaches the content and is not
swallowed. `portal` now returns a `Portal` builder (converts into a `Node` like `Modal`).

## Confirm

`button("Clear").act(..).confirm("Sure?")` now cancels on Escape. `.on_escape(act)` (set before
`.confirm`) runs as well when that happens.
