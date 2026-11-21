# Bundled fonts

**Alloy Sans** (Regular, Medium, SemiBold, Bold) and **Alloy Mono** (Regular, Medium).
Modified versions of IBM Plex Sans and IBM Plex Mono, Copyright 2017/2018 IBM Corp.
Licensed under the SIL Open Font License 1.1; the full text and copyright notice are in
[`OFL.txt`](OFL.txt). Source: https://github.com/IBM/plex.

IBM Plex declares "Plex" as a Reserved Font Name. The OFL forbids a Modified Version from
using it, so these subsets are renamed: family `Alloy Sans` / `Alloy Mono`, files
`AlloySans-*.ttf` / `AlloyMono-*.ttf`. Only the name table was changed (family, full, PostScript
and unique-ID names; the trademark note); the copyright and licence records are intact.
They are not endorsed by IBM.

Bundled so Alloy text looks identical on macOS, Linux and Windows, independent of installed system fonts.
System fonts are still loaded as fallback for scripts and emoji these fonts do not cover.

## Subset

The six files are subsetted (pyftsubset) to Basic Latin, Latin-1, Latin Extended-A, general
punctuation and the symbols Alloy's own widgets draw (arrows, check mark, keyboard glyphs,
currency), with hinting removed (the rasteriser does not hint). The full `name` table is kept:
font lookup by family and weight reads it, and without it Medium and SemiBold resolve to the
wrong face. OpenType layout features are kept,
so kerning and ligatures are unchanged and Latin text renders pixel-identically. That takes the
bundle from 1.15 MB to 0.29 MB. Greek, Cyrillic, Vietnamese and everything else come from the
system fonts (fallback, as for CJK and Arabic), or from fonts the app loads itself.
