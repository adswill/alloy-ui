# Localization

Alloy ships a small i18n layer: plain-text catalogs, one macro, plurals, live language switching and right-to-left layout. There is no build step and no message enum.

## Catalogs and `t!`

A catalog is `key = value` lines, one file per language. Register them once, then ask for text with `t!`:

```rust
use alloy_ui::{i18n, t};

i18n::add("en", "hello = Hello, {name}!\nitems.one = {n} item\nitems.other = {n} items");
i18n::add("fr", "hello = Bonjour, {name} !\nitems.one = {n} élément\nitems.other = {n} éléments");

i18n::set_locale("fr");
assert_eq!(t!("hello", name = "Léa"), "Bonjour, Léa !");
assert_eq!(t!("items", n = 2), "2 éléments");
# i18n::set_locale("en");
```

In a real app the catalogs are files in the repository, embedded with `include_str!`:

```rust,no_run
use alloy_ui::i18n;

const EN: &str = "greeting = Good morning"; // include_str!("../locales/en.txt")
const FR: &str = "greeting = Bonjour";       // include_str!("../locales/fr.txt")

fn main() {
    i18n::add("en", EN);
    i18n::add("fr", FR);
    i18n::set_locale(&i18n::detect()); // the system language, English if unknown
    // ... sketch(..).run(..)
}
```

Format rules: `#` starts a comment, `\n` is a line break, a trailing `\` continues a value on the next line, `{name}` is replaced by the argument of that name, and `{{` and `}}` are literal braces.

The key must be a string literal, so `t!(key)` with a variable is a compile error that points you to `i18n::translate(&key, &[])` for keys computed at run time.

## Plurals

Give the entry one line per plural category and pass the count as `n`:

```text
inbox.summary.one   = You have {n} unread message
inbox.summary.other = You have {n} unread messages
```

The category comes from the language's CLDR rule: `one`/`other` for English, `0` and `1` are `one` in French, `one`/`few`/`many` for Russian and Polish, all six (`zero` to `other`) for Arabic, only `other` for Japanese, Chinese and Korean. A language whose catalog lacks the category falls back to `other`.

## Fallback and checking

A key is looked up in the exact locale (`pt-br`), then its language (`pt`), then the fallback locale (English, change it with `i18n::set_fallback`), and finally the key itself is shown and one warning is logged. A missing translation never panics.

Keep catalogs honest in a test:

```rust
use alloy_ui::i18n;

i18n::add("en", "save = Save\ncancel = Cancel");
i18n::add("de", "save = Speichern");
assert_eq!(i18n::missing("de", "en"), ["cancel"]);
```

## Switching language live

`language_picker` is a segmented control that sets the locale. The view is rebuilt after every message, so everything on screen changes at once and the app's state is untouched:

```rust,no_run
use alloy_ui::{i18n, prelude::*};

state! { struct Inbox { unread: u32 = 3 } }

fn view(s: &Inbox) -> impl View<Act<Inbox>> {
    col![
        heading(t!("inbox.title")),
        text(t!("inbox.summary", n = s.unread)),
        language_picker::<Inbox>(&[("en", "English"), ("fr", "Français"), ("ar", "العربية")]),
    ]
    .pad(24.0)
}
```

To switch from your own code call `i18n::set_locale("fr")` inside any handler.

## Right-to-left

For Arabic, Hebrew, Persian, Urdu and the other right-to-left languages `i18n::is_rtl()` is true, and the layout mirrors with no changes to your code:

* every `row!` puts its first child on the right, so a sidebar-and-content app flips;
* a column's start edge is the right one, so left-aligned stacks hug the right side;
* text is shaped bidirectionally (Latin names inside Arabic sentences keep their direction), and Arabic and Hebrew keep their joining at every weight.

Not mirrored yet: icons that point somewhere, table column order and the scrollbar side. Tab order follows the mirrored visual order.

Try it: `cargo run -p alloy-ui --example i18n` (or `LANG=ar cargo run -p alloy-ui --example i18n`), or scaffold a whole app with `alloy new inbox --template localized`.
