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
| counter release binary (windowed build, also runs headless) | 6.41 MB (4.41 MB slim; was 7.15 MB, see the last section) | < 10 MB |
| counter spawn to first frame written (--headless, median of 7) | 25 ms | < 200 ms |

## Bundle sizes (counter, `alloy bundle -p alloy-ui-examples --bin counter`)

Linux x86_64, run locally (the CI jobs that would record Windows and macOS sizes have not run yet, see `crates/cli/ci/`):

| Artifact | Bytes |
|---|---|
| counter binary (release, stripped) | 7,517,208 |
| .deb | 3,212,402 |
| .rpm | 3,213,834 |

Windows (exe, NSIS, MSI) and macOS (.app, .dmg) sizes: not measured. `verify-windows.ps1` and `verify-macos.sh` write them to the job summary when a runner is available.

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

Binary: dropping the wgpu `gles` feature saves 0.38 MB (7.49 to 7.12 MB). Not applied: it removes the OpenGL fallback for machines without Vulkan. Six bundled bundled fonts are 1.15 MB of the binary.

## Large views (Linux, headless, CPU only)

`cargo run --release -p alloy-ui-bench --bin large -- <rows>` drives a scrolling checklist (one `check` per row, 440x470 window) through `Harness` and times: build + first settle (cold text caches), one frame's draw list, and one no-op `Tick` (event, view rebuild, layout, draw, diff). Linux x86_64 cloud container, shared CPU, so figures move 20 to 30% between runs. No GPU is involved.

Two real problems showed up and were fixed:

1. **Text cache cliff.** The shaping cache was cleared wholesale at 4096 entries. A view with more distinct strings than that (10 000 rows) re-shaped every string on every frame, about 150 us each. It is now two generations (an LRU approximation) that doubles its size when it sees keys it just evicted, capped at 262 144 entries.
2. **O(n^2) tab order.** Each focusable widget registered itself with a linear `contains` scan over all earlier ones. Registration is now a hash-set check.

| Rows | Metric | Before | After |
|---|---|---|---|
| 1 000 | no-op tick | 0.68 ms | 0.7 to 1.0 ms (noise) |
| 10 000 | no-op tick (view + layout + draw) | 1 780 ms | 7.5 to 9.6 ms |
| 10 000 | draw list only | 16.1 ms | 1.3 to 1.8 ms |
| 10 000 | build + first settle (cold caches) | 3 370 ms | 1 260 to 1 870 ms |
| 20 000 | no-op tick | not measured before the fix | 17 ms |
| 40 000 | no-op tick | not measured before the fix | 43 ms |

A 10 000-row view therefore goes from about 0.5 frames per second to over 100 on the CPU side. The cost that remains is the cold first pass (about 150 us per distinct string to shape) and rebuilding and laying out every row on each event; real virtualisation (build only visible rows) is the next step and is not done. Draw output still contains commands for off-screen rows (26 673 commands for 10 000 rows); culling them without dropping their tab-order registration needs a widget-contract change, so it was left alone.

Smaller changes: the six bundled fonts are borrowed from the binary instead of copied to the heap at startup (1.15 MB less heap; `TextSystem::new` is about 4 ms with 117 system faces, measured by `--bin startup`).

Tried and not kept: removing winit's `wayland-csd-adwaita` feature changed the counter binary by under 0.1 MB (7.17 MB), so Wayland window decorations stay as they were. Binary size is otherwise unchanged from the figures above; the biggest remaining items are wgpu and the bundled fonts.

## Not measured yet

- 120 fps on ProMotion, and any frame rate on a real GPU: the container has none.
- Everything on macOS and Windows.

## Developer experience: lines of code vs iced

> **Read the audit at the end of this section first.** The 10x to 14x ratios below apply only to apps that are one of Alloy's built-in blocks; ordinary apps measure about 1.6x.

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

### Audit of the "10x fewer lines" claim

An independent review of the numbers above, written as a sceptic. Short version: **the 10x claim
holds only for apps whose whole UI is a library block (a record editor, a status board, a
notes list). On three new apps that no block was built for, Alloy is 1.5x to 1.8x shorter
(1.6x in total). Against a more compact iced version the block-shaped apps shrink too.** Quote
"about 1.6x on ordinary apps, 9x to 14x on apps that are one of Alloy's ready-made blocks".

**Method.** Lines are non-blank, non-comment lines after default `rustfmt`. Until this audit
`count.py` did not run rustfmt, it only claimed to, and `admin.rs.txt` was not rustfmt-clean, so
the iced admin app was undercounted (316; it is 338 formatted, so its ratio rises from 11.7x to
12.5x). `count.py` now formats both sides and strips `/* */` comments. Tokens are counted as before.

| App | iced | Alloy | lines | built for it? |
|---|---|---|---|---|
| `admin` | 338 | 27 | 12.5x | yes: `derive(Record)`, `crud` |
| `status` | 332 | 30 | 11.1x | yes: `Resource::board`, `Service::mock` |
| `status`, compact iced | 267 | 30 | **8.9x** | see below |
| `notes` | 377 | 26 | 14.5x | yes: `browse_of`, `Stamp`, `#[form(long)]` |
| `counter` / `todo` / `text_editor` / `settings` / `dashboard` | 443 | 241 | 1.8x | no |
| **`pomodoro` (new)** | 159 | 101 | **1.6x** | no |
| **`files` (new)** | 222 | 149 | **1.5x** | no |
| **`wizard` (new)** | 238 | 130 | **1.8x** | no |

**(a) Are the iced apps fair?** Mostly. They are idiomatic iced 0.14, compiled, and rustfmt-clean
(except admin, now fixed). They are not minimal. To measure how much a seasoned iced developer
could save, `status_compact.rs.txt` is a rewrite of the status app (all behaviour kept) that drops
the `Status`/`Column` enums and `impl` blocks for const tables and indices, folds `Load` into an
`Option<Result>`, and inlines helpers: 267 lines, 20% shorter than 332. That moves the status
ratio from 11.1x to 8.9x, so it is under 10x. The same trick would likely shave the iced notes and
admin apps by a similar amount (not rewritten, not measured). No iced helper crate was used;
`iced_aw` and similar add widgets (date picker, tabs) but nothing that replaces a table or a form.

**(b) Are the Alloy features general?** Not all of them. `Resource::board(title, |s| s.status)`
renders stat cards per status, filter chips, a sortable table and a detail pane with a trend
chart. `Service::mock(..).fail_every(3, "..")` is the exact fake backend of the iced status app
(700 ms delay, every third fetch fails). `browse_of` is the notes app's UI (search, order, pin,
delete with confirmation, word-count footer). Those are real, reusable blocks for "status board"
and "notes list" apps, but the 11x to 14x is a property of the block, not of the framework.
Three apps written for this audit (iced first, then Alloy, same feature list and spec, written
by the same author, so some bias is possible) give 1.5x to 1.8x. No block applied; the Alloy
code is the ordinary closure API.

**(c) Counting rules.** Now the same on both sides (rustfmt, no comments, no blank lines). The
Alloy `vals![..]` seed rows count as code, as do the iced `const` tables. The earlier
tables silently relied on already-formatted files.

**(d) Does Alloy match the iced feature set?** Not entirely. Differences found, not fixed:

- Status: iced colours the stat numbers and the Status column per status; Alloy's `board` does
  not. The iced detail card has a "Latency, last 16 checks" caption that Alloy's lacks. Alloy
  wraps the app in a one-page sidebar. Refresh-disabled-while-loading was not verified.
- Alloy's table sorted size columns by the digits of their display text ("2.0 KB" before
  "500 B"); found by the files app. Fixed (`Table::key`).
- No password field existed. Added (`input(..).secure()`, widgets `TextInput::secure`).
- No way to disable a button conditionally and no fixed-width input. Added (`Btn::enabled`,
  `Input::w`, `Txt::accent_if`).

**Behaviour checks.** `crates/alloy/tests/audit_apps.rs` drives the three new Alloy apps through
the real runtime (wizard validation, masked password, step flow; file browser navigation, hidden
files, folders first, size sort; pomodoro state machine). The iced versions compile
(`loc/iced/*.rs.txt`, scratch crate with `iced = "0.14"`) but were not run in a window or tested
beyond that; their behaviour was checked by reading. Windowed iced and Alloy runs on macOS
and Windows were not done.

**What still holds:** Alloy needs no message enum, no `Message` plumbing for simple state, and
brings a table, form fields, async resource and persistence as built-ins. Those are real, they
are worth 1.5x to 2x on ordinary apps and 9x or more on block-shaped ones. A claim of "10x fewer
lines than iced" should say "for apps that are mostly a table, a form or a list".

### Pass 7: declared state and fewer handlers

Any-app features only: `state!` (struct plus starting values, so `main` needs no initialiser),
`Wizard::step` (Next is gated by the `field` errors on the page, no separate valid flag),
`radio_list`/`radio_list_with` (radios from a `choices!` enum), `valid::password`/`positive`,
`clock`/`money`, and button list ops `.push/.keep/.clear` on `at!` bindings. Same `count.py`,
same iced files.

| App | iced | Alloy pass 6 | Alloy now | ratio now |
|---|---|---|---|---|
| `pomodoro` | 150 | 66 | 63 | 2.4x |
| `files` | 222 | 69 | 66 | 3.4x |
| `wizard` | 238 | 73 | 66 | 3.6x |
| `split` | 100 | 47 | 43 | 2.3x |
| all four | 710 | 255 | 238 | **3.0x** |
| `todo` (moved to `at!` and `.keep`) | 127 | 66 | 61 | 2.1x |

Still about 3x, not 10x: pass 7 trims about 7%. The remainder is domain code, `view` layout
and rustfmt line breaks. Apps made of ready blocks stay at 9x to 14x.

### Pass 6: general ergonomics for everyday apps

Goal from the audit: raise the ratio on apps no block was built for, with features any app can
use, not with new blocks. Measured with the same `loc/count.py`, same iced files (iced
pomodoro now stores `elapsed` instead of `left`, 150 lines instead of 159, because the Alloy
rewrite derives the remaining time and the iced app can do exactly the same).

| App | iced | Alloy before | Alloy now | ratio before* | ratio now |
|---|---|---|---|---|---|
| `pomodoro` | 150 | 101 | 66 | 1.6x | **2.3x** |
| `files` | 222 | 149 | 69 | 1.5x | **3.2x** |
| `wizard` | 238 | 130 | 73 | 1.8x | **3.3x** |
| `split` (new, unseen) | 100 | 61 | 47 | 1.6x | **2.1x** |
| the three audit apps together | 610 | 380 | 208 | 1.6x | **2.9x** |
| all four | 710 | 441 | 255 | 1.6x | **2.8x** |

\* "before" ratios use the audit's iced file (pomodoro 159 lines); against the new 150-line iced file the old Alloy pomodoro would be 1.5x.

**Not 4x.** Everyday apps sit at 2x to 3.3x. What is left in the Alloy files is state structs,
domain logic (the pomodoro state machine, the bill maths), `format!` lines, and the vertical
growth `rustfmt` gives any widget call with more than about 60 characters. None of that is
framework boilerplate. 4x on ordinary apps would need blocks again (a countdown, a file
browser), which is the thing the audit warned against.

`split` was written (iced first, compiled) before looking for features. With the API as it was
after the first batch of changes it was 61 lines (1.6x). It then showed two general gaps, a
numeric text field and a `- n +` control, which were added (`number`, `stepper`, `.labelled`);
that took it to 47. Treat its 2.1x as "seen once", not as an unbiased sample.

What changed, all in `crates/alloy` (no core changes):

- **Bindings: `at!(s.field)`.** `input(at!(s.name))`, `check("On", at!(s.on))`,
  `slider(.., at!(s.volume))`, `choose(.., at!(s.mode))`, `options(..)`, `tabs(..)` read the field
  and write edits back. A witness argument fixes the state type, so the closure annotation
  `|s: &mut Timer|` is not needed on anything that takes an `at!`. Also `.toggle(at!(s.running))`,
  `.set(at!(s.step), Step::Done)`, `.add(at!(s.n), 1)` on buttons, `radios(at!(s.plan), options)`,
  nested fields and indexing (`at!(s.minutes[i])`). Plain values still work as before.
- **Typed inputs:** `number(at!(s.price))` (parses as you type, refuses non-numbers),
  `stepper(at!(s.people)).min(1).max(9)`, `.labelled("People")`.
- **Layout defaults:** `.page()` is the root of a screen (theme page margins and group spacing);
  `col!`/`row!` already take their gap from the theme. `Slider::label/unit`, `Txt::dim_if`,
  `View::when(bool)`.
- **Timers:** `sketch(..).tick_while(|s| s.running, secs(1.0), Timer::tick)` subscribes only while
  the condition holds (no timer, no wake-ups, when paused); several ticks per app.
- **Wizard:** `wizard(at!(s.step)).page("Account", valid, body)..finish("Create").done(body)`:
  numbered header, Back/Next with Next gated by the page's validity.
- **Files and forms:** `alloy_ui::fs` (`list` cached by directory mtime, folders first; `read`,
  `write`, `human_size`, `Entry`) which honours an `alloy_ui::fs::grant(CapabilitySet)`
  (`fs_read`/`fs_write` scopes; everything is allowed until a grant is installed), and
  `alloy_ui::valid` (`required`, `email`, `min_len`).
- **Tables and fallible views:** `Table::keep(pred)`, `.count("items")`, `.empty("Nothing here")`,
  `.on_row(|s, row| ..)`, `.selected_where(pred)`; `attempt(result, "Cannot read folder", |v| view)`.
- `choices!` accepts labels: `enum Mode { Focus, Short = "Short break" }`.

Behaviour differences to the iced apps, not hidden: the Alloy `split` number field refuses
letters instead of showing "Enter a number" under the field; the Alloy pomodoro changes the length
of the running mode live (the same as the iced one after the `elapsed` change); the files app's
filter and hidden-file toggle behave as in iced, and its query also clears when you go up a folder.
`crates/alloy/tests/audit_apps.rs` covers all four, `tests/ergonomics.rs` the new API. iced
versions compiled in a scratch crate, not run in a window. Nothing run on macOS or Windows.

### Pass 7: shortcuts, toasts and undo (notes)

`notes.rs` gained `mod+s` (Save toast), `mod+n` (New, from `browse_of`), delete with an Undo toast
instead of a confirmation, and `mod+z` / `mod+shift+z`, in two extra lines
(`.shortcut_as("mod+s", "Save", || toast("Saved"))` and `.undoable()`), so 26 becomes 28 lines.
The iced side, `loc/iced/notes_pro.rs.txt`, is the same notes app plus a keyboard subscription,
snapshot history, a toast list with a timer subscription, and a `stack` overlay: 377 becomes 472
lines (16.9x). That iced file is **not compiled** (written from the iced 0.14 API, no fades or
slides, which Alloy's toasts have), so treat 16.9x as an estimate. The block-app ratio rises only
because the features are free in Alloy; ordinary apps do not change. Verified on Linux by headless
tests (`tests/keys.rs`); the macOS menu path is compiled only, and no native menu backend exists
yet, so on Linux shortcuts go through the shell's own matching.


## Size, cold start, million-row lists, plugins (this pass)

All Linux x86_64 in the cloud container, Xvfb with Mesa llvmpipe software Vulkan. **Not GPU numbers, and nothing here ran on macOS or Windows.** The `cargo bloat` figures are estimates by the tool.

### Binary size (counter, release, stripped)

| Build | Bytes | MB |
|---|---|---|
| Before this pass (wgpu + gles, six full bundled fonts, unwind tables) | 7,576,304 | 7.58 |
| Default build now (wgpu, Vulkan only on Linux, CPU fallback, subset fonts) | 6,414,912 | 6.41 |
| Default + `gles` feature (OpenGL backend for machines with no Vulkan) | 6,789,872 | 6.79 |
| **Slim build**: `--no-default-features --features soft-window` (no wgpu, no naga) | **4,406,976** | **4.41** |

What each step bought, measured one at a time on the counter:

| Change | Saved |
|---|---|
| Drop wgpu `gles` from the defaults (opt-in feature `gles`; machines with no Vulkan use the CPU window path instead) | ~0.3 MB |
| Subset the six bundled fonts to Latin plus the symbols Alloy draws (1.15 MB to 0.28 MB; Greek, Cyrillic and the rest come from system fonts) | 0.87 MB |
| `-C force-unwind-tables=no` (workspace `.cargo/config.toml`; nothing unwinds under `panic = "abort"`) | 0.37 MB |
| Slim build: no wgpu, naga, wgpu-hal (naga alone was 642 KiB of `.text`) | 2.0 MB |

**The default build is 6.4 MB, not under 5 MB.** The remaining weight is wgpu itself (naga 0.64 MB, wgpu-core 0.40 MB, wgpu-hal 0.17 MB of `.text`) on top of winit (0.33 MB), the text stack (skrifa, swash, rustybuzz, ttf-parser, about 0.7 MB) and std. Under 5 MB needs the slim build, which draws with Alloy's own CPU rasteriser. Other ideas, not done: a pre-translated shader to drop naga's WGSL front end (needs per-backend shader blobs, a large change), and `build-std` (nightly only).

### Cold start (`first_paint`: from entering `run` to the end of the first painted frame; median of 11, counter)

| Build / mode | Median | Range |
|---|---|---|
| Before: wgpu on llvmpipe | 86.6 ms | 78 to 99 |
| Now: wgpu on llvmpipe, with prewarm threads | 71.5 ms | 60 to 86 |
| Now: `ALLOY_RENDERER=cpu` (CPU window path in the default binary) | 26.4 ms | 20 to 33 |
| Now: slim build | 25.7 ms | 22 to 31 |

Where the 79 ms of the old GPU path went (`ALLOY_PERF=1` prints these as `alloy-perf phase ...`): loading the Vulkan runtime and creating the wgpu instance 21 ms, device creation 16 ms, text system (font scan) 6 ms, pipeline creation 6 ms, first frame render and present 19 ms, window creation and the rest about 10 ms. The change: `alloy_ui_render::prewarm()` starts the instance and the text system on two background threads while winit creates the window, which took about 15 ms off. The remaining GPU time (device, pipeline, first draw) cannot overlap with the window and, on llvmpipe, includes JIT-compiling the shader on first use, which a real GPU driver does differently. **Under 50 ms is met by the CPU window path (about 26 ms) and not by the GPU path in this container (about 70 ms); a real GPU was not available to measure.** The shell's timer excludes process start (about 5 ms for the 4 MB binary).

The CPU path is not just a start-up trick, but it is not free either, so the default stays on the GPU:

| Example (software GL vs `ALLOY_RENDERER=cpu`) | frames/s, pointer moving | resident memory |
|---|---|---|
| dashboard, llvmpipe | 88 | 127 MB |
| dashboard, CPU window | 49 | 20 MB |
| text_editor, llvmpipe | 35 | 131 MB |
| text_editor, CPU window | 8 in the default binary, 47 in the slim build (run to run variation; the slim run used the same rasteriser) | 18 to 21 MB |

Memory is the clear win (6 to 10x less, no Vulkan loader or llvmpipe); big, busy windows draw slower on the CPU than llvmpipe's multi-threaded renderer. That is why there is no automatic switch to the CPU path when the adapter is a software one.

Slim build, all examples, first paint and idle: counter 25 ms, todo 32, settings 29, dashboard 42, text_editor 49; idle CPU 0.00% except the ticking dashboard (1.40%, the same as the GPU build).

### Lists of a million rows

`vlist` / `VirtualList` already built only the visible rows (fixed row height, cost follows the window; see Pass 6 above for the 100k figures). It had a precision bug beyond about 600k rows of 28 px: scroll offsets and content height were `f32`, which stops resolving single pixels past 16.7 million pixels, so rows would shift by up to a row and the last row could be unreachable. Offsets, content height and the row maths are now `f64` (`Ui::scroll_offset_f64`). `virtual_list_ten_million_rows_stay_pixel_exact` checks 10 million rows (280 million pixels): End reaches the exact last pixel, only a screenful of rows is built, a click at the bottom edge hits row 9,999,999, and a 3 px wheel step moves exactly 3 px. Variable row heights are not supported (fixed height only), and the plain `list` and `table` widgets still build every row.

### Plugins

`docs/PLUGINS.md`, `alloy_ui::plugin` (`Plugin`, `install`, `installed`) and `alloy_ui::ext` (focus, press, state, a11y, colour helpers). Worked example: `examples/plugin-rating` (238 lines for a keyboard-, mouse- and screen-reader-aware star rating plus its translations; 5 tests and a doc-test, all passing headless).
