# The dev loop: `alloy dev` and the inspector

## Rebuild and restart on save

```sh
alloy dev
```

builds the app, runs it, and watches `src/`, `assets/`, `Cargo.toml` and the capability file. Every save rebuilds and restarts the window. A compile error prints cargo's own message and the old window closes only when a new build succeeds, so you fix the error and save again. Closing the window ends the command. `alloy dev --once` runs once without watching, and `alloy dev -- --flag` passes `--flag` to your app.

Alloy does not patch code into a running process. It restarts, which is fast for a debug build and always correct. To make a restart feel like a reload, persist the state:

```rust,ignore
sketch(State::default(), view).persist("my-app")
```

The counter, the open page and every `store` come back as they were, so only the window flickers. Keep transient things (a search box) in `local` so they reset.

## The inspector (F12)

Press **F12** in any debug build, or in a release build started with `ALLOY_INSPECT=1` (`alloy dev` sets it). Every widget gets an outline coloured by depth. The widget under the cursor is highlighted and labelled with its type, size and position, and the bottom bar shows its path from the root, for example `Root > Column > Row > Button`.

Widgets need no code to appear: the inspector reads the same tree and layout the renderer uses. Custom widgets get their type name for free.

For a screenshot in docs or a bug report, use the headless flags:

```sh
cargo run --example notes -- --headless inspect.png --hover Notes --inspect
```

In tests, `Harness::hover(text)` and `Harness::inspect(true)` do the same.
