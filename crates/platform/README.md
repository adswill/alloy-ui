# alloy-ui-platform

Window/event shell and OS services. No `unsafe` (`#![forbid(unsafe_code)]`).

## Decisions
- **winit on all OSes** for windows, input, IME, DnD, scale factor, theme-change events. One code path covers X11 and Wayland; backends are chosen by winit at runtime.
- **Clipboard, dialogs, theme via stock OS tools** (`pbcopy`/`osascript`/`defaults`, `powershell`/`clip`/`reg`, `wl-copy`/`xclip`/`xsel`/`zenity`/`kdialog`/`gsettings`) selected in `src/os/`. Reason: a correct native clipboard on X11 needs a selection-owner event loop, and COM/Cocoa dialogs need unsafe FFI we cannot test locally on Mac/Windows. Trade-off: slower calls and a dependency on helper tools on Linux; clipboard falls back to an in-process buffer. Can be replaced per-OS later behind the same traits.
- **Menus and tray**: data model, shortcut formatting and backend traits only. Native backends are NOT implemented (they return an error).
- Idle: `ControlFlow::Wait`, redraw only on request; background threads wake the loop via `ShellProxy`.

## Verification
| Item | Linux | macOS | Windows |
|---|---|---|---|
| Unit tests (convert, theme parsers, fonts, menu, tray) | run locally | CI only | CI only |
| Real window, redraw, cross-thread wake (`examples/smoke`, X11 under Xvfb) | run locally | not run | not run |
| Clipboard/dialogs/theme backends | compiled | compiled in CI | compiled in CI |
| Wayland | not run | n/a | n/a |

CI needs `libxkbcommon-x11-0` on Ubuntu to run the smoke example (winit loads it at runtime).
