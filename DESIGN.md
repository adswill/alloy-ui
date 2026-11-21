# Visual design rules

Alloy's default theme must not look AI-generated. Rules distilled from Impeccable's published slop catalogue (impeccable.style/slop). Only the public pages were read; its skill files were not reachable, so treat this as a start.

## Avoid
- Inter/Geist everywhere or one family for all text; no type hierarchy.
- Purple-blue gradients, neon glow, gradient text, halos behind content, cream/beige-by-default palettes, grey text on coloured fills.
- Thick coloured side stripes on cards; hairline border plus wide soft shadow together; glassmorphism; cards nested in cards; identical icon/heading/text grids; icon tiles above every heading.
- Hero-metric layouts (huge number, tiny label); modals for multi-column settings.
- Bouncy or elastic easing, pulsing status dots, decorative motion.
- Equal spacing everywhere.

## Do
- Derive palette, type and layout from what the app is for. Use one accent, used sparingly.
- Build hierarchy with size, weight and spacing before adding a second font.
- Group with space (tight within, generous between); flatten nesting instead of adding containers.
- Body text about 14-16px with line height about 1.5; WCAG AA contrast (4.5:1 body, 3:1 large).
- Small, restrained radii; shadows only to express real elevation.
- Motion only for meaningful state change, short and eased out, never bouncy.
- Golden-image tests double as the visual review surface.

## Direction from the project owner (2026-11-20)
Alloy should look like iced and Tauri apps merged, and be genuinely polished and distinctive: iced's honest native-widget clarity with the finish, calm density and typography of a well-made Tauri/web-quality desktop app. Slint is only a loose reference (the owner does not like it much). Quality bar is "very good" at first look: the demos are judged on how they look.

Concretely: confident type hierarchy, generous but not uniform spacing, one restrained accent, crisp 1px borders over shadows, subtle state transitions (hover, press, focus) with ease-out timing, dense-but-calm tool layouts for editor and dashboard, and real content in demos (no lorem ipsum, no placeholder stats).

## Tokens and state language (visual pass, 2026-11-20)
Review surface: `cargo run -p alloy-ui-examples --bin gallery -- --headless gallery.png [--dark]` renders every widget in rest, hover, pressed, focus, disabled and error, using `Forced` to pin states. `docs/gallery_*.png` are the committed renders.

- **Neutrals**: three outline tones with distinct jobs. `border` for panels and dividers, `border_control` (new) for controls that already carry a label and fill (buttons, segmented track, switch track), `border_strong` (3:1) only where the boundary is the affordance (inputs on hover, checkbox, radio).
- **Hover and pressed are overlays** (`hover` 6%, `pressed` 12% of the text colour; 7%/13% of near-white in dark), not new greys, so they work on any surface and ghost buttons are no longer invisible on hover.
- **Focus ring**: 2px, offset by a 1px gap (3px outside the control). It reads as a halo, never merges with the control's border.
- **Error**: `TextInput::invalid(true)` turns the border and focus ring to `danger`.
- **Segmented control**: recessed track, selected segment is a raised white chip with `elevation.low`; no dark outline.
- **Switch**: off track is a quiet neutral with a white knob; on is the accent. Same colours in both themes' roles.
- Elevation, radii (2/4/6) and motion (90/150/240 ms ease-out) are unchanged.

## Navigation, tables and dates (visual pass 2, 2026-11-20)
Renders: `docs/gallery/pass1_light.png` (before) and `pass2_light.png`/`pass2_dark.png` (after). The gallery gained Navigation and date-picker sections (picker shown open).

- **Nav rows**: `Variant::Nav` on `Button` (facade: `button(..).nav(current)`). Left-aligned in the cell's 12px inset, no outline, hover overlay, selected = `selection` tint plus semibold label. No side stripe. `nav` and `shell` use it, and a 1px hairline separates sidebar from page.
- **Tables**: `List::ruled`. Rows get a hairline (inset like the cells, hidden under the selected row), taller padding and an overlay hover, instead of a surface-coloured hover that vanished on the page background. Header reserves arrow space only on the sorted column, so right-aligned numbers line up under their headers.
- **Date picker**: hover and selected chips use the shared overlay and radius; today is accent semibold text (no ring); a hairline separates month header from weekday labels.
- Not done: toast styling (another thread owns toasts in `crates/alloy`), board and browse layouts untouched beyond the nav change.

## Composite blocks and the command palette (visual pass 3, 2026-11-20)
Found by rendering the admin, status, notes and wizard apps and critiquing them cold (before/after in `gallery/pass3_*.png`).

- **Status is a dot plus a word**, never colour alone. `Up`/`Active`/`Done` are green, `Degraded`/`Pending`/`Idle` amber, `Down`/`Failed`/`Offline` red, anything else stays plain text. Enum columns in `table_of` get it automatically, `Table::status()` opts a column in, and `tally` cards carry the same dot. There is no warning role in the palette yet, so amber lives in `crates/alloy/src/tone.rs`.
- **Stat cards**: label first (with its dot), number second at heading size; no hero metric.
- **Wizard header** is a stepper: numbered circles joined by hairlines, accent for done and current, outlined and dim for later steps.
- **Tables** get `spacing.lg` between columns, so right-aligned numbers no longer touch the next column.
- **Browse detail**: checkboxes (Pinned) move into the footer beside the count and Delete instead of floating under the editor; both panes use 16 px padding so their tops line up.
- **Placeholder text** is 70% of secondary text so a hint is never mistaken for a value.
- **Command palette** (`mod+k`): a 560 px panel 14% from the top over a light 14% scrim, raised surface, 1 px border, `elevation.high`. One search row, up to eight results with the selected row on the selection tint, shortcut keys right-aligned and dim (`Ctrl+S`, or `⌘S` on macOS). No icons, no animation, no category headers. It lists every shortcut, every `.command(label, f)`, and for `app()` apps "Go to <page>" and the theme switch. Arrow keys, Enter, Escape and mouse all work; it swallows other input while open.

## Motion (visual pass 4, 2026-11-21)
Renders: `gallery/motion_rest.png`, `motion_midflight.png` (50 ms into a transition), `motion_after.png`, `motion_dark.png`; demo is `crates/alloy/examples/motion.rs` (`--headless x.png --click Details --advance 50`).

- Meaningful change only: details reveal, switch slide, value follow, press, focus. Nothing loops and nothing pulses.
- `reveal(show, view)` fade, 6 px rise and height collapse; `fade`; `animated(x)` eased value; `spring(x)` critically damped spring. All respect reduced motion.
- Button: press fades in 45 ms and out 120 ms, label presses 1 px, focus ring fades in. Slider thumb swells on a spring.
- Depth: `.elevated()` (medium shadow, with a real 1 px border) for the one card that is above the page; `.gradient(..)` only as a tonal shift between two stops of the accent or surface, never hue-shifting.
## Rich widgets and theming (2026-11-21)
Renders: `gallery/rich/showcase_*.png` (light, dark with an indigo accent, high contrast); golden tests `rich_light` and `rich_dark_accent`.

- **Badges** are pills (`badge("Active").success().dot()`): tinted fill with tone-coloured text by default, `.solid()` for the one thing that must stand out, `.outline()` for hairline. The label always carries the meaning.
- **Charts** (`line_chart`, `bar_chart`) get a real y axis with nice ticks, hairline gridlines (only the base line is strong), thinned x labels, a legend for more than one series, and a hover readout on the overlay layer. Series colours are the accent first, then hues kept apart from success, danger and warning. `sparkline` is the axis-free version for cells and stat cards.
- **Swatches** carry a hairline so near-white and near-black stay visible; the selected one gets a ring plus a tick, never colour alone.
- **Accent theming** nudges the brand colour toward black or white until it passes AA on all three surfaces, and re-derives hover, pressed, on-accent, focus ring and selection; tests run six awkward brand colours through both themes.
- **High contrast** is pure black/white, 7:1 text and borders, and a yellow or blue focus ring. **Density** scales the spacing tokens to 80% or 125%.

## Workbench widgets (2026-11-21)
Renders: `gallery/pro/workbench*.png` (`cargo run -p alloy-ui --example workbench -- --headless out.png [--rclick Text] [--click Text] [--key Name]`); golden tests `pro_*` in `crates/widgets/tests`.

- **Split pane**: a 9 px gutter with a 1 px hairline and a short grip. At rest it is as quiet as a divider; hover, drag or focus turns the line 2 px accent. The position is a ratio kept in `Ui`, so it survives window resizes; panes keep a minimum size. Arrow keys, Home and End resize when it has focus.
- **Context menu**: same surface as the pick-list popup (raised, 1 px border, medium elevation), rows on the selection tint, shortcut hints dim and right-aligned, danger in the danger colour, no icons. Opens at the pointer, flips and slides to stay inside the window. Right click, the menu key, or Shift+F10 over the target; Up/Down/Home/End/Enter/Escape inside.
- **Tabs**: label weight and a 2 px accent underline mark the selected tab; no boxed tab shapes. Tabs that do not fit collapse into a `+N` menu while the selected tab stays visible. Optional close buttons appear on the selected and hovered tab; middle click also closes.
- **Popover**: any element in a floating panel, below the anchor (above when there is no room). Controlled by `open` and `on_toggle`.
- **Markdown**: headings carry hierarchy with size and weight (a hairline under the title only), inline code on a surface chip, links in the accent with an underline, quotes as indented secondary text with a neutral hairline (no coloured stripe). Unequal block spacing: tight between list items, generous before headings. No italic face is bundled yet, so emphasis is a medium weight.
- **Virtual list**: fixed row height, only visible rows are built per frame (tested with 100 000), draggable scrollbar thumb, PageUp/PageDown/Home/End.
- Known limit: a click outside an open menu or popover that lands on a sibling control still reaches that control, as with the pick list; a modal capture layer in `Root` would fix all three.

## Presets, icons, italics and scrolling (2026-11-21)
Renders: `gallery/themes/theme_editor_<preset>_<light|dark>.png` from `cargo run -p alloy-ui --example theme_editor -- --headless out.png` (env `EDITOR_PRESET=0..5`, `EDITOR_DARK=1`, `EDITOR_PHOSPHOR=1`); goldens `preset_*` in `crates/widgets/tests/golden`.

- **Presets** (`Preset::{Graphite, Ember, Signal, Flux, Glacier}`, plus the stock `Alloy`; `Look::preset(..)`, `Theme::preset(p, appearance)`): one accent each, neutrals tinted toward the accent's family at 4-12% saturation (never beige), and distinct radii (Signal near-square, Flux roundest). Brand colours go through `with_accent`, so all ten themes pass the same AA checks as the stock theme (unit tests). Composes with accent, high contrast and density.
- **Icons**: `icon(Icon::Search)`. 91 icons from Lucide (ISC) and Phosphor Regular (MIT), bundled as two subset fonts (53 KB total) built by `tools/icons/gen.py`. They are glyphs, so they take the text colour and antialias like text on every renderer. One pack per window (`set_icon_pack`), 16 px default, no icon tiles above headings.
- **Italics**: Alloy Sans Italic and SemiBold Italic, Alloy Mono Italic are bundled. `text(..).italic()`, and Markdown emphasis uses them instead of a medium weight. Fonts are chosen by `FontFamily::{SansItalic, MonoItalic}`.
- **Scrolling**: a scroll offset is a target, and the drawn offset follows it on a critically damped spring (about 150 ms, no overshoot, jumps under reduced motion). Wheel, paging keys, track clicks, caret-follow and jumps all glide, and a burst of wheel notches keeps its velocity across retargets. `Scrollable` gained a draggable thumb (tracks the pointer with no lag), track-click paging and PageUp/PageDown/Home/End under the pointer. `VirtualList` and `TextEditor` use the same spring (`Ui::scroll_shown/scroll_wheel/scroll_to/scroll_jump`).
- Not done: OS-level inertial scrolling needs the platform's gesture phase (the event contract has none), so trackpad momentum is whatever the OS already sends; overscroll bounce is deliberately absent (DESIGN bans bounce).
