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


def fmt(t):
    """Format with default rustfmt so both sides are measured the same way."""
    import subprocess
    r = subprocess.run(["rustfmt", "--edition", "2021", "--emit", "stdout"], input=t, capture_output=True, text=True)
    out = r.stdout.split("\n", 1)[1] if r.stdout.startswith("<stdin>") else r.stdout
    return out if r.returncode == 0 else t


def code(t):
    t = re.sub(r"/\*.*?\*/", "", fmt(t), flags=re.S)
    return "\n".join(l for l in t.splitlines() if l.strip() and not l.strip().startswith("//"))


def row(name, a, b):
    ia, ib = code(a), code(b)
    li, la = len(ia.splitlines()), len(ib.splitlines())
    ti, ta = len(tok.findall(ia)), len(tok.findall(ib))
    return name, li, la, ti, ta


pairs = [("counter", "counter"), ("todo", "todo"), ("text_editor", "editor"),
         ("settings", "settings"), ("dashboard", "dashboard"), ("admin", "admin"), ("status board (async)", "status"), ("notes", "notes_plain"), ("notes + shortcuts, toasts, undo", "notes"), ("admin_blocks", "admin_manual"), ("crud-only", "crud"), ("pomodoro (new)", "pomodoro"), ("files (new)", "files"), ("wizard (new)", "wizard"), ("split (unseen)", "split")]
print("| App | iced lines | Alloy lines | ratio | iced tokens | Alloy tokens | ratio |")
print("|---|---|---|---|---|---|---|")
tot = [0, 0, 0, 0]
for i, a in pairs:
    n_label = i
    i = {"admin_blocks": "admin", "crud-only": "admin", "status board (async)": "status", "pomodoro (new)": "pomodoro", "files (new)": "files", "wizard (new)": "wizard", "split (unseen)": "split", "notes + shortcuts, toasts, undo": "notes_pro"}.get(i, i)
    n, li, la, ti, ta = row(i, (iced / f"{i}.rs.txt").read_text(), (root / "notes_plain.rs.txt" if a == "notes_plain" else alloy / f"{a}.rs").read_text())
    n = n_label
    print(f"| {n} | {li} | {la} | {li/la:.1f}x | {ti} | {ta} | {ti/ta:.1f}x |")
    if n_label not in ("admin_blocks", "crud-only", "notes + shortcuts, toasts, undo") and "(new)" not in n_label and "(unseen)" not in n_label:
        tot = [x + y for x, y in zip(tot, (li, la, ti, ta))]
print(f"| **total** | {tot[0]} | {tot[1]} | {tot[0]/tot[1]:.1f}x | {tot[2]} | {tot[3]} | {tot[2]/tot[3]:.1f}x |")
