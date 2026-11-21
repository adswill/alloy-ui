#!/usr/bin/env python3
"""Compile every Rust snippet in this extension against the real Alloy crate.

Each snippet is expanded the way VS Code expands it (placeholders replaced by their defaults),
dropped into a temporary example of the `alloy-ui` crate and built with `cargo check`.
Whole-program snippets compile as they are; fragments (a `col!`, an `input`, a `table`...) are
wrapped in a small view over a state with the fields the snippets mention.

The `alloy-live` snippet needs `live!` (not in this tree yet), so it is only expanded, not compiled.

Run from anywhere:  python3 editors/vscode/test/check_snippets.py
"""
import json, pathlib, re, subprocess, sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parents[2]
EXAMPLES = REPO / "crates/alloy/examples"

snips = json.loads((HERE.parent / "snippets/rust.code-snippets").read_text())
kinds = json.loads((HERE / "snippet-kinds.json").read_text())


def expand(body):
    text = "\n".join(body).replace("\t", "    ")
    for _ in range(3):
        text = re.sub(r"\$\{\d+:([^}]*)\}", r"\1", text)
    return re.sub(r"\$\d+", "", text).replace("\\}", "}")


PRELUDE = """#![allow(warnings)]
use alloy_ui::{prelude::*, valid};
#[derive(Default)] struct Row { name: String, count: u32 }
#[derive(Default)] struct S { name: String, done: bool, volume: f32, rows: Vec<Row> }
"""


def wrap(name, code, kind):
    if kind == "program":
        extra = "" if "fn main" in code else "\nfn main() {}\n"  # e.g. the #[test] snippet
        return "#![allow(warnings)]\n" + code + extra
    if code.lstrip().startswith("let ") or "toast(" in code:
        return PRELUDE + f"fn f(_s: &S) {{\n{code}\n}}\nfn main() {{}}\n"
    return PRELUDE + f"fn view(s: &S) -> impl View<Act<S>> {{ col![\n{code}\n] }}\nfn main() {{ let _ = sketch(S::default(), view); }}\n"


files, skipped = [], []
try:
    for i, (name, spec) in enumerate(snips.items()):
        kind = kinds[name]["kind"]
        if kind == "live":
            skipped.append(name)
            continue
        code = expand(spec["body"])
        f = EXAMPLES / f"_snippet_{i}.rs"
        f.write_text(wrap(name, code, kind))
        files.append((name, f))
    r = subprocess.run(["cargo", "check", "-p", "alloy-ui", "--examples", "--quiet"], cwd=REPO, capture_output=True, text=True)
    if r.returncode:
        sys.stderr.write(r.stderr[-6000:])
        print(f"snippets: FAILED ({len(files)} compiled)")
        sys.exit(1)
    print(f"snippets: {len(files)} compiled against alloy-ui, {len(skipped)} expanded only ({', '.join(skipped)})")
finally:
    for _, f in files:
        f.unlink(missing_ok=True)
