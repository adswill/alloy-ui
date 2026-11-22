# Window and menus

## Close to the menu bar

`cx.cancel_close()` keeps the window open on screen. To hide it instead, call `cx.hide()` (or
`cx.hide_window()`), and bring it back from any handler with `show_window()`. `hide_window()` and
`window_hidden()` work from tray, menu and global-hotkey handlers too.

```rust,ignore
sketch(state, view)
    .tray("App", |t| t.click(|_| show_window()).item("Quit", |_| quit()))
    .on_close_requested(|cx| cx.hide())
    .run("App");
```

## Minimum window size

`Sketch::min_window_size(w, h)` (or `App::min_window_size`) stops the user shrinking the window
below that size. `Harness::resize` clamps the same way and `Harness::size()` reads the result.

## Application menu and standard items

```rust,ignore
sketch(state, view)
    .app_menu(|m| m.about()
        .item("Settings...", "mod+,", |s| s.open_settings())
        .separator().hide().hide_others().show_all().separator().quit())
    .menu("Edit", |m| m.cut().copy().paste().select_all())
    .menu("File", |m| m.item_enabled("Save", "mod+s", s_dirty, |s| s.save()));
```

* `app_menu` is the first menu of a Mac app. Other systems ignore it (custom items keep their shortcuts).
* `cut/copy/paste/select_all` replay `mod+x/c/v/a` to the focused widget, so a text field handles
  its own selection and no app shortcut is stolen. Hide, Hide Others, Show All and About are
  answered by AppKit.
* `item_enabled(label, keys, enabled, f)` greys the item and drops its shortcut and palette entry.
  `enabled` is fixed when the menu is built; a hand-written `App` returns a fresh `Desktop` from
  `desktop()` after each change to follow its state.
* Shortcuts keep every modifier: `"mod+ctrl+f"` is Ctrl+Cmd+F on macOS.
