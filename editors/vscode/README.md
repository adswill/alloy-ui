# Alloy for VS Code

Snippets for writing [Alloy](https://github.com/adswill/alloy-ui) apps in Rust, and for `.alloy` live views: highlighting, error squiggles, completion and hover, all from the same parser the running app uses.

## The language server

Install the `alloy` tool once (it also gives you `alloy dev` and `alloy test`):

```sh
cargo install --git https://github.com/adswill/alloy-ui alloy-ui-cli
```

Open any `.alloy` file and the extension starts `alloy lsp` in the background:

* **Diagnostics.** Every mistake is underlined with the message the app itself would show (`unknown element`, `unknown setting`, `is not a flag`, a missing closing quote, bad indentation), all at once, not just the first. A flag or setting that loads but does nothing on that element (`text "x" primary`) is a warning.
* **Completion.** Elements at the start of a line (`theme` only on the first), then the settings and flags that make sense for that element, `@actions` already used in the file, `{variables}` inside quoted text, and the values of `density=` and `contrast=`.
* **Hover.** An element name shows what it does and what it accepts.

Commands (Command Palette): **Alloy: Run with live reload** (`alloy dev` in a terminal), **Alloy: Run tests with snapshot diffs** (`alloy test`), **Alloy: Restart language server**. If `alloy` is not on your PATH, set `alloy.server.path`. Without the tool the extension still gives you colours and snippets and offers to copy the install command.

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

The element table lives in `crates/core/src/live_syntax.rs` and is shared by the runtime and the language server. If the format changes, update that file, `syntaxes/alloy.tmLanguage.json` and the checks in `test/`.

## Checked, not just written

* `test/grammar.test.js` tokenises sample views with the same TextMate engine VS Code uses and asserts the scope of each piece (`npm install && npm test`).
* `test/extension.test.js` loads `extension.js` against a stub of the VS Code API (commands, terminal, restart) and, when an `alloy` binary is present, drives the real server with a JSON-RPC client: diagnostics, completion, hover, an edit that clears the errors, and a clean shutdown. CI sets `ALLOY_BIN` and requires it.
* `test/check_snippets.py` expands every Rust snippet the way VS Code does and compiles it against the real `alloy-ui` crate, so a snippet cannot drift from the API.

## Install from source

This extension is not on the Marketplace. To try it:

```sh
cd editors/vscode
npm ci
npx vsce package                  # writes alloy-ui-0.2.0.vsix
code --install-extension alloy-ui-0.2.0.vsix
```

CI builds the same file on every run (job `vscode`, artifact `alloy-vscode-vsix`) so you can download and install it without a toolchain. It is not published to the Marketplace.

Licensed under MIT.
