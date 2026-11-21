# System integration

Five things a native app is expected to do, each one line in app code: drag and drop, deep links with a single running copy, a see-through window, rich clipboard, and following the OS accessibility and accent settings.

## Drag and drop inside the app

`.drag(payload)` makes any view a drag source and `.drop_act(handler)` makes a view a target. The payload is any `Clone + Send + Sync` value, and a target only receives payloads of its own type, so a list of `String` cards ignores a dragged `u32`.

```rust,no_run
use alloy_ui::prelude::*;

#[derive(Clone, Default)]
struct Board {
    todo: Vec<String>,
    done: Vec<String>,
}

fn main() {
    sketch(Board::default(), |b: &Board| {
        let card = |c: &String| text(c).pad(6.0).card().drag(c.clone());
        row![
            col(b.todo.iter().map(card).collect::<Vec<_>>())
                .pad(12.0)
                .w(220.0)
                .drop_act(|b: &mut Board, c: String| {
                    b.done.retain(|x| *x != c);
                    b.todo.push(c);
                }),
            col(b.done.iter().map(card).collect::<Vec<_>>())
                .pad(12.0)
                .w(220.0)
                .drop_act(|b: &mut Board, c: String| {
                    b.todo.retain(|x| *x != c);
                    b.done.push(c);
                }),
        ]
    })
    .run("Board");
}
```

A drag starts after the pointer moves 4 px. The original dims, a translucent copy follows the pointer on the overlay, and a target that accepts the payload highlights. Releasing outside any target drops nothing. `View::drop_on(|payload| Msg)` is the Elm-style twin for `App` implementations. Tests drive it with `Harness::drag(from, to)`.

### Files dropped on the window

Dropped files arrive as paths. While files hover, `files_hovering()` is true, so a view can show a hint.

```rust,no_run
use alloy_ui::prelude::*;
use std::path::PathBuf;

fn main() {
    sketch(Vec::<PathBuf>::new(), |files: &Vec<PathBuf>| {
        text(if files_hovering() {
            "Drop to add".to_string()
        } else {
            format!("{} files", files.len())
        })
    })
    .on_drop(|files, dropped| files.extend(dropped))
    .run("Drop files");
}
```

## Deep links and one running copy

`.url_scheme("myapp")` registers the scheme and `.on_open_url` receives every `myapp://...` link. `.on_open_files` receives documents opened through the OS. `.single_instance(id)` makes a second launch hand its arguments to the first and exit, so the link arrives in the window you already have.

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(String::new(), |last: &String| text(format!("Opened: {last}")))
        .single_instance("dev.example.links")
        .url_scheme("myapp")
        .on_open_url(|last, url| *last = url)
        .run("Links");
}
```

How each system delivers the link:

- **Linux and Windows** pass the URL as an argument. On first run the app writes a `.desktop` file plus `xdg-mime` (Linux) or a per-user registry key (Windows), so no installer is needed to try it.
- **macOS** routes links through Apple Events, and the scheme must be in `Info.plist`. `alloy bundle` writes it for you from `url-schemes = ["myapp"]` in `[package.metadata.alloy]`; `.desktop` bundles gain `x-scheme-handler` the same way.

Single instance uses a loopback TCP port and a random token stored in a 0600 lock file in the runtime directory. A lock left by a crash is detected and replaced.

## Window effects

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_| text("Hello").pad(24.0))
        .window_effect(WindowEffect::Mica)
        .run("Glass");
}
```

`WindowEffect` is `None`, `Mica`, `Acrylic`, `Blur` or `Transparent`. The page background gets the alpha that suits the effect, and the effect is requested before the window exists because X11 needs a 32 bit visual at creation time.

If the system cannot do an effect, Alloy falls back instead of drawing a broken window: where the compositor shows through but cannot blur, the page is slightly translucent; where nothing shows through, the page stays opaque. Windows 11 uses DWM backdrops, macOS an `NSVisualEffectView`, KDE on X11 asks the compositor to blur, and other compositors get plain transparency.

## Rich clipboard

```rust,no_run
use alloy_ui::{clipboard, prelude::*};

fn main() {
    sketch((), |_| {
        row![
            button("Copy table").act(|_: &mut ()| {
                clipboard::copy_html("<table><tr><td>1</td></tr></table>", "1");
            }),
            button("Copy image").act(|_: &mut ()| {
                clipboard::copy_image(1, 1, &[255, 0, 0, 255]);
            }),
            button("Paste").act(|_: &mut ()| {
                let c = clipboard::paste();
                println!("{:?} {} files", c.text, c.files.len());
            }),
        ]
    })
    .run("Clipboard");
}
```

`copy_html(html, plain)` puts both flavours on the clipboard so rich editors get markup and terminals get text. `copy_image(w, h, rgba)` and `copy_image_file(path)` copy pictures as PNG. `copy_files(paths)` copies file references for the file manager to paste. `paste()` returns whatever is there as a `ClipContent` with `text`, `html`, `image` and `files`; `paste_text()` is the plain version. Large values transfer in chunks (X11 INCR), so images of several megabytes work.

## OS accessibility and accent

Reduced motion, dark mode, high contrast and the accent colour are read at start-up and again when they change. Reduced motion is always followed: every animation jumps to its end state. The accent is opt in, because brand colour is usually a deliberate choice: add `.system_accent()` to use the OS colour for primary buttons, focus rings and selections. Read the current values with `system()`.

## Status

| Feature | Linux | macOS | Windows |
|---|---|---|---|
| In-app drag and drop | run | same code | same code |
| Files dropped on window | run (X11) | compiled | compiled |
| Deep links, single instance | run | compiled | compiled |
| Window effects | run (X11 with a compositor) | compiled | compiled |
| Rich clipboard (HTML, image, files) | run (X11 and Wayland) | compiled | compiled |
| Reduced motion and accent | run (gsettings) | compiled | compiled |

"Run" means exercised here with a real window or protocol; "compiled" means it type-checks and passes clippy for that target but has not run on that system. Known gaps: dragging files out of the window, files dropped on Wayland (winit 0.30 has no event for it), blur on KDE Wayland, and the pointer position during an X11 file drag.
