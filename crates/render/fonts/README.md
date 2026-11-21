# Bundled fonts

IBM Plex Sans (Regular, Medium, SemiBold, Bold, plus Italic and SemiBold Italic) and IBM Plex Mono (Regular, Medium, Italic).
Copyright (c) 2017 IBM Corp. with Reserved Font Name "Plex".
Licensed under the SIL Open Font License, Version 1.1 (https://openfontlicense.org).

Bundled so Alloy text looks identical on macOS, Linux and Windows, independent of installed system fonts.
System fonts are still loaded as fallback for scripts and emoji Plex does not cover.

## Icon fonts

`Lucide-subset.ttf` (ISC, see `LICENSE-lucide.txt`) and `Phosphor-subset.ttf` (Phosphor Regular, MIT, see
`LICENSE-phosphor.txt`) hold the 91 icons listed in `crates/widgets/src/icon_data.rs`. They are subsets made by
`tools/icons/gen.py`; icons are drawn as glyphs, so they take the text colour, antialias and scale like text.
