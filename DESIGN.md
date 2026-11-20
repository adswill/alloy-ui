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
