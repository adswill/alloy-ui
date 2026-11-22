# Runtime and Harness notes

## Writing an `App` by hand

`sketch(..)` and `app(..).page(..)` wire up a few things for you. A hand-written `impl App`
gets the ones marked "runtime" automatically; the rest you do yourself.

| what | `Sketch` / `Pages` | hand-written `impl App` |
|---|---|---|
| `resource` / `resource_for` delivery and `.every(..)` refresh | runtime | runtime (the window host pumps them for every app) |
| `Cmd::perform`, `perform_keyed`, `cancel` | runtime | runtime |
| `.tick(period, ..)` timers | `Sketch::tick` | return `every(period, || Msg::Tick)` from `subscriptions()` |
| close request, tray, menus, URL schemes | builder methods | `on_close`, `desktop()`, `on_open_url` |
| live `.alloy` reload polling | runtime | not wired; use `Sketch` for live views |

```ignore
struct Shop;
impl App for Shop {
    type Msg = ();
    fn view(&self) -> impl View<()> {
        // Leaves "Loading" in the real window too; no pump to wire up.
        let r = resource_for(1u32, |k| async move { Ok::<_, String>(k) });
        text(if r.get().is_some() { "ready" } else { "Loading" })
    }
}
```

## Toasts in tests

`Harness::new` clears the thread's toast list and clock, so a toast left over from a dropped
`Harness` can no longer panic the next one. Toast expiry also uses saturating time.

## Harness

* `Harness::pump_like_runtime(true)` makes `settle()` apply resources only on the runtime's
  timer (50 ms of scripted time while a fetch runs, 500 ms for refresh timers) instead of
  pumping directly. Use it to mimic the window. The window host pumps for every `App`, so a
  hand-written app and a `Sketch` behave the same.
* `settle_ignoring(&["delivery"])` does not wait for `Cmd::perform_keyed("delivery", ..)`
  futures, so an app whose timer starts a never-ending command still settles at once.
* Scroll sign: `scroll(over, dy)` / `scroll_here(dy)` take the wheel's `dy`. Positive shows
  earlier content (content moves down); negative shows the content below. `scroll` hovers the
  text on every call; once it left the clip use `scroll_here`.
* `Harness::phases()` lists timed phases of building the Harness (`harness-ui-setup`,
  `harness-first-view`); `perf_mark("name", |h| ..)` adds your own. With `ALLOY_PERF=1` they
  are also printed like the real window's phase log.
