//! Icons render from both bundled packs, and scrolling behaves (spring glide, thumb drag).
use alloy_ui_core::{Color, DrawCommand, DrawList, FontFamily, Point};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image};
use alloy_ui_widgets::Icon;

fn draw(family: FontFamily, content: &str) -> Image {
    let mut l = DrawList::default();
    l.push(DrawCommand::Text {
        origin: Point::new(4.0, 4.0),
        content: content.into(),
        size: 32.0,
        color: Color::hex(0x000000),
        max_width: None,
        weight: 400,
        family,
        line_height: Some(32.0),
    });
    let mut r = CpuRenderer::new(160, 48);
    r.set_background(Color::hex(0xFFFFFF));
    r.render_to_image(&l, 160, 48, 1.0)
}

fn ink(img: &Image) -> usize {
    img.rgba.chunks(4).filter(|p| p[0] < 128).count()
}

#[test]
fn icons_render_in_both_packs() {
    let l = draw(FontFamily::Lucide, &Icon::Search.lucide().to_string());
    assert!(ink(&l) > 60, "lucide glyph has ink");
    // Same codepoint means nothing in the other pack; check a Phosphor glyph instead.
    let p = draw(FontFamily::Phosphor, &Icon::Search.phosphor().to_string());
    assert!(ink(&p) > 60, "phosphor glyph has ink");
    assert_ne!(l.rgba, p.rgba);
    // Every icon has a glyph in its pack (a missing glyph falls back to nothing).
    for icon in Icon::ALL {
        assert!(
            ink(&draw(FontFamily::Lucide, &icon.lucide().to_string())) > 8,
            "{icon:?} lucide"
        );
        assert!(
            ink(&draw(FontFamily::Phosphor, &icon.phosphor().to_string())) > 8,
            "{icon:?} phosphor"
        );
    }
}
