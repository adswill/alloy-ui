# alloy-ui-platform

Window/event shell and OS services. `unsafe` is denied crate-wide and allowed only in the native menu/tray code in `os/macos.rs` and `os/windows.rs`, each use with a SAFETY comment.

## Decisions
- **winit on all OSes** for windows, input, IME, DnD, scale factor, theme-change events. One code path covers X11 and Wayland; backends are chosen by winit at runtime.
- **Clipboard**: native on macOS (NSPasteboard) and Windows (Win32 CF_UNICODETEXT); Linux speaks the X11 selection protocol itself (`os/x11_clip.rs`, x11rb: owner thread, UTF8_STRING/STRING/TARGETS, multi-format HTML/PNG/URI list, INCR on read and write up to 64 MiB), and uses `wl-copy`/`wl-paste` on Wayland. **Dialogs** (`open_file`, `open_files`, `save_file`) **and dark-mode query**: native on macOS (NSOpenPanel/NSAlert/effectiveAppearance, with osascript/defaults fallback off the main thread) and Windows (IFileOpenDialog/IFileSaveDialog via the `windows` crate, MessageBoxW, registry); Linux uses stock tools (`pbcopy`/`osascript`/`defaults`, `powershell`/`clip`/`reg`, `xclip`/`xsel`/`zenity`/`kdialog`/`gsettings`) selected in `src/os/`. Reason: a correct native clipboard on X11 needs a selection-owner event loop, and COM/Cocoa dialogs need unsafe FFI we cannot test locally on Mac/Windows. Trade-off: slower calls and a dependency on helper tools on Linux; clipboard falls back to an in-process buffer. Can be replaced per-OS later behind the same traits.
- **Menus and tray**: data model in `menu`/`tray`; `ShellContext::set_menu/set_tray` install them. macOS: NSMenu + NSStatusItem via objc2. Windows: HMENU menu bar + `Shell_NotifyIconW` tray via windows-sys, using winit's message hook. Activations arrive as `PlatformEvent::MenuAction(id)` / `TrayClicked`. Linux: tray is a StatusNotifierItem over our own minimal D-Bus client (`os/dbus.rs`; icon, tooltip, click, context menu via com.canonical.dbusmenu). Linux menu bar: exported as a dbusmenu and registered with `com.canonical.AppMenu.Registrar` for the X11 window id (`os/linux_appmenu.rs`, sharing the tray's dbusmenu code), so KDE Plasma's Global Menu, Unity and MATE show it; with no registrar (GNOME) or a Wayland window `set_menu` returns an error and the shell still resolves menu shortcuts itself. Menu shortcuts are display-only on Windows; handle them as key events.
- **Multi-window**: `ShellContext::open_window(WindowKey, WindowSettings)` / `close_window`. Every callback runs with `ctx.window()` and `ctx.window_key()` set to the source window (global events such as `Wake` and menu/tray actions use `WindowKey::MAIN`). `PlatformEvent::WindowClosed` fires before a window is dropped so per-window surfaces can be released. The loop exits when the last window closes. The runtime/facade side (per-window views) is owned by core and the alloy facade.
- **Frame stats**: `ctx.frame_stats()` returns frame count, time to first paint (from entering `run`) and fps over the last full second. `ALLOY_PERF=1` also prints `alloy-perf first_paint_ms=..` and a totals line at exit to stderr, for benchmark scripts. These are CPU-side frame boundaries (the handler's `redraw` returned), not display scan-out times.
- **Wayland clipboard**: data-control (`ext_data_control_v1`, falling back to `zwlr_data_control_v1`) rather than core `wl_data_device`, because `wl_data_device` only delivers the selection to the focused client and needs an input serial to set it, neither of which a clipboard connection separate from winit's has. Works on wlroots compositors, KDE, COSMIC, niri. GNOME/Mutter offers neither protocol, so there the XWayland path (or the in-process buffer) applies. Owned selections die with the process unless a clipboard manager keeps them, as with X11.
- Idle: `ControlFlow::Wait`, redraw only on request; background threads wake the loop via `ShellProxy`.

## Mobile (prototype)
winit's NativeActivity (Android) and UIKit (iOS) backends supply the window. This crate adds `touch.rs` (gesture recognizer, pointer emulation, momentum), `safe_area.rs` (insets), `os/mobile.rs` (no dialogs, menu or tray), `os/android.rs` (the `AndroidApp` handle, content rect) and `os/ios.rs` (UIPasteboard, `safeAreaInsets`). `PlatformEvent` gains `Touch`, `Gesture`, `Suspended`, `Resumed` and `SafeAreaChanged`. Verified by unit tests, clippy on both targets and a built iOS static library and linked Android library; never run on a device. See `docs/guide/14-mobile.md`.

## Accessibility

`ctx.set_accessibility(&AccessTree)` publishes the frame's accessible nodes to the OS (see `src/access.rs`); `PlatformEvent::Accessibility` carries requests back. Node ids must be stable between frames (use the widget's focus id where it has one). The Linux adapter connects on a background thread, so a missing accessibility bus costs nothing and the app simply is not exposed.

| OS | Mechanism | Roles / patterns | Events sent |
|---|---|---|---|
| Linux | AT-SPI2 over D-Bus on the crate's own client (`os/linux_atspi.rs`) | Accessible, Component (extents, hit test, focus), Action, Value, Text, EditableText | ChildrenChanged, StateChanged (focused, checked, enabled), PropertyChange (name, value), TextChanged, Window Create/Activate |
| macOS | `NSAccessibilityElement` subclass children of a transparent `NSView` (`os/macos_access.rs`) | role, label, value, frame, enabled, focused; press, increment, decrement, set value, focus | LayoutChanged, TitleChanged, ValueChanged, FocusedUIElementChanged |
| Windows | HWND subclass answering `WM_GETOBJECT`, UIA fragment providers (`os/windows_access.rs`) | Invoke, Toggle, Value, RangeValue | structure changed, property changed (name, toggle, value, enabled), focus changed |

Known gaps: no text caret or selection (Text reports the caret at the end), no per-node parent hierarchy (all nodes are direct children of the window), no live regions, Wayland reports window-relative screen coordinates because clients cannot know their position.

## IME and key events

While an IME preedit string is non-empty the shell drops `KeyPressed` and `KeyReleased` (and the text of the key) instead of forwarding them; `ImePreedit` and the final `TextInput` commit still arrive. Reason: winit documents IME events and raw key events as independent streams, and on some backends (Wayland compositors without a text-input grab, Windows and X11 for keys the IME passes through) raw keys such as Backspace or Enter can still arrive during composition, so a text widget would edit twice. Not verified against a real IME on any OS: only Linux could be run here, and no input method was available.

## Notifications, global hotkeys and file-dialog options

- **Notifications** (`notify::send`): Linux calls `org.freedesktop.Notifications.Notify` on our own D-Bus client and falls back to `notify-send`; macOS posts through `osascript` (works unbundled and from any thread, attributed to Script Editor); Windows shows a toast through PowerShell using PowerShell's own app id. Chosen over `UNUserNotificationCenter` and `ToastNotificationManager` proper because those need a bundle identity or an installed shortcut, which an unpackaged binary does not have; bundling should switch to them.
- **Global hotkeys** (`hotkey::register`, event `PlatformEvent::GlobalHotkey`): Windows `RegisterHotKey` on a thread per hotkey with its own message loop; macOS Carbon `RegisterEventHotKey` (events come through winit's loop, no accessibility permission); X11 `XGrabKey` over x11rb, one connection and thread per hotkey, grabbing every Caps Lock and Num Lock combination, released by waking the thread with an empty `SendEvent`. Wayland returns an error: no compositor-neutral API (the GlobalShortcuts portal is a follow-up).
- **File dialogs** (`dialog::show(&FileDialogOptions)`): open, open many, folder and save with title, start folder, suggested name and extension filters. Linux builds `zenity` or `kdialog` arguments (pure functions, unit-tested), macOS uses `NSOpenPanel`/`NSSavePanel` on the main thread and a generated AppleScript elsewhere, Windows uses `IFileDialog` with `COMDLG_FILTERSPEC`, `SetFolder`, `SetTitle` and `FOS_PICKFOLDERS`.

## Verification
| Item | Linux | macOS | Windows |
|---|---|---|---|
| Unit tests (convert, theme parsers, fonts, menu, tray) | run locally | CI only | CI only |
| Real window, redraw, cross-thread wake (`examples/smoke`, X11 under Xvfb) | run locally | not run | not run |
| Linux X11 clipboard (own client): round trip, `xclip` cross-check both ways, 2 MiB INCR read | run locally under Xvfb (`tests/x11_clipboard.rs`; skipped without `DISPLAY`) | n/a | n/a |
| Linux dialogs (open, multi-open, save) | argument handling and parsing run against a fake `zenity` (`tests/linux_dialogs.rs`); no real zenity/kdialog window | n/a | n/a |
| Windows multi-select buffer parsing (`dialog.rs`) | run locally | CI only | CI only |
| Clipboard (macOS, Windows, Wayland), save/multi-open dialogs on macOS and Windows, theme backends | compiled | compiled in CI | compiled in CI |
| Linux tray (SNI over D-Bus) | verified against a real dbus-daemon (properties, Activate, dbusmenu GetLayout + Event via gdbus/busctl); not against a real desktop tray host | n/a | n/a |
| Linux accessibility (AT-SPI2) | **run**: `tests/atspi_e2e.sh` starts a private session bus with the real `at-spi-bus-launcher` and registry, and drives our server with libatspi (PyGObject): tree, roles, states, text, value, extents, hit test, actions back to the sink, focus and name-change events. Not tried with Orca. | n/a | n/a |
| macOS accessibility (NSAccessibility) | n/a | clippy-checked for `aarch64-apple-darwin` only; never run, VoiceOver untested | n/a |
| Windows accessibility (UI Automation) | n/a | n/a | clippy-checked for `x86_64-pc-windows-gnu` only; never run, Narrator untested |
| Native menu bar + tray | tray: see above. Menu bar: dbusmenu export verified against a real dbus-daemon (`os/linux_appmenu.rs` test, GetLayout and Event via busctl); registration with a mock `com.canonical.AppMenu.Registrar` and `StatusNotifierWatcher` while a real window ran under Xvfb (window id and `/MenuBar` received, menu Event and tray Activate changed app state). Not tried on a real Plasma | type-checked + clippy only, never run | type-checked + clippy only, never run |
| Notifications | `os/linux_notify.rs` test against a fake `org.freedesktop.Notifications` service on a real bus (summary, body, app name); the running example's Notify button reached a mock service. Not shown by a real desktop | AppleScript text unit-tested; compiled and clippy-checked only | toast script XML escaping unit-tested; compiled and clippy-checked only |
| Global hotkeys | **run**: `xdotool` typed the grabbed key under Xvfb and the event arrived with no window focused; a second grab was refused and the key was free again after release; a running example reacted to real key presses | compiled and clippy-checked only, never run | compiled and clippy-checked only, never run |
| File dialogs with options | argument builders unit-tested; `zenity` 3.x started with the generated open, save and folder arguments (no one chose a file); `kdialog` not available | AppleScript text unit-tested; AppKit path compiled only | filter patterns unit-tested; `IFileDialog` path compiled and clippy-checked only |
| Wayland window (winit on a Wayland compositor) | not run | n/a | n/a |
| Rich clipboard (HTML, PNG, files): X11 and headless sway (wl-clipboard) | run locally (`tests/rich_clipboard.rs`) | compiled (clippy `aarch64-apple-darwin`) | compiled (clippy `x86_64-pc-windows-gnu`) |
| OS settings (reduced motion, accent, dark) | run against a fake `gsettings` (`tests/settings_linux.rs`) | compiled | compiled |
| Window effects (transparency, KDE blur property) | run under Xvfb with xcompmgr | compiled | compiled |
| Deep links, single instance, file drop events | parsers and the lock run locally | compiled | compiled |

CI needs `libxkbcommon-x11-0` on Ubuntu to run the smoke example (winit loads it at runtime).
