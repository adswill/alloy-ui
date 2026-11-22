# 22. Web GUI: drive an app from a browser

Alloy draws with its own renderer, so there is no webview. The web GUI keeps it that way: your app
runs natively and a browser tab is only a **remote display**. The page holds no app logic.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    let app = sketch(0i64, |n| button(format!("{n}")).act(|n| *n += 1));
    alloy_ui::web::serve(app, "127.0.0.1:8080", (900.0, 600.0)).unwrap();
}
```

Open `http://127.0.0.1:8080`.

You do not have to write that call. Any app that ends in `.run("Title")` also serves itself when
started with `--web [addr]` or with `ALLOY_WEB=127.0.0.1:8080` set, so `cargo run --release -p
alloy-ui --example notes -- --web` shows the notes example in a browser with no code change. Try the counter: `cargo run -p alloy-ui-examples --bin web`.

## How it works

- `serve` runs the app through the same headless runtime that `Harness` uses in tests.
- `GET /frame?v=N` returns the newest frame as a PNG, or `204` when nothing changed since version `N`.
- `POST /event` takes JSON: `move{x,y}`, `down`, `up`, `scroll{dy}`, `text{s}`, `key{k,mods}`, `resize{w,h}`.
- The server uses only `std` plus the `serde_json` the crate already depends on.

## What it is for

Trying an app without installing a toolchain on the viewing machine, screen-sharing a demo, and
looking at an app running on a remote or headless box.

## Limits (honest list)

- Frames are drawn by the CPU rasteriser, so there is no GPU shader content (`gpu_view`, 3D).
- No audio or video, no native menus, tray, file dialogs or clipboard bridge.
- One shared app instance: every viewer sees and drives the same window.
- No authentication. Bind to loopback unless you mean to expose the app, because the page can click and type.
- Checked on macOS only. Linux and Windows are covered by CI, not by hand.
