# Editors, charts and icons

## 46. Show Markdown

`markdown(src)` renders headings, lists, quotes, code and links with the theme's type. Links come back to your handler instead of opening a browser.

```rust
use alloy_ui::prelude::*;

#[derive(Default)]
struct Help {
    last: String,
}

fn view(s: &Help) -> impl View<Act<Help>> {
    col![
        scroll(markdown("# Shortcuts\n\n- **Cmd+K** opens the palette\n- See [the guide](guide)").link(|s: &mut Help, url| s.last = url))
            .fill_h(),
        caption(s.last.clone()).dim(),
    ]
    .gap(8.0)
    .pad(16.0)
}
# fn main() { let _ = sketch(Help::default(), view); }
```

## 47. A code editor

`code_editor` brings line numbers, syntax colours, undo and a status bar. Bind it to a `String` like any input.

```rust
use alloy_ui::prelude::*;

struct Snippet {
    src: String,
}

fn view(s: &Snippet) -> impl View<Act<Snippet>> {
    code_editor(at!(s.src)).file("main.rs").lang(Lang::Rust).fill_h()
}

fn main() {
    let _ = sketch(Snippet { src: "fn main() {\n    println!(\"hi\");\n}\n".into() }, view);
}
```

## 48. A rich text field

`rich_editor` edits bold, italic, headings and links, and keeps the document as a value you can save or convert to Markdown.

```rust
use alloy_ui::prelude::*;

struct Note {
    doc: RichText,
}

fn view(s: &Note) -> impl View<Act<Note>> {
    rich_editor(at!(s.doc)).placeholder("Write something").fill_h()
}

fn main() {
    let _ = sketch(Note { doc: RichText::from_markdown("# Title\n\nSome **bold** words.") }, view);
}
```

## 49. Icons beside labels

`icon(Icon::..)` draws from the built-in icon fonts, so it takes the text colour and scales with the theme. `.accent()` and `.dim()` tint it.

```rust
use alloy_ui::prelude::*;

fn view(_: &()) -> impl View<Act<()>> {
    col![
        row![icon(Icon::File).dim(), text("notes.md")].gap(8.0).align_center(),
        row![icon(Icon::Info).accent(), text("Everything is up to date")].gap(8.0).align_center(),
    ]
    .gap(10.0)
    .pad(20.0)
}
# fn main() { let _ = sketch((), view); }
```

## 50. Pick a colour

`color_picker` binds to a `Color` and reports each change, so a swatch or a whole theme can follow it live.

```rust
use alloy_ui::prelude::*;

struct Brand {
    color: Color,
}

fn view(s: &Brand) -> impl View<Act<Brand>> {
    col![
        color_picker(at!(s.color)),
        text("Preview").pad_xy(16.0, 10.0).bg(s.color).round(6.0),
    ]
    .gap(12.0)
    .pad(20.0)
}
# fn main() { let _ = sketch(Brand { color: Color::hex(0x6D4AFF) }, view); }
```
