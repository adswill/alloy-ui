//! Editors: a code editor (showing this very file), a rich-text editor next to its Markdown
//! round trip, and a Markdown editor with a live preview in a split pane.
use crate::ui::*;
use crate::Studio;
use alloy_ui::i18n;
use alloy_ui::prelude::*;

pub const NOTES: &str = "# Q4 planning\n\nThe **orders grid** now handles a hundred thousand rows without a hitch. Sort any column, drag a header edge to resize it, double-click a *Customer* to rename it.\n\n## Before launch\n\n- Freeze the public API\n- Write the changelog\n- Tag **v0.4** and publish the [release notes](https://alloy.dev/notes)\n\n1. Smoke test on three platforms\n2. Check idle CPU stays near zero\n\n> Small, sharp tools win. Ship the editor, then ship the docs.";

pub const MARKDOWN: &str = "# Release notes, v0.4\n\nAlloy now draws **every pixel itself**, and this page is rendered by Alloy's own Markdown widget as you type.\n\n## What is new\n\n- Data grid with 100 000 rows\n- Code editor with syntax colours\n- Rich-text editor, *italics* included\n  - nested lists work\n- Kanban, calendar and node graph\n\n1. Update the crate\n2. Run `alloy dev`\n3. Press F12 and look around\n\n> Edit the left pane. The right pane follows.\n\n```rust\nsketch(0, |n| button(n.to_string()).act(|n| *n += 1)).run(\"Hi\")\n```\n\nSee the [guide](https://alloy.dev/guide) for more.";

pub fn view(s: &Studio) -> N {
    let body: N = match s.editors_tab {
        0 => col![
            caption("This is the source of this very page, editable. Try Cmd/Ctrl+F is not wired; Tab indents, Cmd/Ctrl+Z undoes.").dim(),
            code_editor(at!(s.src)).file("editors.rs").lang(Lang::Rust).fill_h(),
        ]
        .gap(8.0)
        .into(),
        1 => split(
            col![
                caption("Rich text: bold, italic, lists and links. Select text and use the toolbar or the usual shortcuts.").dim(),
                rich_editor(at!(s.doc))
                    .placeholder("Start writing")
                    .link(|s: &mut Studio, url| {
                        s.opened_link = url;
                        toast("Link clicked");
                    })
                    .fill_h(),
            ]
            .gap(8.0)
            .pad_xy(0.0, 0.0),
            col![
                caption(if s.opened_link.is_empty() {
                    "The same document as Markdown".to_string()
                } else {
                    format!("Last link: {}", s.opened_link)
                })
                .dim(),
                scroll(markdown(s.doc.to_markdown()).width(420.0)).fill_h(),
            ]
            .gap(8.0)
            .pad_xy(14.0, 0.0),
        )
        .ratio(0.58)
        .into(),
        _ => split(
            col![
                caption("Markdown source").dim(),
                editor(at!(s.md)).fill_h(),
            ]
            .gap(8.0),
            col![
                caption("Preview").dim(),
                scroll(markdown(&s.md).width(460.0)).fill_h(),
            ]
            .gap(8.0)
            .pad_xy(14.0, 0.0),
        )
        .ratio(0.5)
        .into(),
    };
    fixed(col![
        header(
            i18n::translate("ed.title", &[]),
            i18n::translate("ed.sub", &[]),
            badge("Native text engine").accent().outline()
        ),
        tab_bar(["Code", "Rich text", "Markdown"], at!(s.editors_tab)),
        body.fill_h(),
    ])
}
