# The dev loop: `alloy dev` and the inspector

## Rebuild and restart on save

```sh
alloy dev
```

builds the app, runs it, and watches `src/`, `assets/`, `Cargo.toml` and the capability file. Every save rebuilds and restarts the window. A compile error prints cargo's own message and the old window closes only when a new build succeeds, so you fix the error and save again. Closing the window ends the command. `alloy dev --once` runs once without watching, and `alloy dev -- --flag` passes `--flag` to your app.

Rust code is not patched into a running process (see "Why not hot-patch Rust?" below). Rust changes restart the app, which is fast for a debug build and always correct. The parts of a screen that change most often, its layout, copy, spacing and theme, can live in a [live view](#live-views-reload-on-save-with-no-restart) that reloads the instant you save, without a restart. To make a Rust restart feel like a reload, persist the state:

```rust,ignore
sketch(State::default(), view).persist("my-app")
```

The counter, the open page and every `store` come back as they were, so only the window flickers. Keep transient things (a search box) in `local` so they reset.

## Live views: reload on save with no restart

Write the screen in a small indented file and let Rust wire the behaviour:

```text
# ui/home.alloy
theme dark accent=#6D4AFF
col page gap=14
  heading "Hello, {name}"
  text "{count} so far" dim
  row gap=8 center
    button "Add" primary @add
    button "Reset" ghost @reset
```

```rust
use alloy_ui::prelude::*;
# #[cfg(any())]
fn main() {
    sketch(0i64, |n| {
        live!("ui/home.alloy")
            .var("name", "Ada")
            .var("count", n)
            .on("add", |n: &mut i64| *n += 1)
            .on("reset", |n| *n = 0)
    })
    .run("Home")
}
# fn main() {}
```

Under `alloy dev` the file is re-read on every save (a 250 ms check that only exists in dev) and the window changes in place: the counter, focus, scroll positions and open page are untouched, because nothing restarted. Try it: `cargo run -p alloy-ui --example live`, then edit `crates/alloy/ui/counter.alloy`. In a release build the file is compiled into the binary and never read from disk, so there is no file to ship and no cost.

A mistake does not close the window. The last good screen stays and a red line names the problem, for example `home.alloy: line 4: unknown element `bttn``. Elements:

| Element | Settings and flags |
|---|---|
| `col`, `row` | `gap=`, `pad=`, `w=`, `h=`, `round=`; flags `page`, `fill`, `center`, `card`, `surface`, `outline`, `elevated` |
| `scroll` | a scrolling column (`gap=`, `pad=`) that keeps its offset across reloads |
| `text`, `heading`, `caption` | `dim`, `danger`, `success`, `accent` |
| `button` | `primary`, `ghost`, `fill`; `@name` wires it to `.on("name", ..)` |
| `badge "Live"` | `success`, `warning`, `danger`, `accent`, `auto`, `dot`, `outline` |
| `avatar "Ada Lovelace"` | initials on a colour taken from the name |
| `progress` | `value=`, `min=`, `max=`; any of them may be `{var}` |
| `if name` / `unless name` | show the children when the yes/no value `name` is true / false |
| `rule`, `space`, `vspace`, `gap` | dividers and spacing (`gap` takes `w=`, `h=`) |

Text goes in `"quotes"` and may contain `{var}`. A `button` with no matching `.on` is disabled so a typo is visible. Pass yes/no values with `.when("signed_in", s.user.is_some())`. A first `theme` line sets `dark`, `accent=#rrggbb`, `density=compact|spacious` and `contrast=high`; the theme editor example prints that line for you under "Copy .alloy line".

```text
col page gap=14
  row gap=10 center
    avatar "{name}"
    heading "{name}"
    badge "Pro" accent
    space
  if busy
    progress value={done} max=100
  col card gap=6
    caption "Storage" dim
    progress value=62
  scroll gap=4
    text "Changelog"
```

`alloy dev` also reopens the window where it was (position and size), so a restart after a Rust change looks like a reload. Combine it with `.persist("my-app")` and the state comes back too.

## Why not hot-patch Rust?

Reloading compiled view code in-process means loading the app as a dynamic library and swapping it while the window runs. We looked at it and chose not to ship it:

* Each library would carry its own copy of Alloy, with its own thread-locals, so the ambient window state a view reads would be missing unless Alloy itself became a shared library on all three systems.
* The state type crosses the boundary. Add a field and the old and new layouts disagree, which is undefined behaviour rather than an error message.
* Rust has no stable ABI, so it only works while every compiler flag and dependency version matches exactly, and a failure is a crash, not a message.

A restart with `.persist()` is correct in every one of those cases, and a live view covers the edits you make hundreds of times a day. If you need the rest, build the heavy logic as a library and keep the view in a live file.

## The inspector (F12)

Press **F12** in any debug build, or in a release build started with `ALLOY_INSPECT=1` (`alloy dev` sets it). Every widget gets an outline coloured by depth. The widget under the cursor is highlighted and labelled with its type, size and position, and the bottom bar shows its path from the root, for example `Root > Column > Row > Button`.

Widgets need no code to appear: the inspector reads the same tree and layout the renderer uses. Custom widgets get their type name for free.

For a screenshot in docs or a bug report, use the headless flags:

```sh
cargo run --example notes -- --headless inspect.png --hover Notes --inspect
```

In tests, `Harness::hover(text)` and `Harness::inspect(true)` do the same.
