# Alloy architecture

## Layers
```
examples / apps
  alloy-ui-widgets   built-in widgets          -> core, layout, theme
  alloy-ui-theme     palette, type, spacing    -> core
  alloy-ui-layout    flex layout engine        -> core
  alloy-ui-render    wgpu + headless renderer  -> core (consumes DrawList)
  alloy-ui-platform  window/menu/tray/dialogs  -> core (traits; per-OS modules)
  alloy-ui-ipc       typed commands, capabilities
  alloy-ui-core      geometry, Widget, Element, Event, DrawCommand, Application
  alloy-ui-cli       `alloy` build/bundle binary
```
Widgets never see wgpu or a window. They emit `DrawCommand`s into a `DrawList`; the renderer consumes it. The shell turns OS events into `core::Event`. This keeps widgets testable headlessly.

## Frame loop
event -> widgets' `on_event` push messages -> `Application::update` -> `Command`s run in the background -> `view` rebuilds the tree -> layout -> `draw` -> renderer. Redraw only when state or animation changes, so idle CPU stays near zero.

## What we took from iced (reference only, no code copied)
- State / Message / update / view as the application contract; a view tree rebuilt each update.
- Commands for async side effects and subscriptions for ongoing event sources.
- Differences: our renderer interface is a plain command list (`DrawList`) rather than a generic `Renderer` type parameter on every widget, which keeps widget signatures small. Widget state lives in a runtime-owned tree keyed by position. Layout is our own flex model with Fill(weight)/Shrink/Fixed.

## What we took from Tauri (reference only)
- The shell philosophy: small binary, quick start, low idle CPU, native windows/menus/tray/dialogs/clipboard.
- Typed commands between background work and the UI, and a capability allow-list in a config file.
- Differences: no webview and no JS bridge. "Commands" are Rust futures producing messages; capabilities gate platform APIs at the Rust level.

## Verification policy
macOS may be checked locally; Linux and Windows are only verified by the CI matrix. Docs state which is which.
