//! The effect primitives: radial gradient, sheen, inner shadow, grain and
//! backdrop blur. CPU tests run everywhere; GPU parity tests skip without an adapter.

use alloy_ui_core::{Color, DrawCommand, DrawList, Point, Rectangle, Size};
use alloy_ui_render::testing::{
    assert_golden, compare, default_golden_dir, gpu_or_skip, Tolerance,
};
use alloy_ui_render::{CpuRenderer, HeadlessRenderer, Image};

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

fn px(img: &Image, x: u32, y: u32) -> [u8; 4] {
    img.pixel(x, y)
}

/// Stripes under a frosted panel, plus the other effects on tiles.
fn scene() -> DrawList {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 200.0, 140.0), 0xF4F3EF, 0.0));
    for i in 0..10 {
        let c = if i % 2 == 0 { 0x2A6F97 } else { 0xC8553D };
        l.push(quad(rect(i as f32 * 20.0, 0.0, 10.0, 70.0), c, 0.0));
    }
    l.push(DrawCommand::BackdropBlur {
        bounds: rect(20.0, 20.0, 160.0, 40.0),
        radius: 10.0,
        blur: 6.0,
        tint: Color::hex(0xFFFFFF).with_alpha(0.55),
    });
    l.push(DrawCommand::RadialGradient {
        bounds: rect(10.0, 80.0, 50.0, 50.0),
        center: Point::new(25.0, 95.0),
        gradient_radius: 30.0,
        from: Color::hex(0x2A6F97),
        to: Color::hex(0x2A6F97).with_alpha(0.0),
        radius: 8.0,
    });
    l.push(quad(rect(70.0, 80.0, 50.0, 50.0), 0xE7E4DC, 8.0));
    l.push(DrawCommand::InnerShadow {
        bounds: rect(70.0, 80.0, 50.0, 50.0),
        radius: 8.0,
        color: Color::hex(0x000000).with_alpha(0.35),
        offset: Point::new(0.0, 3.0),
        blur: 8.0,
    });
    l.push(DrawCommand::Sheen {
        bounds: rect(130.0, 80.0, 60.0, 20.0),
        base: Color::hex(0xDCD8CE),
        highlight: Color::hex(0xF8F6F1),
        angle_deg: 90.0,
        position: 0.5,
        half_width: 0.25,
        radius: 6.0,
    });
    l.push(DrawCommand::Grain {
        bounds: rect(130.0, 108.0, 60.0, 22.0),
        radius: 6.0,
        amount: 0.4,
        seed: 7,
    });
    l
}

fn cpu(l: &DrawList, w: u32, h: u32, s: f32) -> Image {
    CpuRenderer::new(w, h).render_to_image(l, w, h, s)
}

#[test]
fn radial_gradient_fades_from_the_centre_and_respects_the_corner() {
    let mut l = DrawList::default();
    l.push(DrawCommand::RadialGradient {
        bounds: rect(0.0, 0.0, 40.0, 40.0),
        center: Point::new(20.0, 20.0),
        gradient_radius: 20.0,
        from: Color::hex(0xFF0000),
        to: Color::hex(0x0000FF),
        radius: 16.0,
    });
    let img = cpu(&l, 40, 40, 1.0);
    assert_eq!(
        px(&img, 0, 0),
        [255, 255, 255, 255],
        "corner is outside the rounded shape"
    );
    let centre = px(&img, 20, 20);
    assert!(centre[0] > 240 && centre[2] < 20, "{centre:?}");
    let edge = px(&img, 38, 20);
    assert!(edge[2] > 200 && edge[0] < 60, "{edge:?}");
}

#[test]
fn sheen_band_is_brightest_at_its_position_and_gone_beyond_its_width() {
    let mk = |position: f32| {
        let mut l = DrawList::default();
        l.push(DrawCommand::Sheen {
            bounds: rect(0.0, 0.0, 100.0, 10.0),
            base: Color::hex(0x808080),
            highlight: Color::hex(0xFFFFFF),
            angle_deg: 90.0,
            position,
            half_width: 0.2,
            radius: 0.0,
        });
        cpu(&l, 100, 10, 1.0)
    };
    let img = mk(0.5);
    assert!(px(&img, 50, 5)[0] > 245, "centre {:?}", px(&img, 50, 5));
    assert!(
        (px(&img, 10, 5)[0] as i32 - 128).abs() <= 1,
        "outside the band is the base"
    );
    let moved = mk(0.1);
    assert!(px(&moved, 10, 5)[0] > 245 && (px(&moved, 90, 5)[0] as i32 - 128).abs() <= 1);
}

#[test]
fn inner_shadow_stays_inside_and_is_thickest_away_from_the_light() {
    let mut l = DrawList::default();
    l.push(DrawCommand::InnerShadow {
        bounds: rect(10.0, 10.0, 60.0, 60.0),
        radius: 8.0,
        color: Color::hex(0x000000).with_alpha(0.8),
        offset: Point::new(0.0, 6.0),
        blur: 8.0,
    });
    let img = cpu(&l, 80, 80, 1.0);
    assert_eq!(
        px(&img, 5, 5),
        [255, 255, 255, 255],
        "nothing outside the shape"
    );
    let top = px(&img, 40, 12)[0];
    let bottom = px(&img, 40, 67)[0];
    let middle = px(&img, 40, 40)[0];
    assert!(
        top < 120,
        "shadow hugs the top edge when offset downwards, got {top}"
    );
    assert!(bottom > 245, "far edge stays light, got {bottom}");
    assert!(middle > 245, "centre untouched, got {middle}");
}

#[test]
fn grain_is_deterministic_zero_mean_and_bounded() {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 64.0, 64.0), 0x808080, 0.0));
    l.push(DrawCommand::Grain {
        bounds: rect(0.0, 0.0, 64.0, 64.0),
        radius: 0.0,
        amount: 0.10,
        seed: 3,
    });
    let a = cpu(&l, 64, 64, 1.0);
    let b = cpu(&l, 64, 64, 1.0);
    assert_eq!(a.rgba, b.rgba, "same seed, same pattern");
    let (mut sum, mut lo, mut hi) = (0i64, 255i32, 0i32);
    for y in 0..64 {
        for x in 0..64 {
            let v = px(&a, x, y)[0] as i32;
            sum += v as i64;
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    let mean = sum as f32 / 4096.0;
    assert!((mean - 128.0).abs() < 2.0, "mean {mean}");
    assert!(
        lo >= 128 - 14 && hi <= 128 + 14,
        "range {lo}..{hi} exceeds amount"
    );
    assert!(hi - lo > 8, "visible grain {lo}..{hi}");
    l.0.pop();
    l.push(DrawCommand::Grain {
        bounds: rect(0.0, 0.0, 64.0, 64.0),
        radius: 0.0,
        amount: 0.10,
        seed: 4,
    });
    assert_ne!(
        cpu(&l, 64, 64, 1.0).rgba,
        a.rgba,
        "seed changes the pattern"
    );
}

#[test]
fn backdrop_blur_smooths_what_is_below_and_keeps_edges_crisp() {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 80.0, 40.0), 0xFFFFFF, 0.0));
    l.push(quad(rect(40.0, 0.0, 40.0, 40.0), 0x000000, 0.0));
    l.push(DrawCommand::BackdropBlur {
        bounds: rect(0.0, 10.0, 80.0, 20.0),
        radius: 0.0,
        blur: 5.0,
        tint: Color::TRANSPARENT,
    });
    let img = cpu(&l, 80, 40, 1.0);
    // The black/white edge at x = 40 is now a ramp inside the panel...
    let (a, b) = (px(&img, 36, 20)[0], px(&img, 44, 20)[0]);
    assert!(a > b && a < 250 && b > 5, "blurred ramp {a} {b}");
    assert!((px(&img, 40, 20)[0] as i32 - 128).abs() < 12);
    // ...while the same edge outside the panel is untouched.
    assert_eq!(px(&img, 36, 2)[0], 255);
    assert_eq!(px(&img, 44, 2)[0], 0);
    // Flat colour is a fixed point, even at the panel's own edge (edge-clamped).
    assert_eq!(px(&img, 2, 10)[0], 255);
}

#[test]
fn backdrop_tint_lays_over_the_blur() {
    let mut l = DrawList::default();
    l.push(quad(rect(0.0, 0.0, 20.0, 20.0), 0x000000, 0.0));
    l.push(DrawCommand::BackdropBlur {
        bounds: rect(0.0, 0.0, 20.0, 20.0),
        radius: 0.0,
        blur: 2.0,
        tint: Color::hex(0xFFFFFF).with_alpha(0.5),
    });
    let v = px(&cpu(&l, 20, 20, 1.0), 10, 10)[0];
    assert!(
        (v as i32 - 128).abs() <= 2,
        "half white over black, got {v}"
    );
}

#[test]
fn cpu_golden_effects() {
    let img = cpu(&scene(), 400, 280, 2.0);
    assert_golden(&default_golden_dir(), "effects_2x", &img, Tolerance::CPU);
}

#[test]
fn gpu_effects_match_cpu_reference() {
    let Some(mut gpu) = gpu_or_skip(200, 140) else {
        return;
    };
    for scale in [1.0, 2.0] {
        let (w, h) = ((200.0 * scale) as u32, (140.0 * scale) as u32);
        let a = gpu.render_to_image(&scene(), w, h, scale);
        let b = cpu(&scene(), w, h, scale);
        let d = compare(&a, &b, Tolerance::GPU).expect("same size");
        assert!(
            d.bad_fraction <= Tolerance::GPU.bad_fraction,
            "GPU vs CPU effects at {scale}x: {d:?}"
        );
    }
}
