//! The whole icon packs, looked up by name. Run with `--features icons-full`.
#![cfg(feature = "icons-full")]
use alloy_ui::prelude::*;
use alloy_ui::testing::Harness;
use alloy_ui::DrawCommand;

fn glyph_of(pack: IconPack, name: &str) -> (String, usize) {
    let name = name.to_string();
    let h = Harness::new(
        sketch((), move |_| {
            set_icon_pack(pack);
            col![icon_named(&name).size(32.0)].page()
        }),
        120.0,
        80.0,
    );
    let glyph = h
        .draw_list()
        .0
        .iter()
        .find_map(|c| match c {
            DrawCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .expect("an icon is drawn");
    // How much of the frame the icon inked, so a blank glyph cannot pass.
    let ink = {
        let img = alloy_ui_render::Image::decode_png(&h.png()).expect("png");
        img.rgba.chunks(4).filter(|p| p[0] < 200).count()
    };
    (glyph, ink)
}

#[test]
fn names_outside_the_built_in_set_resolve_in_both_packs_and_draw() {
    for pack in [IconPack::Lucide, IconPack::Phosphor] {
        let help = glyph_of(pack, "no-such-icon-anywhere").0;
        for name in ["anchor", "rocket", "compass"] {
            let (g, ink) = glyph_of(pack, name);
            assert_ne!(g, help, "{pack:?} {name} resolves to a real glyph");
            assert!(ink > 40, "{pack:?} {name} actually draws ({ink} px)");
        }
    }
}

#[test]
fn unknown_names_show_the_help_icon_so_the_gap_is_visible() {
    let a = glyph_of(IconPack::Lucide, "no-such-icon-anywhere").0;
    let b = glyph_of(IconPack::Lucide, "also-missing").0;
    assert_eq!(a, b);
}
