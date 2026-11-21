#!/usr/bin/env python3
"""Build the hosted docs: turns docs/guide/*.md and docs/cookbook/*.md into static HTML.

    pip install markdown pygments
    python3 website/build.py

The generated pages are committed so GitHub Pages can serve the folder as it is.
Code blocks come straight from the doc-tested Markdown; doc-test scaffolding lines
(`# fn main() {}`) are hidden, the same way rustdoc hides them.
"""
import html
import pathlib
import re
import sys

import markdown

ROOT = pathlib.Path(__file__).resolve().parent
REPO = ROOT.parent
GH = "https://github.com/adswill/alloy-ui/blob/main/"
SECTIONS = [("guide", "Guide", REPO / "docs/guide"), ("cookbook", "Cookbook", REPO / "docs/cookbook")]

FENCE = re.compile(r"^```(\S*)\s*$")


def prepare(text: str) -> str:
    """Normalise fence info strings and hide rustdoc scaffolding lines."""
    out, in_block, lang = [], False, ""
    for line in text.splitlines():
        m = FENCE.match(line)
        if m and not in_block:
            in_block, lang = True, m.group(1).split(",")[0] or "text"
            lang = {"ignore": "rust", "no_run": "rust"}.get(lang, lang)
            out.append("```" + lang)
            continue
        if line.strip() == "```" and in_block:
            in_block = False
            out.append(line)
            continue
        if in_block and lang == "rust" and re.match(r"^\s*#( |$)", line):
            continue
        out.append(line)
    return "\n".join(out) + "\n"


def fix_links(body: str, section: str, names: dict) -> str:
    def repl(m):
        href = m.group(1)
        if href.startswith(("http", "#", "mailto:")):
            return m.group(0)
        path, _, frag = href.partition("#")
        frag = "#" + frag if frag else ""
        base = path.split("/")[-1]
        if path.endswith(".md"):
            stem = base[:-3]
            if stem == "README":
                sec = "cookbook" if "cookbook" in path else "guide" if "guide" in path or section == "guide" else section
                return f'href="../index.html{frag}"' if sec == section else f'href="../{sec}/index.html{frag}"'
            for sec, stems in names.items():
                if stem in stems:
                    return f'href="{"" if sec == section else "../" + sec + "/"}{stem}.html{frag}"'
        if path.startswith("../../") or path.startswith("../"):
            tail = path.replace("../", "")
            return f'href="{GH}{tail}{frag}"'
        return m.group(0)

    return re.sub(r'href="([^"]+)"', repl, body)


def render(md_text: str) -> str:
    md = markdown.Markdown(
        extensions=["tables", "fenced_code", "codehilite", "sane_lists"],
        extension_configs={"codehilite": {"css_class": "hl", "guess_lang": False, "noclasses": False}},
    )
    return md.convert(prepare(md_text))


def title_of(md_text: str, fallback: str) -> str:
    m = re.search(r"^#\s+(.+)$", md_text, re.M)
    return m.group(1).strip().replace("`", "") if m else fallback


def shell(title, desc, depth, side, body, pager="", current="docs"):
    up = "../" * depth
    def nav(label, href, key):
        cur = ' aria-current="page"' if key == current else ""
        return f'<a href="{up}{href}"{cur}>{label}</a>'
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{html.escape(title)} · Alloy</title>
<meta name="description" content="{html.escape(desc)}">
<link rel="icon" href="{up}assets/logo/alloy-favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="{up}style.css">
<script>try{{var t=localStorage.getItem("alloy-theme");if(t)document.documentElement.dataset.theme=t}}catch(e){{}}</script>
</head>
<body>
<header class="nav"><div class="wrap">
<a class="brand" href="{up}index.html"><img src="{up}assets/logo/alloy-mark.svg" alt=""><span>Alloy</span></a>
<nav aria-label="Main">
{nav("Guide", "docs/guide/index.html", "guide")}
{nav("Cookbook", "docs/cookbook/index.html", "cookbook")}
{nav("Editor", "docs/editor.html", "editor")}
</nav>
<span class="sp"></span>
<button class="theme-btn" id="theme" type="button" aria-label="Toggle light and dark">Theme</button>
<a class="gh" href="https://github.com/adswill/alloy-ui">GitHub</a>
</div></header>
<main class="wrap docs">
<aside class="side" aria-label="Documentation">{side}</aside>
<article class="prose">
{body}
{pager}
</article>
</main>
<footer><div class="wrap"><span>Alloy is MIT or Apache-2.0 licensed. Docs are built from the repository.</span><span><a href="{up}index.html">Home</a> · <a href="https://github.com/adswill/alloy-ui">Source</a></span></div></footer>
<script src="{up}site.js" defer></script>
</body>
</html>
"""


def main():
    pages = {}  # section -> [(stem, title, md)]
    for key, _label, folder in SECTIONS:
        items = []
        for p in sorted(folder.glob("[0-9][0-9]-*.md")):
            text = p.read_text()
            items.append((p.stem, title_of(text, p.stem), text))
        pages[key] = items
    names = {k: {s for s, _, _ in v} for k, v in pages.items()}

    def sidebar(section, stem, flat=False):
        parts = []
        for key, label, _ in SECTIONS:
            parts.append(f"<h4>{label}</h4>")
            idx_cur = ' aria-current="page"' if (key == section and stem == "index") else ""
            href = (f"{key}/" if flat else "" if key == section else f"../{key}/") + "index.html"
            parts.append(f'<a href="{href}"{idx_cur}>Overview</a>')
            for s, t, _ in pages[key]:
                cur = ' aria-current="page"' if (key == section and s == stem) else ""
                href = (f"{key}/" if flat else "" if key == section else f"../{key}/") + f"{s}.html"
                label_t = re.sub(r"^\d+\.\s*", "", t)
                parts.append(f'<a href="{href}"{cur}>{html.escape(label_t)}</a>')
        cur = ' aria-current="page"' if section == "editor" else ""
        parts.append(f'<h4>Tools</h4><a href="{"" if flat else "../"}editor.html"{cur}>VS Code extension</a>')
        return "".join(parts)

    (ROOT / "docs").mkdir(exist_ok=True)
    for key, label, folder in SECTIONS:
        out = ROOT / "docs" / key
        out.mkdir(parents=True, exist_ok=True)
        items = pages[key]
        readme = (folder / "README.md").read_text()
        # index page
        body = render(readme)
        body = fix_links(body, key, names)
        if key == "cookbook":
            cards = "".join(
                f'<a href="{s}.html"><b>{html.escape(re.sub(chr(94)+r"\d+\.\s*","",t))}</b><span>{html.escape(_blurb(md))}</span></a>'
                for s, t, md in items
            )
            body += f'<div class="cards">{cards}</div>'
        (out / "index.html").write_text(
            shell(f"{label}", f"The Alloy {label.lower()}", 2, sidebar(key, "index"), body, current=key)
        )
        for i, (s, t, md) in enumerate(items):
            body = fix_links(render(md), key, names)
            if key == "cookbook":
                n = len(re.findall(r"^## \d+\.", md, re.M))
                body = body.replace("</h1>", f'</h1>\n<p class="tests">{n} recipes, each compiled as a doc-test</p>', 1)
            prev_ = items[i - 1] if i else None
            next_ = items[i + 1] if i + 1 < len(items) else None
            pager = '<nav class="pager">'
            pager += f'<a href="{prev_[0]}.html">&larr; {html.escape(prev_[1])}</a>' if prev_ else "<span></span>"
            pager += f'<a href="{next_[0]}.html">{html.escape(next_[1])} &rarr;</a>' if next_ else "<span></span>"
            pager += "</nav>"
            (out / f"{s}.html").write_text(
                shell(t, f"{t} in the Alloy {label.lower()}", 2, sidebar(key, s), body, pager, current=key)
            )
    build_home()
    # editor page
    ed = REPO / "editors/vscode/README.md"
    if ed.exists():
        body = fix_links(render(ed.read_text()), "guide", names)
        (ROOT / "docs/editor.html").write_text(
            shell("VS Code extension", "Snippets and .alloy syntax for VS Code", 1, sidebar("editor", "none", flat=True), body, current="editor")
        )
    print("built", sum(len(v) for v in pages.values()), "pages")


def build_home():
    from pygments import highlight
    from pygments.formatters import HtmlFormatter
    from pygments.lexers import get_lexer_by_name

    def code(lang, src):
        h = highlight(src.strip("\n"), get_lexer_by_name(lang), HtmlFormatter(cssclass="hl", nowrap=True))
        return f'<pre class="hl"><code>{h}</code></pre>'

    def strip_header(src):  # drop the leading //! doc line from example files
        return "\n".join(l for l in src.splitlines() if not l.startswith("//!"))

    ex = REPO / "crates/alloy/examples"
    samples = {
        "counter": ("rust", (ex / "counter.rs").read_text().split("\n", 1)[1].replace("use alloy_ui", "use alloy_ui", 1)),
        "notes": ("rust", strip_header((ex / "notes.rs").read_text())),
        "resource": ("rust", """use alloy_ui::prelude::*;

async fn load_releases() -> Result<Vec<String>, String> {
    sleep(secs(0.4)).await; // stands in for a network call
    Ok(vec!["0.4.0".into(), "0.3.2".into()])
}

fn main() {
    sketch((), |_| {
        resource(load_releases).page("Releases", |v| {
            col(v.iter().map(|r| text(format!("alloy {r}")))).gap(6.0)
        })
    })
    .run("Releases")
}"""),
        "test": ("rust", """#[test]
fn adding_counts() {
    let mut ui = Harness::new(sketch(0i64, view), 360.0, 200.0);
    ui.click("Add").click("Add");
    assert!(ui.has_text("2 so far"));
    ui.save_png("target/counter.png"); // CPU-rendered, no GPU"""+"\n}"),
        "cli": ("console", """$ alloy new hello --identifier com.example.hello
$ cd hello && alloy dev                # validates capabilities, runs a debug build
$ alloy bundle --format deb,rpm        # pure-Rust packagers, nothing to install
$ alloy bundle --format app,dmg        # macOS bundles (built on macOS)
$ alloy bundle --format exe,msi        # Windows bundles"""),
        "start": ("console", """$ cargo install --git https://github.com/adswill/alloy-ui alloy-ui-cli
$ alloy new hello --identifier com.example.hello
$ cd hello && alloy dev"""),
    }
    tpl = (ROOT / "home.html.in").read_text()
    for name, (lang, src) in samples.items():
        tpl = tpl.replace(f"@@CODE:{name}@@", code(lang, src))
    (ROOT / "index.html").write_text(tpl)


def _blurb(md: str) -> str:
    heads = re.findall(r"^## \d+\.\s*(.+)$", md, re.M)
    return "; ".join(heads[:5])


if __name__ == "__main__":
    sys.exit(main())
