# Theming

Alloy ships two designed themes, light and dark, and follows the system setting on every OS. The visual rules live in [DESIGN.md](../../DESIGN.md): one deep teal-green accent, cool neutrals, small radii, shadows only for elevation, short eased-out motion, and every text colour tested against WCAG AA contrast.

## Light and dark

By default a window follows the system appearance. To let the user choose, add a switch:

```rust,no_run
use alloy_ui::prelude::*;

fn main() {
    app("Settings")
        .page("General", || section("Theme", theme_switch()))
        .run()
}
```

Or set it yourself from a handler: `set_dark(true)` switches the current window.

```rust
use alloy_ui::prelude::*;

fn view(dark: &bool) -> impl View<Act<bool>> {
    choose([("Light", false), ("Dark", true)], *dark).act(|d: &mut bool, v| {
        *d = v;
        set_dark(v);
    })
}
# fn main() { let _ = sketch(false, view); }
```

## Reading the palette

Inside `view`, `palette()` returns the current theme's colours so custom drawing follows light and dark:

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    let p = palette();
    col![text("Surface").color(p.text)].bg(p.surface).pad(16.0)
}
# fn main() { let _ = sketch((), view); }
```

Prefer the semantic helpers (`.dim()`, `.accent()`, `.danger()`, `.surface()`, `.card()`) to raw colours: they are what keep a screen consistent.

## Tokens

The `alloy_ui::theme` crate (`alloy-ui-theme`) holds every token: `Theme::light()` and `Theme::dark()`, the palette, the type scale (display, heading, body, caption at a 1.5 line height), spacing, radii (2, 4 and 6 px), elevation and motion. `ThemeMode` is a user preference (`Light`, `Dark`, `System`) that resolves to an `Appearance`. Install a custom `Theme` on the low-level `Ui` with `Ui::set_theme`.

## Brand accent, high contrast and density

A `Look` describes the whole window in one value. Every part is optional, and it can be applied again at any time (a settings page, a keyboard shortcut) with no restart:

```ignore
set_look(
    Look::light()
        .is_dark(state.dark)
        .accent(Color::hex(0x4338CA))   // any colour; nudged until it passes WCAG AA
        .high_contrast(state.contrast)  // 7:1 text, solid borders, a bold focus ring
        .density(Density::Compact),     // Compact, Comfortable or Spacious spacing
);
```

`set_accent(color)` changes only the accent. The same pieces exist on `Theme` (`with_accent`, `high_contrast`, `density`) for the low-level API. Light and dark themes now also carry a `warning` colour, so amber is a palette role like `danger` and `success`.

`cargo run -p alloy-ui --example showcase` shows all of it live, along with the new badges, swatches, charts with axes, sparklines, tree and colour picker.
