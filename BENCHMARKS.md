# Benchmarks

Only numbers that were actually measured are listed. Everything below ran on one
machine: the Linux cloud development container (shared CPU, software rendering
through Mesa llvmpipe under Xvfb). Treat it as indicative, not as a GPU result.
macOS and Windows have not been run yet; run the harness there and add a section.

## Headless harness

`cargo run --release -p alloy-ui-bench`

- **Frame build**: `update` + `view` + layout + draw-list construction through the real `Runtime`.
- **CPU raster**: the software reference renderer at 2x scale, not the GPU path.
- **Per no-op event**: runtime cost of an input event that changes nothing.
- **Spawn to first frame**: wall time from spawning `counter --headless` to its PNG being written.
- **Binary size**: the stripped release `counter` (opt-level z, fat LTO, windowed build).

    Finished `release` profile [optimized] target(s) in 0.12s
### linux / x86_64

| Example | Frame build (median) | CPU raster 2x (median) | Per no-op event |
|---|---|---|---|
| counter | 3 us | 15.5 ms | 2432 ns |
| todo | 7 us | 26.6 ms | 5131 ns |
| text_editor | 4 us | 34.7 ms | 2574 ns |
| settings | 5 us | 33.4 ms | 3986 ns |
| dashboard | 14 us | 48.3 ms | 10638 ns |

| Metric | Value | Target |
|---|---|---|
| counter release binary (windowed build, also runs headless) | 5.73 MB | < 10 MB |
| counter spawn to first frame written (--headless, median of 7) | 25 ms | < 200 ms |
| fps, ProMotion 120 fps | not measured (no frame counter in the shell yet) | 60 fps |

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

## Not measured yet

- Frames per second (the shell has no frame counter; the 60 fps and 120 fps ProMotion targets are unverified).
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
