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

## Publish

`.github/workflows/pages.yml` rebuilds the site and pushes it to the `gh-pages` branch whenever `website/`, `docs/` or the comparison notes change on `main`. On its first run it also tries to switch Pages on for that branch. If the repository does not let the workflow do that, set it once under Settings, Pages, Source: Deploy from a branch, `gh-pages`, `/ (root)`.
