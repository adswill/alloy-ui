//! Text weight, line height, measurement and line-drawing tests.

use alloy_ui_core::{Color, DrawCommand, DrawList, Point};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image, TextMeasurer, TextSystem};

fn font(name: &str) -> Option<Vec<u8>> {
    [
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/dejavu",
        "/usr/share/fonts/TTF",
        "/Library/Fonts",
    ]
    .iter()
    .find_map(|d| std::fs::read(format!("{d}/{name}")).ok())
}

fn dejavu() -> Option<TextSystem> {
    Some(TextSystem::with_fonts([
        font("DejaVuSans.ttf")?,
        font("DejaVuSans-Bold.ttf")?,
    ]))
}

fn text(content: &str, weight: u16, line_height: Option<f32>, max_width: Option<f32>) -> DrawList {
    let mut l = DrawList::default();
    l.push(DrawCommand::Text {
        family: alloy_ui_core::FontFamily::Sans,
        weight,
        line_height,
        origin: Point::new(4.0, 4.0),
        content: content.into(),
        size: 16.0,
        color: Color::hex(0x000000),
        max_width,
    });
    l
}

fn dark_bounds(img: &Image) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for y in 0..img.height {
        for x in 0..img.width {
            if img.rgba[((y * img.width + x) * 4) as usize] < 128 {
                b = Some(match b {
                    None => (x, y, x, y),
                    Some((a, c, d, e)) => (a.min(x), c.min(y), d.max(x), e.max(y)),
                });
            }
        }
    }
    b
}

#[test]
fn bold_is_wider_than_regular() {
    let Some(sys) = dejavu() else {
        eprintln!("SKIP: DejaVu fonts not found");
        return;
    };
    let m = TextMeasurer::from_text_system(&sys);
    let r = m.measure("Hello world", 16.0, 400, None);
    let b = m.measure("Hello world", 16.0, 700, None);
    assert!(b.width > r.width + 1.0, "{} vs {}", b.width, r.width);
}

#[test]
fn measure_matches_rendered_width() {
    let Some(sys) = dejavu() else {
        eprintln!("SKIP: DejaVu fonts not found");
        return;
    };
    let mut cpu = CpuRenderer::with_text_system(300, 60, sys);
    let m = cpu.text_measurer();
    let content = "Hello world";
    let size = m.measure(content, 16.0, 400, None);
    let (cw, _) = cpu.text_system().extent(
        content,
        16.0,
        400,
        alloy_ui_core::FontFamily::Sans,
        None,
        None,
    );
    assert!((cw - size.width).abs() < 0.5);
    let img = cpu.render_to_image(&text(content, 400, None, None), 300, 60, 1.0);
    let (x0, _, x1, _) = dark_bounds(&img).expect("ink");
    // Ink lies within the advance box, and is close to it (side bearings only).
    let ink = (x1 - x0 + 1) as f32;
    assert!(
        ink <= size.width + 0.5 && ink > size.width - 4.0,
        "ink {ink} vs {}",
        size.width
    );
    assert!(x0 as f32 >= 3.0 && (x1 as f32) <= 4.0 + size.width + 1.0);
}

#[test]
fn line_height_changes_y_extent() {
    let Some(sys) = dejavu() else {
        eprintln!("SKIP: DejaVu fonts not found");
        return;
    };
    let mut cpu = CpuRenderer::with_text_system(60, 200, sys);
    let content = "aaa aaa aaa aaa";
    let ext = |cpu: &mut CpuRenderer, lh| {
        let img = cpu.render_to_image(&text(content, 400, lh, Some(50.0)), 60, 200, 1.0);
        let (_, y0, _, y1) = dark_bounds(&img).expect("ink");
        y1 - y0
    };
    let tight = ext(&mut cpu, Some(16.0));
    let default = ext(&mut cpu, None);
    let loose = ext(&mut cpu, Some(36.0));
    assert!(
        tight < default && default < loose,
        "{tight} {default} {loose}"
    );
    let m = cpu.text_measurer();
    let h = m.measure_with_line_height(content, 16.0, 400, Some(36.0), Some(50.0));
    assert!(h.height >= 72.0);
}

#[test]
fn diagonal_line_draws_along_path_only() {
    let mut cpu = CpuRenderer::new(100, 100);
    let mut l = DrawList::default();
    l.push(DrawCommand::Line {
        from: Point::new(10.0, 10.0),
        to: Point::new(90.0, 90.0),
        width: 3.0,
        color: Color::hex(0xFF0000),
    });
    let img = cpu.render_to_image(&l, 100, 100, 1.0);
    let px = |x: u32, y: u32| {
        let i = ((y * 100 + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
    };
    for t in [20, 50, 80] {
        assert_eq!(px(t, t), [255, 0, 0], "on line at {t}");
    }
    for (x, y) in [(80, 20), (20, 80), (50, 70), (5, 50), (95, 10)] {
        assert_eq!(px(x, y), [255, 255, 255], "off line at {x},{y}");
    }
}
