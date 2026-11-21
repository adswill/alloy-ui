# Alloy Studio

One app that uses nearly everything Alloy ships, so you can click around instead of reading docs.

```sh
cargo run --release -p alloy-ui --example alloy_studio
```

The first release build takes a few minutes (fat LTO). For a quicker, slower-running build use `cargo run -p alloy-ui --example alloy_studio`.

Needs Rust 1.80+. On macOS nothing else is required. On Linux install `libxkbcommon-x11-0` and a Vulkan driver (`mesa-vulkan-drivers`); on Windows nothing.

Environment switches: `STUDIO_DARK=1` starts dark, `STUDIO_PRESET=0..5` picks a theme, `LANG=ar_AE.UTF-8` starts in Arabic (right-to-left). Screenshots without a window: add `--headless out.png --click Data`.

## Tour

| Page | Try this |
|---|---|
| Overview | Switch 7/30/90 days, hover the charts, click a donut slice, delete a task and press Undo in the toast, open Export. |
| Data | Sort the 100 000 row grid, drag a header edge, double-click a customer to rename. Customers tab: click a row (router drill-down), Alt+Left to go back. |
| Editors | Edit the code (it is this page's source), type in the rich editor, edit Markdown and watch the split preview. |
| Planner | Drag kanban cards, then Cmd/Ctrl+Z. Calendar: drag events, drag an edge to resize, click an empty slot to create. |
| Canvas | Wire the node graph, draw by hand, orbit the 3D scene, play the media test card. |
| Forms | Profile (validation after touch), the wizard, and the CRUD table. |
| Workbench | Tree, split panes, right-click for a context menu, the 100 000 row build list, drag files between lists. |
| Motion | Reveal, eased values, springs, toasts, elevation. |
| Appearance | Six presets, dark, accent, density, icon pack, window effect, language, contrast report. |
| System | Notifications, file dialogs, clipboard flavours, dropping files on the window, the monitor window, activity log. |

Everywhere: `Cmd/Ctrl+K` command palette (every page and menu item), `Cmd/Ctrl+1..0` jump to a page, `Cmd/Ctrl+N` second window, `Cmd/Ctrl+Z` undo, `F12` inspector, `Ctrl+Alt+S` global hotkey, tray icon (macOS menu bar), the native menu bar.

## What is verified

Run for real under Xvfb with software Vulkan on Linux (windowed navigation, second window, headless renders of every page, Arabic). The macOS and Windows paths of menus, tray, notifications, dialogs, hotkeys and window effects are compiled but have not been run by the author, so expect small differences there; failures are logged and skipped, never fatal. On macOS notifications are attributed to Script Editor until the app is bundled, and `alloy-studio://` links need a bundle.
