# Alloy cookbook

Thirty short recipes. Each one is a complete, working answer to a thing people actually build, and every code block is compiled as a doc-test by `cargo test -p alloy-ui --doc`, so none of them can go stale. Most also drive the app with the headless `Harness` and assert what the screen says afterwards.

1. [State and input](01-state-and-input.md): a counter with a step, a todo list with a filter, search as you type, a validated signup form, a settings form with typed numbers
2. [Layout](02-layout.md): sidebar and content, a toolbar, a scrolling log, a row of cards, an empty state
3. [Data](03-data.md): a sortable table, a master-detail list, a CRUD screen, an async fetch with retry, auto-refresh
4. [Finished-app details](04-app-polish.md): shortcuts, the command palette, toasts with Undo, undo and redo, saved state
5. [Look and language](05-look-and-language.md): a dark-mode switch, a brand accent, custom colours, progress and charts, translations and plurals
6. [Shell and testing](06-shell-and-testing.md): testing without a screen, a screenshot from a test, two windows, capability checks, a typed command

New here? Read the [guide](../guide/README.md) first; the recipes assume `use alloy_ui::prelude::*;`.
