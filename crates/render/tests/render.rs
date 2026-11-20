//! Renderer tests. CPU tests run everywhere; GPU tests skip (with a logged
//! message) when no adapter exists. Goldens are produced by the CPU reference.

use alloy_ui_core::{Border, Color, DrawCommand, DrawList, Point, Rectangle, Shadow, Size};
use alloy_ui_render::testing::{
    assert_golden, compare, default_golden_dir, gpu_or_skip, Tolerance,
};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image, Renderer, TextSystem};

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn quad(b: Rectangle, fill: u32, radius: f32) -> DrawCommand {
    DrawCommand::Quad {
        bounds: b,
        fill: Color::hex(fill),
        radius,
        border: None,
        shadow: None,
    }
}

fn scene_shapes() -> DrawList {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 160.0, 120.0), 0xF2F2EE, 0.0));
    l.push(quad(rect(10.0, 10.0, 60.0, 40.0), 0x2A6F97, 0.0));
    l.push(quad(rect(80.0, 10.0, 60.0, 40.0), 0xC8553D, 12.0));
    l.push(DrawCommand::Quad {
        bounds: rect(10.0, 60.0, 60.0, 40.0),
        fill: Color::hex(0xFFFFFF),
        radius: 8.0,
        border: Some(Border {
            color: Color::hex(0x222222),
            width: 3.0,
        }),
        shadow: Some(Shadow {
            color: Color::hex(0x000000).with_alpha(0.35),
            offset: Point::new(0.0, 4.0),
            blur: 10.0,
        }),
    });
    // Clipped: the circle-ish quad is cut by the clip rectangle.
    l.push(DrawCommand::PushClip(rect(90.0, 70.0, 30.0, 30.0)));
    l.push(quad(rect(80.0, 60.0, 60.0, 60.0), 0x3B7A57, 30.0));
    l.push(DrawCommand::PopClip);
    l
}

fn px(img: &Image, x: u32, y: u32) -> [u8; 4] {
    img.pixel(x, y)
}

#[test]
fn cpu_quad_fill_and_antialiasing() {
    let mut r = CpuRenderer::new(32, 32);
    let mut l = DrawList::default();
    l.push(quad(rect(4.0, 4.0, 16.0, 16.0), 0xFF0000, 0.0));
    l.push(quad(rect(24.5, 4.0, 4.0, 4.0), 0x000000, 0.0));
    let img = r.render_to_image(&l, 32, 32, 1.0);
    assert_eq!(px(&img, 10, 10), [255, 0, 0, 255]);
    assert_eq!(px(&img, 2, 2), [255, 255, 255, 255]);
    // Half-covered column at x=24 is mid grey.
    let edge = px(&img, 24, 5);
    assert!((120..=135).contains(&edge[0]), "edge {edge:?}");
}

#[test]
fn cpu_clip_stack_nests() {
    let mut r = CpuRenderer::new(20, 20);
    let mut l = DrawList::default();
    l.push(DrawCommand::PushClip(rect(0.0, 0.0, 10.0, 10.0)));
    l.push(DrawCommand::PushClip(rect(5.0, 5.0, 10.0, 10.0)));
    l.push(quad(rect(0.0, 0.0, 20.0, 20.0), 0x000000, 0.0));
    l.push(DrawCommand::PopClip);
    l.push(quad(rect(0.0, 0.0, 3.0, 3.0), 0xFF0000, 0.0));
    l.push(DrawCommand::PopClip);
    let img = r.render_to_image(&l, 20, 20, 1.0);
    assert_eq!(px(&img, 7, 7), [0, 0, 0, 255]);
    assert_eq!(px(&img, 4, 4), [255, 255, 255, 255]);
    assert_eq!(px(&img, 12, 12), [255, 255, 255, 255]);
    assert_eq!(px(&img, 1, 1), [255, 0, 0, 255]);
    assert_eq!(px(&img, 5, 1), [255, 255, 255, 255]);
}

#[test]
fn cpu_scale_factor_and_border() {
    let mut r = CpuRenderer::new(40, 40);
    let mut l = DrawList::default();
    l.push(DrawCommand::Quad {
        bounds: rect(2.0, 2.0, 16.0, 16.0),
        fill: Color::hex(0xFFFFFF),
        radius: 0.0,
        border: Some(Border {
            color: Color::hex(0x0000FF),
            width: 2.0,
        }),
        shadow: None,
    });
    let img = r.render_to_image(&l, 40, 40, 2.0);
    assert_eq!(px(&img, 5, 5), [0, 0, 255, 255]); // inside 4px border at 2x
    assert_eq!(px(&img, 20, 20), [255, 255, 255, 255]);
    assert_eq!(px(&img, 2, 2), [255, 255, 255, 255]);
}

#[test]
fn cpu_image_draw() {
    let mut r = CpuRenderer::new(16, 16);
    let src = Image::filled(2, 2, [0, 255, 0, 255]);
    let id = r.register_image(&src);
    let mut l = DrawList::default();
    l.push(DrawCommand::Image {
        bounds: rect(4.0, 4.0, 8.0, 8.0),
        id,
    });
    l.push(DrawCommand::Image {
        bounds: rect(0.0, 0.0, 2.0, 2.0),
        id: 9999, // unknown id is skipped
    });
    let img = r.render_to_image(&l, 16, 16, 1.0);
    assert_eq!(px(&img, 8, 8), [0, 255, 0, 255]);
    assert_eq!(px(&img, 0, 0), [255, 255, 255, 255]);
    r.unregister_image(id);
    let img = r.render_to_image(&l, 16, 16, 1.0);
    assert_eq!(px(&img, 8, 8), [255, 255, 255, 255]);
}

#[test]
fn cpu_golden_shapes() {
    let mut r = CpuRenderer::new(160, 120);
    let img = r.render_to_image(&scene_shapes(), 160, 120, 1.0);
    assert_golden(&default_golden_dir(), "shapes_1x", &img, Tolerance::CPU);
}

#[test]
fn cpu_golden_shapes_2x() {
    let mut r = CpuRenderer::new(320, 240);
    let img = r.render_to_image(&scene_shapes(), 320, 240, 2.0);
    assert_golden(&default_golden_dir(), "shapes_2x", &img, Tolerance::CPU);
}

#[test]
fn cpu_text_draws_when_fonts_exist() {
    let mut r = CpuRenderer::with_text_system(120, 40, TextSystem::new());
    if r.text_system().face_count() == 0 {
        eprintln!("SKIP: no system fonts available");
        return;
    }
    let mut l = DrawList::default();
    l.push(DrawCommand::Text {
        origin: Point::new(4.0, 4.0),
        content: "Hello".into(),
        size: 16.0,
        color: Color::hex(0x000000),
        max_width: None,
    });
    let img = r.render_to_image(&l, 120, 40, 1.0);
    let dark = img.rgba.chunks_exact(4).filter(|p| p[0] < 128).count();
    assert!(dark > 20, "expected glyph pixels, found {dark}");
}

#[test]
fn png_roundtrip() {
    let mut img = Image::filled(3, 2, [10, 20, 30, 255]);
    img.rgba[0] = 200;
    let back = Image::decode_png(&img.encode_png().unwrap()).unwrap();
    assert_eq!(img, back);
}

#[test]
fn compare_detects_differences() {
    let a = Image::filled(4, 4, [0, 0, 0, 255]);
    let mut b = a.clone();
    assert_eq!(compare(&a, &b, Tolerance::CPU).unwrap().max_channel, 0);
    b.rgba[0] = 50;
    let d = compare(&a, &b, Tolerance::CPU).unwrap();
    assert_eq!(d.max_channel, 50);
    assert!(d.bad_fraction > 0.0);
    assert!(compare(&a, &Image::filled(2, 2, [0; 4]), Tolerance::CPU).is_none());
}

#[test]
fn shader_validates() {
    let src = include_str!("../src/shader.wgsl");
    let module = naga::front::wgsl::parse_str(src).expect("WGSL parses");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("WGSL validates");
}

#[test]
fn gpu_matches_cpu_reference() {
    let Some(mut gpu) = gpu_or_skip(160, 120) else {
        return;
    };
    let mut cpu = CpuRenderer::new(160, 120);
    for scale in [1.0, 2.0] {
        let (w, h) = ((160.0 * scale) as u32, (120.0 * scale) as u32);
        let list = scene_shapes();
        let a = gpu.render_to_image(&list, w, h, scale);
        let b = cpu.render_to_image(&list, w, h, scale);
        let d = compare(&a, &b, Tolerance::GPU).expect("same size");
        assert!(
            d.bad_fraction <= Tolerance::GPU.bad_fraction,
            "GPU vs CPU at {scale}x: {d:?}"
        );
    }
    // And against the stored golden.
    let img = gpu.render_to_image(&scene_shapes(), 160, 120, 1.0);
    assert_golden(&default_golden_dir(), "shapes_1x", &img, Tolerance::GPU);
}

#[test]
fn gpu_images_and_render_trait() {
    let Some(mut gpu) = gpu_or_skip(16, 16) else {
        return;
    };
    let id = gpu.register_image(&Image::filled(2, 2, [0, 255, 0, 255]));
    let mut l = DrawList::default();
    l.push(DrawCommand::Image {
        bounds: rect(4.0, 4.0, 8.0, 8.0),
        id,
    });
    gpu.render(&l, 1.0);
    let img = gpu.last_image().expect("rendered").clone();
    assert_eq!(px(&img, 8, 8), [0, 255, 0, 255]);
    assert_eq!(px(&img, 0, 0), [255, 255, 255, 255]);
}

#[test]
fn gpu_text_matches_cpu_roughly() {
    let Some(mut gpu) = gpu_or_skip(120, 40) else {
        return;
    };
    let mut cpu = CpuRenderer::new(120, 40);
    if cpu.text_system().face_count() == 0 {
        eprintln!("SKIP: no system fonts available");
        return;
    }
    let mut l = DrawList::default();
    l.push(DrawCommand::Text {
        origin: Point::new(4.0, 4.0),
        content: "Hello".into(),
        size: 16.0,
        color: Color::hex(0x000000),
        max_width: None,
    });
    let a = gpu.render_to_image(&l, 120, 40, 1.0);
    let b = cpu.render_to_image(&l, 120, 40, 1.0);
    let d = compare(&a, &b, Tolerance::GPU).unwrap();
    assert!(d.bad_fraction < 0.01, "{d:?}");
}
