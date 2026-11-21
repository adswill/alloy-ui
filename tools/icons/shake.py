#!/usr/bin/env python3
"""Tree-shakes the full icon fonts down to the icons an app uses.

Scans Rust sources for `icon_named("...")` and `Ico::named("...")` string literals, keeps only
those glyphs (plus the built-in `Icon` set) from the full Lucide and Phosphor fonts, and writes
`Lucide.ttf` and `Phosphor.ttf` into <out-dir>. Build the app with

    ALLOY_ICONS_SUBSET=<out-dir> cargo build --release --features icons-lucide-full

and the renderer embeds those small fonts instead of the 1 MB full pack. Names that are built
at run time cannot be found by the scan: list them in <out-dir>/extra.txt, one per line.

Usage: shake.py <src-dir> <out-dir> <repo-root>      (needs `pip install fonttools`)
"""
import json, re, subprocess, sys, pathlib

src, out, root = map(pathlib.Path, sys.argv[1:4])
out.mkdir(parents=True, exist_ok=True)
names = set()
for f in src.rglob("*.rs"):
    names |= set(re.findall(r'(?:icon_named|Ico::named)\(\s*"([a-z0-9-]+)"', f.read_text()))
extra = out / "extra.txt"
if extra.exists():
    names |= {l.strip() for l in extra.read_text().split() if l.strip()}

tables = (root / "crates/widgets/src/icon_names.rs").read_text()
def parse(block):
    body = tables.split(f"pub(crate) static {block}")[1].split("];")[0]
    return {m.group(1): int(m.group(2), 16) for m in re.finditer(r'\("([a-z0-9-]+)", 0x([0-9A-F]+)\)', body)}
fonts = root / "crates/render/fonts"
# The built-in Icon set lives in the small subset fonts already; keep those codepoints too.
for pack, table, full, small in (("Lucide", "LUCIDE", "full/Lucide-full.ttf", "Lucide-subset.ttf"),
                                 ("Phosphor", "PHOSPHOR", "full/Phosphor-full.ttf", "Phosphor-subset.ttf")):
    t = parse(table)
    keep = {t[n] for n in names if n in t}
    from fontTools.ttLib import TTFont
    keep |= set(TTFont(fonts / small).getBestCmap())
    unicodes = ",".join(f"U+{c:04X}" for c in sorted(keep))
    subprocess.check_call(["pyftsubset", str(fonts / full), f"--unicodes={unicodes}",
                           f"--output-file={out / (pack + '.ttf')}", "--no-hinting"])
    print(pack, len(keep), "glyphs")
