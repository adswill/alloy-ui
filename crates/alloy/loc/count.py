#!/usr/bin/env python3
"""Count code lines and tokens: iced 0.14 reference apps vs the Alloy examples.

Run from the repository root: python3 crates/alloy/loc/count.py
Lines are non-blank, non-comment lines after `cargo fmt` (default rustfmt).
"""
import re, pathlib

root = pathlib.Path(__file__).parent
iced = root / "iced"
alloy = root.parent / "examples"
tok = re.compile(r'"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|\d[\d_.]*|::|->|=>|\.\.=|[^\sA-Za-z0-9_]')


def code(t):
    return "\n".join(l for l in t.splitlines() if l.strip() and not l.strip().startswith("//"))


def row(name, a, b):
    ia, ib = code(a), code(b)
    li, la = len(ia.splitlines()), len(ib.splitlines())
    ti, ta = len(tok.findall(ia)), len(tok.findall(ib))
    return name, li, la, ti, ta


pairs = [("counter", "counter"), ("todo", "todo"), ("text_editor", "editor"),
         ("settings", "settings"), ("dashboard", "dashboard")]
print("| App | iced lines | Alloy lines | ratio | iced tokens | Alloy tokens | ratio |")
print("|---|---|---|---|---|---|---|")
tot = [0, 0, 0, 0]
for i, a in pairs:
    n, li, la, ti, ta = row(i, (iced / f"{i}.rs.txt").read_text(), (alloy / f"{a}.rs").read_text())
    print(f"| {n} | {li} | {la} | {li/la:.1f}x | {ti} | {ta} | {ti/ta:.1f}x |")
    tot = [x + y for x, y in zip(tot, (li, la, ti, ta))]
print(f"| **total** | {tot[0]} | {tot[1]} | {tot[0]/tot[1]:.1f}x | {tot[2]} | {tot[3]} | {tot[2]/tot[3]:.1f}x |")
