# Alloy vs iced vs Tauri: three apps from one brief

The same three apps, built from [BRIEF.md](BRIEF.md) in Alloy, iced 0.14 and Tauri 2 (HTML, CSS and vanilla JS, no framework, no npm). Nothing in the Alloy crates depends on this directory; iced and Tauri are dependencies only here. Each framework has its own cargo workspace (`alloy/`, `iced/`, `tauri/`).

Screenshots (real release binaries, Linux, Xvfb, software rendering): [screenshots/](screenshots/), also copied to the project files under `comparison/`.

## Lines of code (`python3 comparison/count.py`)

Rust is counted after default rustfmt, without blanks or comments. HTML, CSS, JS and JSON are counted as written, without blanks or comments.

| App | Alloy | iced | Tauri (Rust + HTML/CSS/JS) | iced / Alloy | Tauri / Alloy |
|---|---|---|---|---|---|
| notes | 134 | 294 | 257 (8 + 241 + 8 conf) | 2.2x | 1.9x |
| dashboard | 96 | 364 | 210 (8 + 194 + 8 conf) | 3.8x | 2.2x |
| settings | 125 | 365 | 298 (8 + 282 + 8 conf) | 2.9x | 2.4x |
| iced shared styling (ui.rs, used by all three) | | 464 | | | |
| **total** | 355 | 1487 | 765 | 4.2x | 2.2x |

The iced apps share one 464-line styling file (colours, button, input, slider and chip styles). It is counted once, in the total. The Tauri column includes the tiny Rust shell and `tauri.conf.json`.

## Binary, first paint, idle (`python3 comparison/measure.py`)

Linux, Xvfb, Mesa llvmpipe (software GL), 4 vCPU cloud container, release builds with the same profile (opt-level z, fat LTO, strip, panic abort). Three runs each. First paint is the time until the window centre stops being black, polled with ImageMagick, so it has roughly 50 ms of resolution. Idle CPU is measured over 5 s after a 3 s settle. One Tauri run never painted within 20 s.

| Binary | Size MB | First paint ms (median; runs) | Idle CPU % | RSS MB |
|---|---|---|---|---|
| alloy/notes | 7.65 | 180 (180, 161, 198) | 0.0 | 164 |
| alloy/dashboard | 7.56 | 163 (173, 163, 140) | 0.0 | 153 |
| alloy/settings | 7.6 | 160 (186, 160, 156) | 0.0 | 153 |
| iced/notes | 8.13 | 191 (219, 191, 175) | 0.0 | 138 |
| iced/dashboard | 7.97 | 167 (174, 165, 167) | 0.0 | 135 |
| iced/settings | 8.02 | 162 (202, 160, 162) | 0.0 | 135 |
| tauri/tauri-notes | 3.19 | 740 (748, never painted, 732) | 0.0 | 338 |
| tauri/tauri-dashboard | 3.19 | 710 (702, 710, 717) | 0.0 | 334 |
| tauri/tauri-settings | 3.19 | 694 (870, 694, 276) | 0.0 | 333 |

## What this shows, and what it does not

* **Lines.** Alloy is 4.2x smaller than iced in total, but only 2.2x to 3.8x per app once iced's shared styling file is spread out, and 1.9x to 2.4x smaller than the Tauri versions. That matches the earlier honest figure of about 3x for everyday apps. These apps are hand-written screens, not built from Alloy's ready-made blocks, so the 9x to 14x numbers do not apply here.
* **Polish is not equal.** The iced and Tauri apps (written by separate helper agents from the same brief) are more decorated than the Alloy ones: status pills, colour swatches, chart axis labels, a Dark toggle in every app. The Alloy versions use stock widgets (segmented control for accent instead of swatches, plain text statuses, a monospace editor font, bar chart without labels). Some of the line gap is that missing decoration, which Alloy cannot yet do in one line. Treat the look as a fair draft, not a win; Alloy currently looks plainer here.
* **Size.** Tauri's binary is 2.4x smaller, because the webview is the system's. That is real. But it uses 2x the memory (webkit helper processes) and was 4x slower to first paint in this setup.
* **Startup, CPU, memory** are software-rendered Linux numbers. Do not read them as GPU, macOS or Windows results. First paint here includes software shader compilation for the wgpu apps.
* **Not exercised.** Dark mode, interactions beyond the default screen, Windows and macOS. CI is blocked by a GitHub billing limit, so nothing here ran in CI.
* **Framework change.** To let the Preferences accent control work, Alloy gained `set_accent(color)` (and `set_dark` keeps it). The apps call it with the brief's indigo.

## Alloy at iced's line budget (`alloy-rich/`)

The lean Alloy apps above stay as they are. `alloy-rich/` rebuilds the same three apps with roughly iced's line count, spent on finish and features instead of brevity: status pills, round colour swatches, a bar and line chart with a value axis and a label under every bar (click a bar to inspect it, bars grow in on a range change), sparklines in the stat tiles, a plan-mix bar, search, sort and filter on the orders table, tag filters, sorting, duplicate and a word-goal bar in Notes, Language and Storage pages, density and reduce-motion in Preferences, empty states, toasts with undo, a Dark toggle in every app, and keyboard shortcuts everywhere (`mod+k` lists them all).

Everything custom lives in one shared kit, `alloy-rich/src/lib.rs` (pill, chip, kbd, group, setting, swatches, plot, sparkline, stack, legend, sized text), counted like iced's `ui.rs`. Nothing in the Alloy crates depends on it.

| App | Alloy lean | Alloy rich | iced |
|---|---|---|---|
| notes | 134 | 259 | 294 |
| dashboard | 96 | 290 | 364 |
| settings | 125 | 350 | 365 |
| shared kit / styling | | 431 | 464 |
| **total** | 355 | 1,330 | 1487 |

Screenshots: `screenshots/alloy_rich_*.png` (light, dark, several pages), rendered by the built-in `--headless` mode (software, 2x). Two honest caveats: headless renders do not run timers, so the switch animation and the toggle knob slide are not captured (`mod+.` finishes the chart animation for the render), and these were not run on macOS or Windows.
