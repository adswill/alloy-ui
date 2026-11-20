# alloy-ui-platform

Window/event shell and OS services. `unsafe` is denied crate-wide and allowed only in the native menu/tray code in `os/macos.rs` and `os/windows.rs`, each use with a SAFETY comment.

## Decisions
- **winit on all OSes** for windows, input, IME, DnD, scale factor, theme-change events. One code path covers X11 and Wayland; backends are chosen by winit at runtime.
- **Clipboard**: native on macOS (NSPasteboard) and Windows (Win32 CF_UNICODETEXT); Linux speaks the X11 selection protocol itself (`os/x11_clip.rs`, x11rb: owner thread, UTF8_STRING/STRING/TARGETS, INCR on read; values over 128 KiB written fall back to `xclip`/`xsel`), and uses `wl-copy`/`wl-paste` on Wayland. **Dialogs** (`open_file`, `open_files`, `save_file`) **and dark-mode query**: native on macOS (NSOpenPanel/NSAlert/effectiveAppearance, with osascript/defaults fallback off the main thread) and Windows (GetOpenFileNameW/MessageBoxW/registry); Linux uses stock tools (`pbcopy`/`osascript`/`defaults`, `powershell`/`clip`/`reg`, `wl-copy`/`xclip`/`xsel`/`zenity`/`kdialog`/`gsettings`) selected in `src/os/`. Reason: a correct native clipboard on X11 needs a selection-owner event loop, and COM/Cocoa dialogs need unsafe FFI we cannot test locally on Mac/Windows. Trade-off: slower calls and a dependency on helper tools on Linux; clipboard falls back to an in-process buffer. Can be replaced per-OS later behind the same traits.
- **Menus and tray**: data model in `menu`/`tray`; `ShellContext::set_menu/set_tray` install them. macOS: NSMenu + NSStatusItem via objc2. Windows: HMENU menu bar + `Shell_NotifyIconW` tray via windows-sys, using winit's message hook. Activations arrive as `PlatformEvent::MenuAction(id)` / `TrayClicked`. Linux: tray is a StatusNotifierItem over our own minimal D-Bus client (`os/dbus.rs`; icon, tooltip, click, context menu via com.canonical.dbusmenu). No menu bar on Linux (returns an error). Menu shortcuts are display-only on Windows; handle them as key events.
- Idle: `ControlFlow::Wait`, redraw only on request; background threads wake the loop via `ShellProxy`.

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
| Native menu bar + tray | n/a (unsupported) | type-checked + clippy only, never run | type-checked + clippy only, never run |
| Wayland | not run | n/a | n/a |

CI needs `libxkbcommon-x11-0` on Ubuntu to run the smoke example (winit loads it at runtime).
