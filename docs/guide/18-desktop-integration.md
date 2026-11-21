# Desktop integration

A menu bar, a tray icon, notifications, file dialogs and a hotkey that works while another app is in front: each is one call, and each uses the operating system's own machinery (no web view, no helper app).

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(0i64, |n| col![text(n.to_string()), button("Add").act(|n| *n += 1)])
        .menu("File", |m| {
            m.item("Reset", "mod+r", |n| *n = 0)
                .separator()
                .item("Quit", "mod+q", |_| quit())
        })
        .tray("Counter", |t| t.click(|n| *n += 1).item("Quit", |_| quit()))
        .global_hotkey("ctrl+alt+c", |n| *n += 1)
        .run("Counter")
}
```

`mod` is Cmd on macOS and Ctrl elsewhere. `app(..)` apps have the same `.menu`, `.tray` and `.global_hotkey`, with plain `|| ..` closures. Every menu item is also a command in the palette (`mod+k`) and its shortcut works inside the window on every OS, so an app is fully usable on a desktop that has no menu bar to show.

## Menus

`.menu(title, |m| ..)` adds a top-level menu. Items are `item(label, shortcut, action)`, with `""` for no shortcut, plus `separator()` and nested `sub(title, |m| ..)`. Shortcuts of `shortcut_as` that no menu lists still go into an automatic Actions menu (and Undo and Redo into Edit), so nothing is hidden.

| OS | What you get |
|---|---|
| macOS | The real menu bar. Alloy adds the application menu (named after the window title, with Quit) in front of yours. |
| Windows | A native menu bar on the window. |
| Linux | Exported over D-Bus to the desktop's global menu (KDE Plasma's Global Menu widget, Unity, MATE). GNOME and bare window managers have no menu bar to put it in; the app logs that at debug level and the shortcuts and palette still work. Wayland windows cannot join a global menu yet. |

`quit()` (from the prelude) closes every window and exits. Call it from any handler.

## Tray icon

`.tray(tooltip, |t| ..)` shows an icon with a context menu: `t.item(label, action)`, `t.separator()`, `t.click(action)` for a click on the icon itself, and `t.icon(rgba, width, height)` for your own 32x32 icon. The default is a rounded square with an A.

On macOS it is a status item in the menu bar, on Windows a notification-area icon, on Linux a StatusNotifierItem (KDE, and GNOME with the AppIndicator extension).

## Global hotkeys

`.global_hotkey("ctrl+alt+space", |s| ..)` fires wherever the focus is.

| OS | How |
|---|---|
| Windows | `RegisterHotKey` |
| macOS | Carbon `RegisterEventHotKey` (no permission prompt) |
| X11 | `XGrabKey`, ignoring Caps Lock and Num Lock |
| Wayland | The `org.freedesktop.portal.GlobalShortcuts` portal (GNOME 48+, KDE Plasma 5.27+). The desktop may ask the user to approve or rebind the shortcut, and their choice wins. Sway and other wlroots desktops have no such portal: the failure is logged and the in-window shortcut still works. |

A combination another program already owns is refused and logged; pick another.

## Notifications

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch((), |_| button("Done").act(|_| notify("Export finished", "report.csv is ready")))
        .run("Notify")
}
```

`notify(title, body)` never blocks and never fails loudly. It uses `org.freedesktop.Notifications` on Linux (with `notify-send` as a fallback), Notification Center on macOS and a toast on Windows. On macOS the banner is attributed to Script Editor, and on Windows to PowerShell, until the app is bundled with its own identity.

## File dialogs

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    sketch(String::new(), |path| {
        col![
            text(path.clone()),
            button("Open").act(|path: &mut String| {
                if let Some(p) = files::open().title("Pick a picture").filter("Images", &["png", "jpg"]).pick() {
                    *path = p.display().to_string();
                }
            }),
            button("Save as").act(|path: &mut String| {
                if let Some(p) = files::save("notes.txt").pick() {
                    *path = p.display().to_string();
                }
            }),
        ]
    })
    .run("Dialogs")
}
```

`files::open()`, `files::open_many()` (read with `.pick_all()`), `files::save(name)` and `files::folder()` start a dialog; `.title`, `.dir` and `.filter(name, &[extensions])` refine it; `.pick()` shows it and waits. `None` means the user cancelled, or there is no dialog on this system (the reason is logged).

The dialogs are the OS's own: `NSOpenPanel` on macOS, the common item dialog on Windows, and `zenity` or `kdialog` on Linux (install one; GNOME and KDE desktops already have them).

## Testing

`Harness` can play the OS's part:

```rust
use alloy_ui::{prelude::*, testing::Harness};

let mut h = Harness::new(
    sketch(0i64, |n| text(n.to_string()))
        .menu("File", |m| m.item("Add", "mod+a", |n| *n += 1))
        .global_hotkey("ctrl+alt+a", |n| *n += 10),
    300.0,
    200.0,
);
h.desktop("mod+a");                 // the user chose File > Add
h.desktop("global:ctrl+alt+a");     // the hotkey fired elsewhere
assert!(h.has_text("11"));
assert_eq!(h.desktop_spec().menus[0].title, "File");
```

Ids are the item's shortcut when it has one, otherwise `cmd:<label>`; tray items are `tray:<label>`, the icon click is `tray:click`, and hotkeys are `global:<keys>`.
