#!/usr/bin/env python3
"""Lines of code per app and framework. Rust: rustfmt (default), non-blank, non-comment.
HTML/CSS/JS/JSON: non-blank, non-comment lines as written (no minifying)."""
import re, subprocess, pathlib
root = pathlib.Path(__file__).parent

def rust(p):
    t = p.read_text()
    r = subprocess.run(["rustfmt", "--edition", "2021", "--emit", "stdout"], input=t, capture_output=True, text=True)
    if r.returncode == 0:
        t = r.stdout.split("\n", 1)[1] if r.stdout.startswith("<stdin>") else r.stdout
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    return sum(1 for l in t.splitlines() if l.strip() and not l.strip().startswith("//"))

def web(p):
    t = p.read_text()
    t = re.sub(r"/\*.*?\*/|<!--.*?-->", "", t, flags=re.S)
    return sum(1 for l in t.splitlines() if l.strip() and not l.strip().startswith("//"))

apps = ["notes", "dashboard", "settings"]
print("| App | Alloy | iced | Tauri (Rust + HTML/CSS/JS) | iced / Alloy | Tauri / Alloy |")
print("|---|---|---|---|---|---|")
ui = rust(root / "iced/src/ui.rs")
tot = [0, ui, 0]
for a in apps:
    al = rust(root / f"alloy/src/bin/{a}.rs")
    ic = rust(root / f"iced/src/bin/{a}.rs")
    t = root / f"tauri/{a}"
    parts = {"rust": rust(t / "src-tauri/src/main.rs") + rust(t / "src-tauri/build.rs"),
             "web": sum(web(f) for f in (t / "ui").iterdir() if f.suffix in (".html", ".css", ".js")),
             "conf": web(t / "src-tauri/tauri.conf.json")}
    ta = sum(parts.values())
    tot = [tot[0] + al, tot[1] + ic, tot[2] + ta]
    print(f"| {a} | {al} | {ic} | {ta} ({parts['rust']} + {parts['web']} + {parts['conf']} conf) | {ic/al:.1f}x | {ta/al:.1f}x |")
print(f"| iced shared styling (ui.rs, used by all three) | | {ui} | | | |")
print(f"| **total** | {tot[0]} | {tot[1]} | {tot[2]} | {tot[1]/tot[0]:.1f}x | {tot[2]/tot[0]:.1f}x |")
