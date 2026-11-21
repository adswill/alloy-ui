// Alloy 2D shader. Mirrors cpu.rs formula-for-formula.
// Output is premultiplied alpha; blend state is One / OneMinusSrcAlpha.

struct Globals {
    size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct Instance {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) border_color: vec4<f32>,
    @location(3) uv: vec4<f32>,
    // radius, border width, blur, unused
    // (for gradients `uv` carries the end colour)
    @location(4) params: vec4<f32>,
    // pad, kind, gradient dir x, gradient dir y
    @location(5) info: vec4<f32>,
};

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) p: vec2<f32>,
    @location(1) @interpolate(flat) rect: vec4<f32>,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) @interpolate(flat) border_color: vec4<f32>,
    @location(4) @interpolate(flat) uv: vec4<f32>,
    @location(5) @interpolate(flat) params: vec4<f32>,
    @location(6) @interpolate(flat) kind: f32,
    @location(7) @interpolate(flat) dir: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: Instance) -> VOut {
    let corner = vec2<f32>(f32(vi & 1u), f32(vi >> 1u));
    let pad = inst.info.x;
    let origin = inst.rect.xy - vec2<f32>(pad);
    let extent = inst.rect.zw + vec2<f32>(2.0 * pad);
    let p = origin + corner * extent;
    let ndc = vec2<f32>(p.x / globals.size.x * 2.0 - 1.0, 1.0 - p.y / globals.size.y * 2.0);
    var o: VOut;
    o.pos = vec4<f32>(ndc, 0.0, 1.0);
    o.p = p;
    o.rect = inst.rect;
    o.color = inst.color;
    o.border_color = inst.border_color;
    o.uv = inst.uv;
    o.params = inst.params;
    o.kind = inst.info.y;
    o.dir = inst.info.zw;
    return o;
}

fn sd_rrect(p: vec2<f32>, rect: vec4<f32>, r: f32) -> f32 {
    let h = rect.zw * 0.5;
    let c = rect.xy + h;
    let rr = max(min(r, min(h.x, h.y)), 0.0);
    let q = abs(p - c) - h + vec2<f32>(rr);
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - rr;
}

fn cover(sd: f32) -> f32 {
    return clamp(0.5 - sd, 0.0, 1.0);
}

fn premul(c: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(c.rgb * c.a, c.a);
}

// 4x4 ordered-dither threshold in [0, 16); identical to `bayer4` in cpu.rs.
fn bayer4(p: vec2<f32>) -> f32 {
    let x = u32(floor(p.x)) & 3u;
    let y = u32(floor(p.y)) & 3u;
    let xy = x ^ y;
    return f32(((y & 1u) << 3u) | ((xy & 1u) << 2u) | (((y >> 1u) & 1u) << 1u) | ((xy >> 1u) & 1u));
}

// Per-pixel noise in [-1, 1); identical to `grain_noise` in cpu.rs.
fn grain_noise(px: vec2<u32>, seed: u32) -> f32 {
    var h = (px.x * 0x9E3779B1u) ^ (px.y * 0x85EBCA6Bu) ^ (seed * 0xC2B2AE35u);
    h = h ^ (h >> 15u);
    h = h * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    h = h * 0x297A2D39u;
    h = h ^ (h >> 15u);
    return f32(h >> 8u) / 8388608.0 - 1.0;
}

fn dither_g(g: vec4<f32>, p: vec2<f32>) -> vec4<f32> {
    let dither = ((bayer4(p) + 0.5) / 16.0 - 0.5) / 255.0;
    return vec4<f32>(max(g.rgb + vec3<f32>(dither * g.a), vec3<f32>(0.0)), g.a);
}

fn smooth01(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

// One-dimensional gaussian blur of `tex` around integer pixel `px` along `axis`,
// edge-clamped; weights match `gaussian` in cpu.rs.
fn blur_axis(px: vec2<i32>, axis: vec2<i32>, sigma: f32) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(tex));
    let r = i32(clamp(ceil(sigma * 3.0), 1.0, 120.0));
    var acc = vec4<f32>(0.0);
    var wsum = 0.0;
    for (var i = -r; i <= r; i = i + 1) {
        let w = exp(-f32(i * i) / (2.0 * sigma * sigma));
        let q = clamp(px + axis * i, vec2<i32>(0), dims - vec2<i32>(1));
        acc = acc + textureLoad(tex, q, 0) * w;
        wsum = wsum + w;
    }
    return acc / wsum;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // Sample unconditionally to stay in uniform control flow.
    let t = (in.p - in.rect.xy) / in.rect.zw;
    let texel = textureSampleLevel(tex, samp, mix(in.uv.xy, in.uv.zw, t), 0.0);

    let radius = in.params.x;
    let bw = in.params.y;
    let blur = in.params.z;
    let fill = premul(in.color);
    let border = premul(in.border_color);

    if (in.kind > 4.5 && in.kind < 5.5) {
        // Triangle: vertices in border_color (a, b) and uv.xy (c); radius holds
        // the edge flags. A flagged edge is part of the polygon outline and is
        // anti-aliased. An unflagged edge is an interior seam shared with a
        // neighbouring triangle: it is a hard pixel-centre test with a
        // direction-based tie-break, so exactly one neighbour claims each pixel
        // and translucent fills show no seam.
        let a = in.border_color.xy;
        let b = in.border_color.zw;
        let c = in.uv.xy;
        let flags = u32(radius + 0.5);
        let orient = select(-1.0, 1.0, (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) > 0.0);
        var cov = 1.0;
        for (var i = 0u; i < 3u; i = i + 1u) {
            var p0 = a; var p1 = b;
            if (i == 1u) { p0 = b; p1 = c; }
            if (i == 2u) { p0 = c; p1 = a; }
            let e = p1 - p0;
            let n = vec2<f32>(e.y, -e.x) * orient / max(length(e), 1e-6);
            let d = dot(in.p - p0, n);
            if (((flags >> i) & 1u) == 1u) {
                cov = min(cov, cover(d));
            } else {
                // Same arithmetic for both neighbours: from the lexicographically
                // smaller endpoint; the positively oriented side claims exact ties.
                let fwd = p0.x < p1.x || (p0.x == p1.x && p0.y <= p1.y);
                let lo = select(p1, p0, fwd);
                let hi = select(p0, p1, fwd);
                var third = c;
                if (i == 1u) { third = a; }
                if (i == 2u) { third = b; }
                let c0 = (hi.x - lo.x) * (in.p.y - lo.y) - (hi.y - lo.y) * (in.p.x - lo.x);
                let side = (hi.x - lo.x) * (third.y - lo.y) - (hi.y - lo.y) * (third.x - lo.x);
                let inside = (c0 > 0.0 && side > 0.0) || (c0 < 0.0 && side < 0.0) || (c0 == 0.0 && side > 0.0);
                cov = min(cov, select(0.0, 1.0, inside));
            }
        }
        return fill * cov;
    } else if (in.kind < 0.5) {
        // Shadow
        let sd = sd_rrect(in.p, in.rect, radius);
        var a = cover(sd);
        if (blur > 0.0) {
            let s = clamp((sd + blur * 0.5) / blur, 0.0, 1.0);
            a = 1.0 - s * s * (3.0 - 2.0 * s);
        }
        return fill * a;
    } else if (in.kind < 1.5) {
        // Quad
        let outer = cover(sd_rrect(in.p, in.rect, radius));
        var inner = 1.0;
        if (bw > 0.0) {
            let r = vec4<f32>(in.rect.xy + vec2<f32>(bw), max(in.rect.zw - vec2<f32>(2.0 * bw), vec2<f32>(0.0)));
            inner = cover(sd_rrect(in.p, r, max(radius - bw, 0.0)));
        }
        return outer * mix(border, fill, inner);
    } else if (in.kind > 10.5) {
        // Backdrop blur passes: 11 horizontal into scratch, 12 vertical + tint.
        let px = vec2<i32>(floor(in.p));
        if (in.kind < 11.5) {
            return blur_axis(px, vec2<i32>(1, 0), blur);
        }
        let blurred = blur_axis(px, vec2<i32>(0, 1), blur);
        let cov = cover(sd_rrect(in.p, in.rect, radius));
        return (fill + blurred * (1.0 - fill.a)) * cov;
    } else if (in.kind > 9.5) {
        // Backdrop without a framebuffer copy available: tint only.
        return fill * cover(sd_rrect(in.p, in.rect, radius));
    } else if (in.kind > 8.5) {
        // Grain
        let cov = cover(sd_rrect(in.p, in.rect, radius));
        let n = grain_noise(vec2<u32>(floor(in.p)), u32(in.params.w));
        let a = abs(n) * in.color.a * cov;
        if (n >= 0.0) {
            return vec4<f32>(a, a, a, a);
        }
        return vec4<f32>(0.0, 0.0, 0.0, a);
    } else if (in.kind > 7.5) {
        // Inner shadow
        let cov = cover(sd_rrect(in.p, in.rect, radius));
        let shifted = sd_rrect(in.p - in.dir, in.rect, radius);
        var a = cover(shifted);
        if (blur > 0.0) {
            a = smooth01(clamp((shifted + blur * 0.5) / blur, 0.0, 1.0));
        }
        return fill * (a * cov);
    } else if (in.kind > 6.5) {
        // Sheen: one highlight band across a base colour
        let cov = cover(sd_rrect(in.p, in.rect, radius));
        let len = max(abs(in.dir.x) * in.rect.z + abs(in.dir.y) * in.rect.w, 1e-4);
        let gt = clamp(0.5 + dot(in.p - (in.rect.xy + in.rect.zw * 0.5), in.dir) / len, 0.0, 1.0);
        let d = clamp(abs(gt - blur) / in.params.w, 0.0, 1.0);
        let g = dither_g(mix(fill, premul(in.uv), 1.0 - smooth01(d)), in.p);
        return g * cov;
    } else if (in.kind > 5.5) {
        // Radial gradient
        let cov = cover(sd_rrect(in.p, in.rect, radius));
        let d = length(in.p - in.dir);
        let g = dither_g(mix(fill, premul(in.uv), clamp(d / blur, 0.0, 1.0)), in.p);
        return g * cov;
    } else if (in.kind > 3.5) {
        // Gradient quad (end colour travels in `uv`)
        let outer = cover(sd_rrect(in.p, in.rect, radius));
        var inner = 1.0;
        if (bw > 0.0) {
            let r = vec4<f32>(in.rect.xy + vec2<f32>(bw), max(in.rect.zw - vec2<f32>(2.0 * bw), vec2<f32>(0.0)));
            inner = cover(sd_rrect(in.p, r, max(radius - bw, 0.0)));
        }
        let len = max(abs(in.dir.x) * in.rect.z + abs(in.dir.y) * in.rect.w, 1e-4);
        let gt = clamp(0.5 + dot(in.p - (in.rect.xy + in.rect.zw * 0.5), in.dir) / len, 0.0, 1.0);
        let dither = ((bayer4(in.p) + 0.5) / 16.0 - 0.5) / 255.0;
        var g = mix(fill, premul(in.uv), gt);
        g = vec4<f32>(max(g.rgb + vec3<f32>(dither * g.a), vec3<f32>(0.0)), g.a);
        return outer * mix(border, g, inner);
    } else if (in.kind < 2.5) {
        // Glyph mask
        return fill * texel.a;
    }
    // Premultiplied texture
    let cov = cover(sd_rrect(in.p, in.rect, 0.0));
    return texel * (in.color.a * cov);
}
