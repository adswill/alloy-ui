# Widgets

Widgets are plain functions. They need no handle, no ids and no root wrapper: `button("Save")` is a complete expression. Stable identity (focus, caret, scroll offset) comes from where you wrote the call.

| you want | use |
|---|---|
| text | `text(s)`, `heading(s)`, `caption(s)`, plus `.dim()`, `.accent()`, `.danger()`, `.success()` |
| a button | `button("Go")`, `.primary()`, `.ghost()`, `.enabled(bool)`, `.confirm("Sure?")` |
| one-line input | `input(&s.name)`, `.hint(..)`, `.secure()`, `.submit(f)`; `number(at!(s.n))` for numbers |
| multi-line input | `editor(&s.body)` |
| on/off | `check("Label", at!(s.on))`, `button("Pause").toggle(at!(s.paused))` |
| one of a few | `radios(..)`, `options(at!(s.filter))`, `choose([("A", 1), ("B", 2)], s.v)`, `tabs(..)` |
| one of many | `pick(items, selected)` (dropdown) |
| a number in a range | `slider(0.0..=1.0, at!(s.v))`, `stepper(at!(s.count))`, `progress(0.0..=1.0, s.p)` |
| lists and tables | `list(rows, selected)`, `table(&rows).col("Name", ..)`, `browse`, `records` |
| navigation | `nav(&s.page).bind(..)` for a sidebar, `shell(..)` sidebar plus page, `wizard(..)` |
| scrolling | `scroll(view)` |
| charts | `bars(..)`, `stat(..)`, `stats(..)` |
| anything else | `custom(element)` wraps a raw `alloy_ui::Element` from your own `Widget` |

## Choices from an enum

`choices!` declares an enum that radio groups, segmented controls and sidebars understand:

```rust
use alloy_ui::prelude::*;

choices! { enum Filter { All, Open, Done = "Finished" } }

#[derive(Default)]
struct S {
    filter: Filter,
}

fn view(s: &S) -> impl View<Act<S>> {
    col![
        options(s.filter).bind(|s: &mut S| &mut s.filter),
        text(format!("showing {:?}", s.filter)),
    ]
}
# fn main() { let _ = sketch(S::default(), view); }
```

The first variant is the `Default`. The label is the variant name unless you give one (`Done = "Finished"`).

## Tables

`table` is sortable (click a header), filterable and keyboard-navigable:

```rust
use alloy_ui::prelude::*;

struct Person {
    name: String,
    age: u32,
}

struct People {
    rows: Vec<Person>,
}

fn view(s: &People) -> impl View<Act<People>> {
    col![table(&s.rows)
        .col("Name", |p: &Person| p.name.clone())
        .num("Age", |p: &Person| p.age.to_string())]
}
# fn main() { let _ = sketch(People { rows: vec![] }, view); }
```

## Keyboard and accessibility

Every interactive widget is focusable in draw order (Tab and Shift+Tab), activates with Space or Enter, and appears in the accessibility tree. AT-SPI is tested on Linux; VoiceOver and Narrator support is compiled but not yet verified on those systems.

## Your own widget

For something Alloy does not ship, implement `alloy_ui::Widget` (measure, draw, on_event) and wrap it with `custom(Element::new(my_widget))`. The built-in widgets are written against the same trait.
