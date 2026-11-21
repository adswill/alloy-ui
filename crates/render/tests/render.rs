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
        family: alloy_ui_core::FontFamily::Sans,
        weight: 400,
        line_height: None,
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

fn scene_gradients() -> DrawList {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 160.0, 120.0), 0xF2F2EE, 0.0));
    l.push(DrawCommand::gradient_quad(
        rect(10.0, 10.0, 60.0, 40.0),
        Color::hex(0x2A6F97),
        Color::hex(0xF4D35E),
        0.0,
        0.0,
    ));
    l.push(DrawCommand::gradient_quad(
        rect(80.0, 10.0, 60.0, 40.0),
        Color::hex(0xC8553D),
        Color::hex(0x3B7A57).with_alpha(0.4),
        90.0,
        14.0,
    ));
    l.push(
        DrawCommand::gradient_quad(
            rect(10.0, 60.0, 60.0, 40.0),
            Color::hex(0xFFFFFF),
            Color::hex(0x222222),
            45.0,
            8.0,
        )
        .with_border(Color::hex(0x222222), 3.0)
        .with_shadow(
            Color::hex(0x000000).with_alpha(0.35),
            Point::new(0.0, 4.0),
            10.0,
        ),
    );
    l
}

fn gradient_strip(angle: f32, radius: f32) -> Image {
    let mut r = CpuRenderer::new(100, 20);
    let mut l = DrawList::default();
    l.push(DrawCommand::gradient_quad(
        rect(0.0, 0.0, 100.0, 20.0),
        Color::hex(0x000000),
        Color::hex(0xFFFFFF),
        angle,
        radius,
    ));
    r.render_to_image(&l, 100, 20, 1.0)
}

#[test]
fn cpu_gradient_endpoints_and_angle_90() {
    let img = gradient_strip(90.0, 0.0);
    let (l, m, r) = (px(&img, 0, 10), px(&img, 50, 10), px(&img, 99, 10));
    assert!(l[0] <= 3, "start {l:?}");
    assert!(r[0] >= 252, "end {r:?}");
    assert!((120..=135).contains(&m[0]), "mid {m:?}");
    // Monotonic left to right (allow dither jitter), constant down a column.
    for x in 1..100 {
        assert!(u16::from(px(&img, x, 10)[0]) + 3 >= u16::from(px(&img, x - 1, 10)[0]));
    }
    assert!(px(&img, 50, 2)[0].abs_diff(px(&img, 50, 17)[0]) <= 2);

    // Angle 0 runs top to bottom.
    let mut r0 = CpuRenderer::new(20, 100);
    let mut list = DrawList::default();
    list.push(DrawCommand::gradient_quad(
        rect(0.0, 0.0, 20.0, 100.0),
        Color::hex(0xFF0000),
        Color::hex(0x0000FF),
        0.0,
        0.0,
    ));
    let v = r0.render_to_image(&list, 20, 100, 1.0);
    assert!(px(&v, 10, 0)[0] >= 252 && px(&v, 10, 0)[2] <= 3);
    assert!(px(&v, 10, 99)[2] >= 252 && px(&v, 10, 99)[0] <= 3);
    assert!(px(&v, 2, 50)[0].abs_diff(px(&v, 17, 50)[0]) <= 2);
}

#[test]
fn cpu_gradient_degenerate_equals_flat_and_dither_breaks_bands() {
    let mut r = CpuRenderer::new(8, 8);
    let mut l = DrawList::default();
    let c = Color::hex(0x336699);
    l.push(DrawCommand::gradient_quad(
        rect(0.0, 0.0, 8.0, 8.0),
        c,
        c,
        30.0,
        0.0,
    ));
    let img = r.render_to_image(&l, 8, 8, 1.0);
    for y in 0..8 {
        for x in 0..8 {
            let p = px(&img, x, y);
            assert!(
                p[0].abs_diff(0x33) <= 1 && p[1].abs_diff(0x66) <= 1 && p[2].abs_diff(0x99) <= 1
            );
        }
    }
    // A very shallow ramp (4 levels over 100px) must mix adjacent levels
    // within a column band rather than switching at one hard edge.
    let mut r = CpuRenderer::new(100, 8);
    let mut l = DrawList::default();
    l.push(DrawCommand::gradient_quad(
        rect(0.0, 0.0, 100.0, 8.0),
        Color::hex(0x808080),
        Color::hex(0x848484),
        90.0,
        0.0,
    ));
    let img = r.render_to_image(&l, 100, 8, 1.0);
    let mixed = (0..100)
        .filter(|&x| {
            (0..8)
                .map(|y| px(&img, x, y)[0])
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1
        })
        .count();
    assert!(mixed >= 4, "dither columns {mixed}");
}

#[test]
fn cpu_gradient_rounded_corners_clip_and_border() {
    let mut r = CpuRenderer::new(40, 40);
    let mut l = DrawList::default();
    l.push(DrawCommand::gradient_quad(
        rect(0.0, 0.0, 40.0, 40.0),
        Color::hex(0xFF0000),
        Color::hex(0x0000FF),
        90.0,
        16.0,
    ));
    let img = r.render_to_image(&l, 40, 40, 1.0);
    assert_eq!(px(&img, 0, 0), [255, 255, 255, 255], "corner clipped");
    assert_eq!(px(&img, 39, 39), [255, 255, 255, 255]);
    assert!(px(&img, 20, 20)[0] < 255 && px(&img, 20, 20)[1] < 255);

    let mut l = DrawList::default();
    l.push(
        DrawCommand::gradient_quad(
            rect(0.0, 0.0, 40.0, 40.0),
            Color::hex(0xFF0000),
            Color::hex(0x0000FF),
            90.0,
            0.0,
        )
        .with_border(Color::hex(0x00FF00), 4.0),
    );
    let img = r.render_to_image(&l, 40, 40, 1.0);
    assert_eq!(px(&img, 1, 20), [0, 255, 0, 255]);
    assert!(px(&img, 10, 20)[0] > 150);
}

#[test]
fn cpu_golden_gradients() {
    let mut r = CpuRenderer::new(160, 120);
    let img = r.render_to_image(&scene_gradients(), 160, 120, 1.0);
    assert_golden(&default_golden_dir(), "gradients_1x", &img, Tolerance::CPU);
}

#[test]
fn gpu_gradient_matches_cpu_reference() {
    let Some(mut gpu) = gpu_or_skip(160, 120) else {
        return;
    };
    let mut cpu = CpuRenderer::new(160, 120);
    for scale in [1.0, 2.0] {
        let (w, h) = ((160.0 * scale) as u32, (120.0 * scale) as u32);
        let list = scene_gradients();
        let a = gpu.render_to_image(&list, w, h, scale);
        let b = cpu.render_to_image(&list, w, h, scale);
        let d = compare(&a, &b, Tolerance::GPU).expect("same size");
        assert!(
            d.bad_fraction <= Tolerance::GPU.bad_fraction,
            "GPU vs CPU gradient at {scale}x: {d:?}"
        );
    }
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
        family: alloy_ui_core::FontFamily::Sans,
        weight: 400,
        line_height: None,
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

fn poly(points: &[(f32, f32)], fill: Color) -> DrawCommand {
    DrawCommand::Polygon {
        points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        fill,
    }
}

#[test]
fn cpu_polygon_concave_is_filled_without_seams() {
    // A concave "arrow" with translucent fill: every interior pixel must have
    // the same alpha, i.e. the ear-clip diagonals leave neither gap nor overlap.
    let outline = [
        (10.0, 10.0),
        (50.0, 10.0),
        (50.0, 30.0),
        (30.0, 30.0),
        (30.0, 50.0),
        (10.0, 50.0),
    ];
    let mut l = DrawList::default();
    l.push(poly(&outline, Color::hex(0x2A6F97).with_alpha(0.5)));
    let mut r = CpuRenderer::new(60, 60);
    r.set_background(Color::hex(0xFFFFFF));
    let img = r.render_to_image(&l, 60, 60, 1.0);
    let inside = px(&img, 15, 15);
    for (x, y) in [(12, 12), (40, 20), (20, 40), (28, 28), (20, 20), (25, 25)] {
        assert_eq!(px(&img, x, y), inside, "uniform interior at {x},{y}");
    }
    assert_ne!(inside, [255, 255, 255, 255], "filled");
    // The notch (outside the concave corner) stays background.
    assert_eq!(px(&img, 40, 40), [255, 255, 255, 255]);
    // Same polygon wound the other way round fills identically.
    let mut rev = outline;
    rev.reverse();
    let mut l2 = DrawList::default();
    l2.push(poly(&rev, Color::hex(0x2A6F97).with_alpha(0.5)));
    assert_eq!(r.render_to_image(&l2, 60, 60, 1.0), img);
}

#[test]
fn cpu_polygon_edges_are_antialiased() {
    // A right triangle: the hypotenuse pixels are partial coverage.
    let mut l = DrawList::default();
    l.push(poly(
        &[(4.0, 4.0), (36.0, 4.0), (4.0, 36.0)],
        Color::hex(0x000000),
    ));
    let mut r = CpuRenderer::new(40, 40);
    let img = r.render_to_image(&l, 40, 40, 1.0);
    assert_eq!(px(&img, 10, 10), [0, 0, 0, 255]);
    let partial = (0..40u32)
        .filter(|&i| {
            let p = px(&img, i, 39 - i).first().copied();
            matches!(p, Some(v) if v > 20 && v < 235)
        })
        .count();
    assert!(partial > 0, "hypotenuse has intermediate shades");
    assert_eq!(px(&img, 30, 30), [255, 255, 255, 255]);
}

#[test]
fn gpu_polygon_matches_cpu() {
    let Some(mut g) = gpu_or_skip(80, 80) else {
        return;
    };
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 80.0, 80.0), 0xF2F2EE, 0.0));
    l.push(poly(
        &[
            (10.0, 10.0),
            (70.0, 14.0),
            (50.0, 40.0),
            (68.0, 70.0),
            (12.0, 62.0),
        ],
        Color::hex(0xC8553D).with_alpha(0.7),
    ));
    let mut c = CpuRenderer::new(80, 80);
    let want = c.render_to_image(&l, 80, 80, 1.0);
    let got = g.render_to_image(&l, 80, 80, 1.0);
    let d = compare(&want, &got, Tolerance::GPU).expect("sizes");
    assert!(d.bad_fraction <= 0.01, "{d:?}");
}
