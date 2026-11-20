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
    @location(4) params: vec4<f32>,
    // pad, kind, unused, unused
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

    if (in.kind < 0.5) {
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
    } else if (in.kind < 2.5) {
        // Glyph mask
        return fill * texel.a;
    }
    // Premultiplied texture
    let cov = cover(sd_rrect(in.p, in.rect, 0.0));
    return texel * (in.color.a * cov);
}
