# Benchmarks

Only numbers that were actually measured are listed. Everything below ran on one
machine: the Linux cloud development container (shared CPU, software rendering
through Mesa llvmpipe under Xvfb). Treat it as indicative, not as a GPU result.
macOS and Windows have not been run yet; run the harness there and add a section.

## Headless harness

`cargo run --release -p alloy-ui-bench`

- **Frame build**: `update` + `view` + layout + draw-list construction through the real `Runtime`, with real shaped text metrics (cosmic-text), as in a window.
- **CPU raster**: the software reference renderer at 2x scale.
- **wgpu offscreen**: the GPU renderer path rendering offscreen at 2x including read-back. On this machine the adapter is llvmpipe, a CPU rasteriser, so this is not GPU speed.
- **Frames/s possible**: 1000 / (frame build + wgpu offscreen). An upper bound for the pipeline on this adapter, not a measured on-screen frame rate.
- **Per no-op event**: runtime cost of an input event that changes nothing. It re-runs layout, so it is dominated by text measurement.
- **Spawn to first frame**: wall time from spawning `counter --headless` to its PNG being written.
- **Binary size**: the stripped release `counter` (opt-level z, fat LTO, windowed build).

    Finished `release` profile [optimized] target(s) in 0.12s
### linux / x86_64

Offscreen wgpu adapter: llvmpipe (LLVM 20.1.2, 256 bits) (Cpu, Vulkan)

| Example | Frame build | CPU raster 2x | wgpu offscreen 2x (with read-back) | Frames/s possible (wgpu) | Per no-op event |
|---|---|---|---|---|---|
| counter | 389 us | 16.6 ms | 3.0 ms | 293 | 251489 ns |
| todo | 551 us | 26.8 ms | 8.2 ms | 114 | 444591 ns |
| text_editor | 481 us | 34.5 ms | 8.4 ms | 112 | 307722 ns |
| settings | 288 us | 33.7 ms | 8.7 ms | 111 | 272466 ns |
| dashboard | 1035 us | 47.3 ms | 11.2 ms | 82 | 811323 ns |

| Metric | Value | Target |
|---|---|---|
| counter release binary (windowed build, also runs headless) | 5.74 MB | < 10 MB |
| counter spawn to first frame written (--headless, median of 7) | 27 ms | < 200 ms |
| on-screen fps, ProMotion 120 fps | not measured (the shell has no frame counter) | 60 fps |

Finding: since the examples use real text metrics, a no-op event costs 0.25 to 0.8 ms (it was about 2 to 10 us with approximate metrics). Caching text measurements in the widget layer would remove most of that; it matters for idle CPU only if something triggers frequent re-layout.

## Windowed run (Linux, Xvfb, software GL)

`crates/bench/windowed.sh target/release counter todo text_editor settings dashboard`

- **Window mapped**: from process spawn until an X window exists. This is not time to first painted frame.
- **Idle CPU**: CPU time over 5 s after 1.5 s of settling, window untouched. The dashboard has a 1 s subscription that redraws, so it is not fully idle.
- **Resident memory** is dominated by Mesa llvmpipe and wgpu in this container; a machine with a real GPU driver will differ.

| Example | Window mapped | Idle CPU (5 s) | Resident memory |
|---|---|---|---|
| counter | 27 ms | 0.00% | 120 MB |
| todo | 33 ms | 0.00% | 120 MB |
| text_editor | 26 ms | 0.00% | 120 MB |
| settings | 34 ms | 0.00% | 120 MB |
| dashboard | 30 ms | 1.00% | 123 MB |

A screenshot of the running dashboard window under Xvfb confirmed that the native window paints and that the subscription updates the chart.

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

- On-screen frames per second (the shell has no frame counter; the 60 fps and 120 fps ProMotion targets are unverified). The headless table gives only an upper bound on llvmpipe.
- Time to first painted frame in a window.
- Anything on a real GPU, macOS or Windows.

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
