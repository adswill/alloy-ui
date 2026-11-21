//! The bundled italic and icon faces really render (fonts are embedded, so no system fonts).
use alloy_ui_core::{Color, DrawCommand, DrawList, FontFamily, Point};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image};

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
fn italic_faces_differ_from_upright() {
    for (up, it) in [
        (FontFamily::Sans, FontFamily::SansItalic),
        (FontFamily::Mono, FontFamily::MonoItalic),
    ] {
        let (a, b) = (draw(up, "Hxlf"), draw(it, "Hxlf"));
        assert!(ink(&a) > 100 && ink(&b) > 100);
        assert_ne!(a.rgba, b.rgba, "{it:?} must be a real italic, not a copy");
    }
}
