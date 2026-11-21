# Alloy website

A static site: no framework, no build step to serve it. The framework itself has no web content; this is only its marketing and documentation site.

```
website/
  index.html          home page (generated from home.html.in)
  docs/               guide, cookbook and editor pages (generated)
  assets/             logo, fonts (IBM Plex, SIL OFL), compressed screenshots
  style.css, site.js  the only CSS and JS (theme toggle, tabs, copy button)
  build.py            regenerates index.html and docs/ from the repository's Markdown
```

## Rebuild

```sh
pip install markdown pygments
python3 website/build.py
```

The docs pages are built from `docs/guide/*.md` and `docs/cookbook/*.md`, the same files `cargo test -p alloy-ui --doc` compiles, so the hosted snippets are the tested ones. The home page code samples are read from `crates/alloy/examples/`. Generated files are committed so the folder can be served as is.

## Preview

```sh
python3 -m http.server -d website 8000
```

## Publish (not done yet)

`.github/workflows/pages.yml` can deploy this folder to GitHub Pages. It runs only when started by hand (`workflow_dispatch`), and Pages must first be enabled in the repository settings (Settings, Pages, Source: GitHub Actions). Nothing is published until someone does both.
