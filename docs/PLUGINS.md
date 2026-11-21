# Writing an Alloy plugin

A plugin is a normal Rust crate that adds widgets (and optionally translations) to Alloy apps.
There is no dynamic loading, no ABI, no manifest: the crate is compiled into the app, so it is
type-checked against `alloy-ui`, costs nothing when it is not used, and works on every OS Alloy
does. The worked example is [`examples/plugin-rating`](../examples/plugin-rating), a star
rating widget with tests, about 230 lines.

## The shape of a plugin crate

```toml
[dependencies]
# The facade crate only. `default-features = false` is what keeps the *app's* choice of
# renderer (default wgpu + CPU fallback, or the slim CPU-only build) in force.
alloy-ui = { version = "0.0.1", default-features = false }
```

1. **A widget** is a struct implementing `alloy_ui::Widget<M>` (`measure`, `on_event`, `draw`,
   plus `children`/`arrange` for containers). `M` is the app's message type.
2. **Builder functions** are what apps call. For `sketch`-style apps return something that
   converts into `Node<Act<S>>`; for `App`-style apps take a message closure:

   ```rust,ignore
   #[track_caller]                       // focus id derived from the call site
   pub fn rating<S: 'static>(value: u8) -> Rating<Act<S>> { /* ... */ }
   impl<M: 'static> From<Rating<M>> for Node<M> { /* Node::from(Element::new(r)) */ }
   ```
3. **An optional `Plugin`** bundles non-widget contributions (today: translations) and is
   installed with one call, once, before the first view:

   ```rust,ignore
   alloy_ui::plugin::install(&RatingPlugin);
   ```

   `alloy_ui::plugin::installed()` lists `(name, version)` for an About box or bug report.

## What `alloy_ui::ext` gives a widget

Everything the built-in widgets use internally that an outside widget also needs, as plain
functions (see the rustdoc for each):

| Need | Helper |
|---|---|
| Join the Tab order | `ext::focusable(ui, id)` (call from `draw`) |
| Focus ring identical to the built-ins | `ext::focus_ring(bounds, radius, color)` |
| State that survives `view` rebuilds | `ext::state` / `ext::set_state` (integer slots) |
| Hover, and a press that started here | `ext::hovered`, `ext::press`, `ext::was_pressed`, `ext::release` |
| Screen-reader description | `ext::describe(ui, Role::Slider, name, &bounds, \|n\| ..)` |
| Skip drawing when off-screen | `ext::culled(ui, &rect)` |
| Colour maths on theme colours | `ext::mix`, `ext::over` |
| Containers | `ext::draw_children`, `ext::dispatch`, `widgets::arrange_widget` |
| The app's `Ui`, theme, a stable id | `ext::ui()`, `ext::theme()`, `ext::auto_id(salt)` |

Always read colours, radii and spacing from `ext::theme()` / `ui.theme()` rather than
hard-coding them, so the plugin follows light/dark, accent and density like every other widget.
Use `Ui::ease` / `Ui::spring` for motion; they honour reduced-motion for free.

## Conventions

* **Name** the crate `alloy-ui-<thing>` (crates.io: check the name is free first).
* **Never** depend on `alloy-ui-core`, `-widgets` or `-render` directly; `alloy_ui::widgets`,
  `alloy_ui::render` etc. are re-exported, and only the facade's API is the compatibility promise.
* **Features**: forward none; do not enable `alloy-ui`'s `gpu` / `soft-window` / `gles`. The app picks.
* **Namespacing**: prefix translation keys with the plugin name (`rating.value`) and slot-state
  tags with a constant of your own (see `HOVER_TAG` in the example).
* **Tests**: drive the widget directly (`Widget::on_event` with a hand-built `Layout`) and, for
  the whole path, with `alloy_ui::testing::Harness`. Both run headless on all three OSes.
* **Docs**: a `//!` example that builds a `sketch` app; it runs as a doc-test.

## What is deliberately not here

* Loading plugins at run time (`dlopen`, WASM). Rust has no stable ABI; a compiled-in crate
  gives full type checking and zero overhead. A WASM sandbox for untrusted extensions is a
  possible later layer, not a replacement for this.
* Theme or shortcut registration from a plugin. Apps own those; a plugin exposes builders the
  app wires up. If a plugin needs to be told about the theme it reads it with `ext::theme()`.
