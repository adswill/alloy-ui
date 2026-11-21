# Bundled fonts

IBM Plex Sans (Regular, Medium, SemiBold, Bold) and IBM Plex Mono (Regular, Medium).
Copyright (c) 2017 IBM Corp. with Reserved Font Name "Plex".
Licensed under the SIL Open Font License, Version 1.1 (https://openfontlicense.org).

Bundled so Alloy text looks identical on macOS, Linux and Windows, independent of installed system fonts.
System fonts are still loaded as fallback for scripts and emoji Plex does not cover.

## Subset

The six files are subsetted (pyftsubset) to Basic Latin, Latin-1, Latin Extended-A, general
punctuation and the symbols Alloy's own widgets draw (arrows, check mark, keyboard glyphs,
currency), with hinting removed (the rasteriser does not hint). The full `name` table is kept:
font lookup by family and weight reads it, and without it Medium and SemiBold resolve to the
wrong face. OpenType layout features are kept,
so kerning and ligatures are unchanged and Latin text renders pixel-identically. That takes the
bundle from 1.15 MB to 0.29 MB. Greek, Cyrillic, Vietnamese and everything else come from the
system fonts (fallback, as for CJK and Arabic), or from fonts the app loads itself.
