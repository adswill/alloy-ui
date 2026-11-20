# Benchmarks

Only numbers that were actually measured are listed. Everything below ran on one
machine: the Linux cloud development container (shared CPU, software rendering
through Mesa llvmpipe under Xvfb). Treat it as indicative, not as a GPU result.
macOS and Windows have not been run yet; run the harness there and add a section.

## Headless harness (Linux, software rendering)

`cargo run --release -p alloy-ui-bench` (the five examples in `examples/`, driven through `alloy_ui::testing::Harness`).

- **First frame**: build the runtime, view + layout + draw, rasterise at 2x on the CPU, encode PNG.
- **Repeat frame**: the same again with warm font caches (median of 15). The CPU reference rasteriser is not the GPU path.
- **Per no-op key event**: runtime cost of an input event that changes nothing (a bound on idle work, not process CPU%).
- **Spawn to first frame**: wall time from spawning `counter --headless` to its PNG being written.
- **Binary size**: the stripped release `counter`.

The earlier wgpu-offscreen column was dropped when the examples moved to the facade: `Harness` does not expose the draw list yet.

### linux / x86_64 (cloud container, shared CPU)

| Example | First frame | Repeat frame | Per no-op key event |
|---|---|---|---|
| counter | 26.0 ms | 17.9 ms | 5 us |
| todo | 60.9 ms | 42.1 ms | 8 us |
| text_editor | 84.7 ms | 69.6 ms | 27 us |
| settings | 55.8 ms | 52.3 ms | 7 us |
| dashboard | 86.6 ms | 78.5 ms | 32 us |

| Metric | Value | Target |
|---|---|---|
| counter release binary (windowed build, also runs headless) | 7.15 MB | < 10 MB |
| counter spawn to first frame written (--headless, median of 7) | 25 ms | < 200 ms |

## Windowed run (Linux, Xvfb, Mesa llvmpipe software rendering)

`crates/bench/windowed.py target/release counter todo text_editor settings dashboard`
(needs `xvfb xdotool libxkbcommon-x11-0 mesa-vulkan-drivers libvulkan1` and `pip install python-xlib`).
The shell's frame counter is read through `ALLOY_PERF=1` (`ctx.frame_stats()`), and the window is closed with `WM_DELETE_WINDOW` so the totals are printed.

**These are Linux, software-rendered numbers, not GPU numbers.** There is no GPU in the container, and nothing here was run on macOS or Windows (CI has never completed a three-OS run).

- **First paint**: from entering the shell's `run` to the end of the first painted frame (the shell's own measure; excludes process start).
- **Idle frames**: frames painted in a run of about 6.5 s with the window untouched. Idle apps should paint only a few at start; `dashboard` has a 1 s tick so it keeps painting.
- **Idle CPU**: CPU time over 5 s after 1.5 s of settling.
- **Frames/s, pointer moving**: a separate run in which `xdotool` moves the pointer across the window for 5 s. `xdotool` is spawned once per move (about 30 ms each), so a figure near 30 is input-limited, not render-limited.

| Example | First paint | Idle frames (≈6.5 s run) | Idle CPU (5 s) | Frames/s, pointer moving | Resident memory |
|---|---|---|---|---|---|
| counter | 87 ms | 1 | 0.00% | 36 | 122 MB |
| todo | 91 ms | 7 | 0.00% | 49 | 125 MB |
| text_editor | 106 ms | 7 | 0.00% | 4 | 131 MB |
| settings | 95 ms | 1 | 0.00% | 34 | 123 MB |
| dashboard | 92 ms | 14 | 1.40% | 82 | 127 MB |

Reading it: first paint is 90 to 110 ms and idle CPU is 0.00% for every example except the ticking dashboard (1.4%). The `text_editor` figure of 4 frames/s while the pointer moves over it is a real outlier and is render-limited: the CPU path pays for the whole text area on every hover redraw. Resident memory is dominated by Mesa llvmpipe and wgpu in this container.

## Render and text path profile (Linux, llvmpipe)

All numbers: Linux x86_64 cloud container, software Vulkan (Mesa llvmpipe), not a GPU. macOS and Windows not run.

`cargo run --release -p alloy-ui-bench --bin text` times `TextMeasurer::measure` and `TextSystem::layout` on 60 repeated short strings (the shape of a no-op re-layout or an unchanged frame):

| Operation | Before | After (shaping cache) |
|---|---|---|
| measure, per string | 149 us | 0.38 us |
| glyph layout, per string | 149 us | 0.58 us |

`cargo run --release -p alloy-ui-bench --bin phases [dashboard]` splits one frame after state setup (2x scale, 1040x520 for text_editor):

| Phase | text_editor | dashboard |
|---|---|---|
| input event + view + layout | 3 us | 13 us |
| draw list | 1 us | 4 us |
| flatten + instance build + encode + submit (CPU side of the GPU path) | about 0.2 ms | about 0.2 ms |
| GPU offscreen incl. read-back (llvmpipe rasterising) | 9.4 ms | 16 ms |
| CPU reference rasteriser, warm | 33 ms | 49 ms |

Reading: on the CPU side a whole frame is about 0.2 ms; what remains is fragment work on the software adapter, which a real GPU does not pay. The wgpu offscreen path therefore bounds text_editor near 100 fps and the dashboard near 60 fps on llvmpipe. The reported 4 fps for text_editor under pointer movement could not be reproduced: in this Xvfb session `xdotool` input never reached the window (no redraws), so the window path was timed only for its first two frames (26 ms, then 3.4 ms including present). Not render or layout CPU; the shaping cost it could have hit is now cached.

Binary: dropping the wgpu `gles` feature saves 0.38 MB (7.49 to 7.12 MB). Not applied: it removes the OpenGL fallback for machines without Vulkan. Six bundled Plex fonts are 1.15 MB of the binary.

## Not measured yet

- 120 fps on ProMotion, and any frame rate on a real GPU: the container has none.
- Everything on macOS and Windows.

## Developer experience: lines of code vs iced

Same five apps in idiomatic iced 0.14 and in Alloy's concise API (`crates/alloy/examples`).
The iced versions live in `crates/alloy/loc/iced/*.rs.txt` as text only (never a dependency);
each was compiled once in a scratch crate with `iced = "0.14"` to make sure it is real code.
Lines are non-blank, non-comment lines after default `cargo fmt`; tokens come from a simple
lexer. Reproduce with `python3 crates/alloy/loc/count.py`.

| App | iced lines | Alloy lines | ratio | iced tokens | Alloy tokens | ratio |
|---|---|---|---|---|---|---|
| counter | 41 | 19 | 2.2x | 275 | 136 | 2.0x |
| todo | 127 | 66 | 1.9x | 780 | 526 | 1.5x |
| text_editor | 42 | 19 | 2.2x | 262 | 134 | 2.0x |
| settings | 126 | 64 | 2.0x | 803 | 390 | 2.1x |
| dashboard | 107 | 73 | 1.5x | 877 | 590 | 1.5x |
| **total** | 443 | 241 | 1.8x | 2997 | 1776 | 1.7x |

What this does and does not say:

- Alloy needs roughly half the lines, not a tenth. These apps are small, and what is left after
  the UI boilerplate is real application logic (the todo's add/filter rules, the dashboard's data
  and random walk), which no framework can remove. The counter, where there is no logic, is the
  closest to the framework's own overhead: 19 vs 41 lines.
- The text editor compares Alloy's string-based `editor` with iced's `text_editor`; neither shows a
  caret position in the status line (the old Alloy example did, with a hand-written widget).
- Not equal visually: each toolkit keeps its own look. Alloy's choose/segmented control replaces
  the hand-rolled button row that the iced versions need; that is a real difference in built-in
  widgets, and the numbers include it.
- Elm style (`App` trait with a `Msg` enum) costs more: the counter is 37 lines (`counter_elm`).
  The closure style is the short path; both are typed.

### Bigger app: CRUD admin panel (pass 2)

Small demos cannot show a 10x gap because most of their lines are app logic. A realistic admin
panel has a lot of UI plumbing that iced makes you hand-write. The app: sidebar with two pages;
people table with search, sortable headers (text and numeric), row selection and per-row delete;
add button; edit form for the selected person (text, email with validation, drop-down, slider,
checkbox, "Required" and "Enter a valid email" messages); settings page with a Light/Dark switch;
people, selection and page persisted to JSON in the config directory. The iced version
(`loc/iced/admin.rs.txt`, 316 lines) was compiled with `iced = "0.14"` plus `serde_json` and `dirs`.

| Alloy version | what it uses | Alloy lines | vs iced 316 (lines) | tokens |
|---|---|---|---|---|
| `admin_manual.rs` | `table`, `field`, `local`, `persist` only (no derive) | 156 | 2.0x | 2.2x |
| `admin.rs` | `#[derive(Record)]`, `choices!`, `app().page()`, `crud`, `theme_switch`, `persist` | 27 | **11.7x** | **12.9x** |
| `crud.rs` | `crud_app(title, seed)`: whole app is one struct plus one call | 19 | 16.6x | 23x |

Read these honestly:

- The 11.7x row is the same app feature for feature (pass 2 was 48 lines, 6.6x). What changed in
  pass 3: pages own their state (`store`, saved with `.persist()`), so there is no state struct,
  page enum, `view` function or lens; field types pick the controls (a `choices!` enum is a
  drop-down, `email` is validated by name, the first field is required); the derive also writes
  the serde impls. The 27 lines include the four seed rows.
- Be careful what this says. The admin app is shaped like the library's best case (a record
  editor). The next section shows what happens on apps that are not.
- The 17.6x row compares a CRUD-only app (no sidebar or settings page) against the full 316-line
  iced app, so it flatters Alloy; against the roughly 270 lines of iced that implement the CRUD
  part alone it is about 15x. It says what happens when an app is mostly a record editor.
- The gap comes from the data-shaped blocks: one derive replaces the per-field form, validation,
  message variants and update arms (iced needs a `Message` variant and an update arm per field);
  `table` replaces header buttons, sort state, filtering and row buttons.
- An iced user could write the same generic helper once. Alloy ships it, tested and styled.
- Visuals differ, and the iced version was not tuned for looks.
- Not measured: runtime performance of the table (it re-sorts every frame; fine for thousands of
  rows, not for hundreds of thousands) and Windows/macOS rendering (CI is the source of truth).

### Does it generalise? Two other apps (pass 3)

Two more realistic iced 0.14 apps were written first (compiled, in a scratch crate that is not
a dependency; text copies in `loc/iced`), then the Alloy versions. Both are different in kind
from the admin panel. Lines are non-blank, non-comment lines after `cargo fmt`
(`python3 crates/alloy/loc/count.py`).

| App | what it does | iced | Alloy | lines | tokens |
|---|---|---|---|---|---|
| `status.rs` | async fetch with loading, error + Retry and optional auto-refresh; stat cards, status filter, sortable table, detail chart | 332 | 110 | **3.0x** | 2.9x |
| `notes.rs` | searchable/sortable note list, editor per note, pin, delete with confirmation, word count, autosave, settings page, theme | 377 | 159 | **2.4x** | 2.1x |

Honest reading:

- Both fall far short of 10x. A 10x-or-better ratio only appears when the app is "a table plus a
  form over a struct". Once the app has its own logic (sorting rules, titles from the first
  line, a fake backend) that logic is the same in both frameworks and dominates.
- The fake backend alone (`fetch`, 28 lines in iced, 30 in Alloy) is over a quarter of the Alloy
  status app. Without it the rest is 304 vs 80 lines, 3.8x.
- Where Alloy saved lines: no `Message` enum and `update` (closures over view-local state), the
  async state machine (`resource` replaces the `Loading/Ready/Failed` enum, the `Task::perform`
  plumbing and the retry message), derived table columns, `Copy` state handles (no `.clone()`).
- Where it did not: list rows, layout, handlers that touch several pieces of state. Alloy handlers
  are closures, so each still names what it changes.
- Not covered: **multi-window**. The core now supports opening windows, but the facade does not
  expose it yet, so the second app is a single window with pages.
- Limits found while doing this: the status app's async fetch runs on a std-only executor
  (`sleep`, plain futures); a tokio-based client would need its own runtime inside the fetch.
  `list` has no empty-state or search integration, and `editor` has no font option (the iced
  notes app had a monospace toggle that was removed for parity).

### Pass 4: list-detail, cells, confirm, stat cards (status and notes)

New facade features, all general (`crates/alloy/src/collections.rs`): `Cell` handles (`Local` or one field of one list row) with `.input()`, `.editor()`, `.check()`, `.switch()`, `.options()`; `browse(&items)` list-detail with search, pinning, ordering, New and a per-row detail pane; `Button::confirm`; `stat`/`stats`/`tally`/`filter_by` plus `Local::apply`; `Table::detail`; `Resource::auto`/`header`/`page`; `Pages::settings`.

| App | iced | Alloy before | Alloy now | whole app | UI only* |
|---|---|---|---|---|---|
| `status.rs` | 332 | 110 (3.0x) | 64 | **5.2x** | 304 vs 21 lines of `main` = 14x |
| `notes.rs` | 377 | 159 (2.4x) | 72 | **5.2x** | n/a (see below) |

*Status UI-only compares iced minus its 28-line fake backend against the Alloy `main` (the remaining 43 Alloy lines are the data struct and backend, identical in kind in both).
Notes: 27 lines are the `Note` type and its helper (same logic in both); `main` is 45 lines vs about 330 for the iced UI and update code, roughly 7x.

Honest reading: 10x on the whole app was not reached for these two. What is left is app logic (the data type, fake backend, sort rule) plus imports and the `main` shell; the UI is now a few lines per screen. Not done: multi-window in the facade (needs per-window `Ui` in `Host`; left for a core-aware thread), keyboard shortcut declarations, toasts, chart helpers beyond `bars`.

### Pass 5: mock backend, derived list-detail, status board

New general blocks in `crates/alloy`: `Record::mock(rows)` (fake backend with `.delay`, `.fail_every`, `.resource()`), `series` (sample chart data), `Resource::board(title, |row| choice)` with `.trend`/`.detail` (count cards, filter chips, sortable table, detail card), `browse_of` (list-detail derived from a `Record`: first text field is the title, a `Pinned` bool pins, derived form plus Delete, `.foot`, `.sort_by` switch), `Stamp` (edit time, auto-touched), `#[form(long)]`, `Pages::theme_page`.

| App | iced | Alloy | ratio |
|---|---|---|---|
| `status.rs` | 332 | 30 | **11.1x** |
| `notes.rs` | 377 | 26 | **14.5x** |
| `admin.rs` | 316 | 27 | 11.7x (unchanged) |

Whole apps, data and fake backend included, counted with `loc/count.py`. Differences to be honest about: the status refresh no longer jitters latency (static mock rows); the status detail card lists every field plus a bar chart instead of a custom subtitle; notes moved the sort switch from the settings page to the list header, and the detail form's "Pinned" checkbox sits below the editor. Not done: keyboard shortcuts, toasts, undo, detached editor window.
