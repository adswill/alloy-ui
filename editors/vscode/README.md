# Alloy for VS Code

Snippets for writing [Alloy](https://github.com/adswill/alloy-ui) apps in Rust, and syntax highlighting for `.alloy` live views.

## Rust snippets

Type a prefix in a `.rs` file and press Tab. Whole-program snippets give you a complete app to run; the short ones drop a widget into the `col![...]` you are writing.

| prefix | inserts |
|---|---|
| `alloy-sketch` | a complete app: state plus a view function |
| `alloy-state` | a state struct, a view with a bound input and `main` |
| `alloy-app` | a sidebar app with pages, theme switch and saved state |
| `alloy-elm` | the `App` trait: `Msg`, `update_cmd`, `view` |
| `alloy-choices` | a `choices!` enum with a bound `options` control |
| `alloy-record` | a `derive(Record)` struct shown as a CRUD screen |
| `alloy-resource` | background loading with loading, error and retry |
| `alloy-shortcut` | a shortcut and a palette command |
| `alloy-test` | a headless `Harness` test |
| `alloy-i18n` | catalogs, `set_locale` and `t!` |
| `alloy-live` | a `live!` view wired to state and handlers |
| `alloy-col`, `alloy-row` | a column or row with gap and padding |
| `alloy-input`, `alloy-check`, `alloy-slider` | bound controls using `at!` |
| `alloy-button` | a primary button with a handler |
| `alloy-field` | a labelled control with a validation message |
| `alloy-table` | a sortable table with text and number columns |
| `alloy-toast`, `alloy-local` | a toast with Undo, a `local` value |

## `.alloy` live views

Files ending in `.alloy` are the small indented markup that `live!("ui/home.alloy")` reloads on save under `alloy dev`. The extension colours elements, attributes, flags, `@actions`, `{variables}` inside text and colours, and marks an unknown element in red, the same mistake the runtime reports as `unknown element`.

```alloy
# ui/home.alloy
theme dark accent=#6D4AFF
col page gap=14
  heading "Hello, {name}"
  text "{count} so far" dim
  row gap=8 center
    button "Add" primary @add
    button "Reset" ghost @reset
```

Snippets for the file itself: `col`, `row`, `heading`, `text`, `button`, `theme` and `alloy-counter`. Comments start with `#`, and the editor indents after a `col` or `row` line.

`live!` and the `.alloy` format ship with the live-views change (#49). The grammar follows the element, attribute and flag lists documented there. If the format changes, `syntaxes/alloy.tmLanguage.json` and the checks in `test/` are the two places to update.

## Checked, not just written

* `test/grammar.test.js` tokenises sample views with the same TextMate engine VS Code uses and asserts the scope of each piece (`npm install && npm test`).
* `test/check_snippets.py` expands every Rust snippet the way VS Code does and compiles it against the real `alloy-ui` crate, so a snippet cannot drift from the API.

## Install from source

This extension is not on the Marketplace. To try it:

```sh
cd editors/vscode
npx @vscode/vsce package          # writes alloy-ui-0.1.0.vsix
code --install-extension alloy-ui-0.1.0.vsix
```

Licensed under MIT.
