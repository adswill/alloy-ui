//! Showcase: badges, swatches, charts with axes, sparklines, a tree, a colour picker, and
//! live theming (dark, brand accent, high contrast, density).
use alloy_ui::prelude::*;

const BRANDS: [(&str, u32); 5] = [
    ("Teal", 0x0B6B5F),
    ("Indigo", 0x4338CA),
    ("Crimson", 0xC2185B),
    ("Amber", 0xB45309),
    ("Slate", 0x475569),
];

state! {
    struct Show {
        dark: bool,
        contrast: bool,
        density: usize = 1,
        brand: usize,
        custom: Color = Color::hex(0x4338CA),
        picked: Option<u64>,
    }
}

impl Show {
    fn apply(&self) {
        let d = [Density::Compact, Density::Comfortable, Density::Spacious][self.density];
        set_look(
            Look::light()
                .is_dark(self.dark)
                .accent(if self.brand == 5 {
                    self.custom
                } else {
                    Color::hex(BRANDS[self.brand].1)
                })
                .high_contrast(self.contrast)
                .density(d),
        );
    }
}

fn files() -> Vec<TreeItem> {
    vec![
        TreeItem::new(1, "crates").children([
            TreeItem::new(2, "core")
                .children([TreeItem::new(3, "draw.rs"), TreeItem::new(4, "widget.rs")]),
            TreeItem::new(5, "widgets")
                .children([TreeItem::new(6, "plot.rs"), TreeItem::new(7, "tree.rs")]),
            TreeItem::new(8, "theme"),
        ]),
        TreeItem::new(9, "docs").children([TreeItem::new(10, "DESIGN.md")]),
        TreeItem::new(11, "Cargo.toml"),
    ]
}

fn view(s: &Show) -> impl View<Act<Show>> {
    s.apply();
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let reqs = vec![1240.0, 1810.0, 1595.0, 2230.0, 2410.0, 1380.0, 1120.0];
    let errs = vec![32.0, 51.0, 44.0, 120.0, 61.0, 29.0, 22.0];
    col![
        row![
            heading("Showcase").fill_w(),
            choose([("Light", false), ("Dark", true)], at!(s.dark)),
        ]
        .align_center(),
        // Theming controls.
        row![row(BRANDS.iter().enumerate().map(|(i, (name, hex))| {
            swatch(Color::hex(*hex))
                .named(*name)
                .selected(s.brand == i)
                .act(move |s: &mut Show| s.brand = i)
        }))
        .gap(14.0),],
        row![
            choose(
                [("Compact", 0usize), ("Comfortable", 1), ("Spacious", 2)],
                at!(s.density)
            ),
            check("High contrast", at!(s.contrast)),
        ]
        .gap(16.0)
        .align_center(),
        rule(),
        // Status pills.
        row![
            badge("Active").success().dot(),
            badge("Degraded").warning().dot(),
            badge("Failed").danger().dot(),
            badge("Beta").accent().outline(),
            badge("v0.4.1"),
            badge("New").accent().solid(),
            badge("12"),
        ]
        .gap(8.0)
        .align_center(),
        // Stat row with sparklines.
        row![
            col![
                caption("Requests").dim(),
                heading("12,184"),
                sparkline(reqs.clone()).w(120.0)
            ]
            .gap(6.0),
            col![
                caption("Errors").dim(),
                heading("359"),
                sparkline(errs.clone()).color(palette().danger).w(120.0)
            ]
            .gap(6.0),
        ]
        .gap(40.0),
        line_chart([("Requests", reqs.clone()), ("Errors", errs.clone())])
            .labels(days)
            .area()
            .height(230.0),
        bar_chart([("Deploys", vec![3.0, 5.0, 2.0, 7.0, 6.0, 1.0, 0.0])])
            .labels(days)
            .height(160.0),
        row![
            col![
                text("Files").sub(),
                tree(files())
                    .open([1, 5])
                    .act(|s: &mut Show, id| s.picked = Some(id)),
            ]
            .gap(6.0)
            .w(230.0),
            col![
                text("Custom accent").sub(),
                color_picker(at!(s.custom)).act(|s: &mut Show, c| {
                    s.custom = c;
                    s.brand = 5;
                }),
            ]
            .gap(6.0)
            .w(260.0),
        ]
        .gap(32.0),
    ]
    .gap(16.0)
    .pad(24.0)
}

fn main() {
    let s = Show {
        dark: std::env::var("SHOW_DARK").is_ok(),
        contrast: std::env::var("SHOW_HC").is_ok(),
        brand: std::env::var("SHOW_BRAND")
            .ok()
            .and_then(|b| b.parse().ok())
            .unwrap_or(0),
        ..Show::default()
    };
    sketch(s, view).run_sized("Alloy showcase", 640.0, 1100.0)
}
