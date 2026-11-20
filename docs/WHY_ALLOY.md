# Why Alloy

Why would someone pick Alloy over iced, Slint, egui, Dioxus, GPUI or Tauri? This page is the pitch, and it is also the honest gap list. Every Alloy status below is one of **Have**, **Partial** or **Missing**, judged from the repo on `claude/project-thread-6n01vk` on 2026-11-20. Nothing here claims Windows or Linux behaviour that CI has not run.

## How the evidence was gathered

Rival complaints come from the pages linked inline. Two kinds of claim appear:

- **Verified**: the quote was read in the source page (the 2025 boringcactus survey, the iced book FAQ, the Slint FAQ).
- **Unverified**: the claim only came from a summary or from memory, and is marked *(unverified)*. Do not use these in marketing until someone reads the source.

Dates matter. The boringcactus survey is from April 2025, and the iced FAQ says IME support arrived in iced 0.14, so some iced complaints may now be stale.

## Comparison table

Rival cells are based on the sources linked in the differentiator sections. A dash means this page did not verify it.

| | Alloy | iced | Slint | egui | Dioxus (desktop) | GPUI | Tauri |
|---|---|---|---|---|---|---|---|
| Elm-style State/Message/update/view | Have | Yes | No (own DSL and bindings) | No (immediate mode) | No (React-like) | No | No (web frontend) |
| Own GPU renderer, no webview | Have | Yes | Yes | Yes | No (webview) | Yes | No (webview) |
| Lines of code for a typical app | About 1.6x fewer on ordinary apps; 9x to 14x on table/form/list-shaped apps (see BENCHMARKS.md, audit) | baseline | separate `.slint` markup plus Rust glue | small | small | large (text input alone is 700+ lines) | frontend plus Rust |
| Basic text input widget | Have (`text_input`, `text_edit`, `text_editor`) | Yes | Yes | Yes | via DOM | No, you write it | via DOM |
| Accessibility tree | Missing | Not visible to Narrator in 2025 test | - | Janky in 2025 test | Partial in 2025 test | Not visible in 2025 test | Browser-provided |
| IME | Partial (unverified on every OS) | Added in 0.14 | - | Broke kanji selection in 2025 test | Works | Works | Browser-provided |
| Licence | MIT/Apache (to confirm in Cargo.toml) | MIT | GPLv3, royalty-free with attribution, or paid | MIT/Apache | MIT/Apache | Apache/GPL parts | MIT/Apache |
| Tiny binary, low idle CPU | Have, measured on Linux only | - | - | - | No (webview) | - | Good |
| Hot reload | Missing | Missing | Live preview exists *(unverified)* | - | Yes *(unverified)* | - | Via the web dev server |
| Multi-window | Unknown, needs a check | Yes (daemon API) | - | - | - | - | Yes |

## Top differentiators

### 1. Same app in a tenth of the lines (headline)
**Status: Partial.** The `alloy` facade and the widget builders exist, and the "Make the Alloy API far more concise" thread is working on it. No honest line-count comparison exists yet.

**Pain point.** iced apps need explicit message enums, per-widget builder chains and a lot of ceremony, and GPUI needs more than 700 lines for a basic text input example ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). The iced book describes its own docs as not catering "to a wide audience" ([FAQ](https://book.iced.rs/faq.html)).

**What wins users.** A checked-in `comparison/` directory with the same three apps in Alloy, iced and Tauri, line counts produced by a script, and a rule that the README only quotes numbers that script printed. Target is 50 lines where iced needs 500. Counting must be honest: same formatting, no golfed code, comments and blank lines excluded for both sides.

### 2. It looks finished out of the box
**Status: Partial.** `DESIGN.md` sets the rules (type hierarchy, one accent, 1px borders, ease-out transitions) and the theme crate exists. Text weight and line height are accepted by the renderer but not honoured yet (DECISIONS.md #10), which blocks real type hierarchy.

**Pain point.** GTK is called out as "very GNOMEy" in the survey, and egui's default font lacks hiragana and kanji ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). Default looks are a recurring reason people reach for a webview.

**What wins users.** Real font weights and metrics, bundled fonts with CJK fallback, golden-image tests that double as a screenshot gallery in the README.

### 3. A text input that just works, everywhere
**Status: Partial.** Widgets exist (`text_input`, `text_edit`, `text_editor`), but selection, IME composition and per-OS behaviour have not been verified on Windows or macOS CI.

**Pain point.** The survey tried IME in a long list of Rust toolkits: Floem and Freya showed no composition state, egui ate the Tab key during kanji selection, and iced's IME "won't even switch into active mode" in the tested version ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). iced's FAQ concedes "text editing likely has a bunch of issues still" ([FAQ](https://book.iced.rs/faq.html)).

**What wins users.** An IME and text-editing conformance suite (Japanese, Chinese, emoji, RTL, undo, selection) that runs in CI and is published as a table.

### 4. Accessibility from day one
**Status: Missing.**

**Pain point.** In the same survey, Windows Narrator could not see into iced, GPUI, Floem, Cushy or Freya windows, and egui was "janky". The author notes egui proves "it's possible to render your UI yourself and still have real accessibility support" ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). A 2022 article treats accessibility via AccessKit as an early requirement *(unverified: summary only)* ([engineered.at](https://engineered.at/articles/advice-for-the-next-dozen-rust-guis)).

**What wins users.** Widgets emit an accessibility node alongside draw commands, wired to the platform layer (AccessKit is on the allowed-dependency spirit, but needs a DECISIONS entry since the brief lists platform bindings only). Being the one native Rust toolkit where Narrator and VoiceOver work is a rare, nameable claim.

### 5. No licence traps
**Status: Have, if the repo licence is MIT/Apache** (add a LICENSE file and state it in the README).

**Pain point.** Slint is offered as GPLv3, Royalty-free (with an attribution obligation, and "You are not permitted to use Slint within Embedded Systems"), or commercial ([FAQ](https://raw.githubusercontent.com/slint-ui/slint/HEAD/FAQ.md)). That is workable but needs a legal read for closed-source products.

**What wins users.** A one-line permissive licence, no attribution badge, no embedded carve-out.

### 6. Tauri-style shell with no webview
**Status: Have (basic), Partial (hardening in progress).** The platform crate has windows, menus, tray, dialogs, shortcuts, fonts and system theme modules. The "Harden the platform shell" thread is open.

**Pain point.** Dioxus desktop uses WebView2 and WebKitGTK; the survey author is "skeptical that Diet Electron is really the future" ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). Tauri keeps a host/frontend split with a string-typed IPC boundary *(unverified: summary only)*.

**What wins users.** Typed commands and a capability file (`alloy-ui-ipc`), 5.7 MB counter binary, 0.00% idle CPU and ~25 ms spawn-to-first-frame (all measured on Linux under Xvfb in [BENCHMARKS.md](../BENCHMARKS.md), not yet on macOS or Windows, not on a real GPU).

### 7. Idle means idle, and fast means measured
**Status: Partial.** Redraw only on change, ticks only while something animates (DECISIONS.md #9). No fps counter yet, no GPU or macOS/Windows numbers.

**What wins users.** A published benchmark table per OS against iced, with the harness in the repo so people can rerun it. Add a frame counter and a ProMotion measurement before claiming 120 fps.

### 8. API that stays put
**Status: Missing (the project is pre-1.0).**

**Pain point.** The iced FAQ answers "Why do you change the entire API on every release?" with "Because I am a terrible engineer and I want you all to suffer with me", and states "iced is a one man project" with PR review taking "months" ([FAQ](https://book.iced.rs/faq.html)). Tongue in cheek, but it names a real cost.

**What wins users.** A stability policy (semver, deprecation window, migration notes in every release), a changelog, and a bus factor above one.

### 9. Batteries included, so you do not write the widget
**Status: Partial.** Present today: button, checkbox, radio, slider, toggle, progress, pick_list, segmented, scrollable, list, table, tooltip, image, text widgets, flex layout.

**Pain point.** GPUI "doesn't have a basic text input widget"; Floem containers are limited to 16 children at once; Maycoon has no text input *(unverified)* ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)).

**What wins users.** A widget gallery covering what real apps need: tabs, tree view, menus and context menus, dialogs, date and colour pickers, virtualised lists, split panes, command palette, toasts, drag and drop.

### 10. Docs that work on the first copy-paste
**Status: Partial.** Rustdoc is required by the brief; examples exist with tests.

**Pain point.** The survey reports Azul's hello world "doesn't actually work if I copy and paste it" and GPUI's "documentation is spotty, the installation is janky" ([survey](https://www.boringcactus.com/2025/04/13/2025-survey-of-rust-gui-libraries.html)). The iced book says its docs are not there to cater to a wide audience ([FAQ](https://book.iced.rs/faq.html)).

**What wins users.** A README whose first snippet is a compiled doctest, a 10-minute tutorial, and `alloy new` scaffolding via `alloy-ui-cli`.

### 11. Hot reload and a fast dev loop
**Status: Missing.**

**Pain point.** Not verified from sources read for this page. A Rust forum thread on slow dev rebuilds exists ([users.rust-lang.org](https://users.rust-lang.org/t/rust-development-builds-became-unbearably-slow-then-i-found-a-way-to-get-1-second-rebuilds/142042)) but is about builds in general. Treat hot reload as a bet, not a proven pain point.

**What wins users.** `alloy dev` that rebuilds and relaunches with state restored, plus a headless snapshot mode already used by the examples.

### 12. Multi-window done simply
**Status: Unknown, check `crates/alloy` and the shell.**

**Pain point.** iced has a multi-window path via its daemon API ([iced source](https://docs.rs/crate/iced/0.14.0/source/src/daemon.rs)), and users ask on its forum how to build it ([discourse](https://discourse.iced.rs/t/creating-an-application-with-multiple-windows/1184)). Not verified as a top complaint.

**What wins users.** One `window()` call per window, shared state, per-window view, tested in CI.

## Prioritised roadmap

Ranked by how much each item moves the headline claim (far less code than iced, a great look, fast) and how cheap it is.

| # | Item | Status | Why this rank |
|---|---|---|---|
| 1 | `comparison/` benchmark: same 3 apps in Alloy, iced, Tauri, with an honest LoC script feeding README and BENCHMARKS.md | Partial (the 5 AM demo thread starts it) | It is the headline. Without a number nobody switches. |
| 2 | API conciseness pass: view macros or builders, implicit message routing, derive-free state, defaults for theme and window | Partial (open thread) | The thing the headline depends on. |
| 3 | Honour font weight and line height; bundle UI and mono fonts with CJK fallback | Partial | Looks are the second pitch, and type hierarchy is blocked on it. |
| 4 | Text input and IME conformance suite on all three OSes in CI | Partial | The most repeated complaint across every rival. |
| 5 | Accessibility tree (AccessKit or own) wired through widgets and the platform layer | Missing | Rare and nameable. Cheap to do early, painful to retrofit. |
| 6 | Real numbers: macOS, Windows, real GPU, fps counter, 120 Hz ProMotion; compare to iced | Partial | Turns "optimised" from a claim into evidence. |
| 7 | Widget gallery: tabs, tree, menus, dialogs, pickers, virtual list, split pane, command palette, toasts | Partial (open thread) | Removes "I'd have to write X myself". |
| 8 | `alloy new` and `alloy dev` (relaunch with state restore), plus signed bundles (CLI v2) | Partial | First five minutes and dev loop decide adoption. |
| 9 | Multi-window, drag and drop, system theme live updates, verified per OS | Unknown | Table stakes for real apps. |
| 10 | Docs: doctested README, tutorial, stability and licence policy, changelog | Partial | Cheap, and directly attacks iced's stated weakness. |

## What this page cannot claim yet

- Alloy is smaller than iced for the same app: not measured.
- Alloy is faster than iced: not measured; there is no iced run in BENCHMARKS.md.
- Anything on Windows or macOS beyond compile and CI results: unverified here.
- Several rival points above rest on a single 2025 survey and a few FAQ pages. Hot reload, binary size and multi-window complaints still need primary sources (GitHub issues, Reddit, HN) before they go in a README.
