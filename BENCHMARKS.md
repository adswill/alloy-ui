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
| `admin.rs` | `#[derive(Record)]`, `records()`, `shell`, `theme_switch`, `persist` | 48 | **6.6x** | **6.7x** |
| `crud.rs` | `crud(title, rows)`: whole app is one struct plus one call | 18 | 17.6x | 19x |

Read these honestly:

- The 6.6x row is the same app feature for feature. It is not 10x: 48 lines remain (the struct
  with its form attributes, the page enum and the `view` that picks a page, seed data).
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
