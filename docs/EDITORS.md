# Data grid, code editor, rich text

Three editors in `alloy-ui-widgets`, with one-call facades in `alloy-ui`. Demo: `cargo run -p alloy-ui --example studio` (111 lines: a 100 000-row grid, the code editor showing its own source, a rich-text note). Renders: `gallery/widgets3/`.

```rust
data_grid(&s.orders)                                  // Arc<Vec<Order>>
    .col("Customer", |o| o.customer.clone()).editable().flex()
    .col("Packed", |o| GridCell::bar(o.packed))
    .col("Status", |o| GridCell::status(tone, o.status))
    .col("Total", |o| format!("${:.2}", o.total)).right()
    .edit(|s: &mut App, row, _col, text| ..)
code_editor(at!(s.src)).file("main.rs")
rich_editor(at!(s.doc)).placeholder("Start writing")   // s.doc.to_markdown() / RichText::from_markdown(..)
```

## Data grid
Virtualised (only visible cells are asked for; a test pins the call count), sortable headers (numbers sort as numbers), drag-to-resize columns with double-click fit, row selection stored as ranges (click, Shift, Ctrl/Cmd, Ctrl+A, arrows, Ctrl+C as TSV), inline editing (double-click, Enter, F2 or just type; Enter/Tab commit, Escape cancels), status dot and bar cells.

## Code editor
Own line lexer (`highlight.rs`: Rust, Python, JS/TS, JSON, TOML, shell, C/C++, SQL; state carried per line, so any visible line costs one pass from a cached state). Line numbers, current-line band, indent guides, auto-indent, bracket and quote pairing, Tab/Shift+Tab block indent, Ctrl+/ comments, Ctrl+D duplicate, Ctrl+F find (smart case, n of m, Enter/Shift+Enter, F3), status bar, tabs and wide characters in cell math, scrollbars.

## Rich text
`RichText` is blocks of styled runs addressed by flat character offsets (blocks joined by `\n`), so selection and word movement reuse the plain-text helpers. Bold, italic, underline, inline code, links (Ctrl/Cmd+K, Ctrl/Cmd-click opens), H1 to H3, bullets, numbers, quotes, markdown typing shortcuts, undo/redo with typing coalescing, wrapping layout cached per width, toolbar. Italic is drawn as medium weight until a bundled italic face exists (the toolbar glyph is a drawn slanted I).

## Notes for other crates
- `Ui::modifiers()` exists because mouse events carry none; the platform shell now sends a `KeyReleased(Named("Modifiers"))` on modifier changes and `Root` records them.
- `Ui::state::<T>(id, ..)` is a typed per-widget store.
- The headless harness metrics now honour weight and family (they ignored them, so mono and bold measured as regular sans).

## Not done
Grid: column reorder and freeze, cell types beyond text/status/bar. Code: multi-cursor, folding, bracket-match highlight, replace. Rich: nested lists, images, tables, real italic. Windows and macOS behaviour is unverified; everything here ran on Linux only, and CI is blocked by billing.
