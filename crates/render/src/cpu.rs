//! Pure-CPU reference rasteriser. Implements the same formulas as the WGSL
//! shader in `shader.wgsl`, so it doubles as the specification the GPU path is
//! tested against and as a fallback on machines without any graphics adapter.

use std::collections::HashMap;

use alloy_ui_core::{Color, DrawList};

use crate::image::Image;
use crate::scene::{flatten, Kind, Prim, Source};
use crate::text::TextSystem;
use crate::{HeadlessRenderer, Renderer};

struct Stored {
    width: u32,
    height: u32,
    /// Premultiplied RGBA8.
    rgba: Vec<u8>,
}

/// Software renderer producing an [`Image`]. Deterministic on every platform.
pub struct CpuRenderer {
    width: u32,
    height: u32,
    background: Color,
    images: HashMap<u64, Stored>,
    next_id: u64,
    text: TextSystem,
    last: Option<Image>,
}

impl CpuRenderer {
    /// A renderer whose [`Renderer::render`] target is `width`x`height`, using
    /// the OS fonts for text and an opaque white background.
    pub fn new(width: u32, height: u32) -> Self {
        Self::with_text_system(width, height, TextSystem::new())
    }

    /// As [`CpuRenderer::new`] but with a caller-provided [`TextSystem`]
    /// (e.g. one loaded with fixed fonts for golden tests).
    pub fn with_text_system(width: u32, height: u32, text: TextSystem) -> Self {
        Self {
            width,
            height,
            background: Color::hex(0xFFFFFF),
            images: HashMap::new(),
            next_id: 1,
            text,
            last: None,
        }
    }

    /// Set the colour the target is cleared to before drawing.
    pub fn set_background(&mut self, color: Color) {
        self.background = color;
    }

    /// Access the text system (to check font availability).
    pub fn text_system(&mut self) -> &mut TextSystem {
        &mut self.text
    }

    /// A measurement handle sharing this renderer's fonts.
    pub fn text_measurer(&self) -> crate::TextMeasurer {
        self.text.measurer()
    }

    /// The image produced by the most recent [`Renderer::render`] call.
    pub fn last_image(&self) -> Option<&Image> {
        self.last.as_ref()
    }
}

impl Renderer for CpuRenderer {
    fn render(&mut self, list: &DrawList, scale_factor: f32) {
        let img = self.render_to_image(list, self.width, self.height, scale_factor);
        self.last = Some(img);
    }

    fn register_image(&mut self, image: &Image) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.images.insert(
            id,
            Stored {
                width: image.width,
                height: image.height,
                rgba: image.premultiplied(),
            },
        );
        id
    }

    fn unregister_image(&mut self, id: u64) {
        self.images.remove(&id);
    }
}

impl HeadlessRenderer for CpuRenderer {
    fn render_to_image(
        &mut self,
        list: &DrawList,
        width: u32,
        height: u32,
        scale_factor: f32,
    ) -> Image {
        let prims = flatten(list, scale_factor, width, height, &mut self.text);
        let bg = self.background;
        let clear = [bg.r * bg.a, bg.g * bg.a, bg.b * bg.a, bg.a];
        let mut buf = vec![clear; width as usize * height as usize];
        for p in &prims {
            self.raster(p, &mut buf, width, height);
        }
        let mut rgba = Vec::with_capacity(buf.len() * 4);
        for px in buf {
            let a = px[3];
            let inv = if a > 0.0 { 1.0 / a } else { 0.0 };
            for c in &px[..3] {
                rgba.push(to_u8(c * inv));
            }
            rgba.push(to_u8(a));
        }
        Image {
            width,
            height,
            rgba,
        }
    }
}

fn to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn premul(c: [f32; 4]) -> [f32; 4] {
    [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]
}

fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Signed distance to a rounded rectangle `[x, y, w, h]`.
fn sd_rrect(p: (f32, f32), rect: [f32; 4], r: f32) -> f32 {
    let hx = rect[2] * 0.5;
    let hy = rect[3] * 0.5;
    let cx = rect[0] + hx;
    let cy = rect[1] + hy;
    let rr = r.min(hx.min(hy)).max(0.0);
    let qx = (p.0 - cx).abs() - hx + rr;
    let qy = (p.1 - cy).abs() - hy + rr;
    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    outside + qx.max(qy).min(0.0) - rr
}

fn cover(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

/// Bilinear sample with clamp-to-edge at normalised `(u, v)`; premultiplied.
fn sample(rgba: &[u8], w: u32, h: u32, u: f32, v: f32) -> [f32; 4] {
    let fx = u * w as f32 - 0.5;
    let fy = v * h as f32 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let fetch = |x: f32, y: f32| -> [f32; 4] {
        let xi = (x as i32).clamp(0, w as i32 - 1) as usize;
        let yi = (y as i32).clamp(0, h as i32 - 1) as usize;
        let i = (yi * w as usize + xi) * 4;
        [
            rgba[i] as f32 / 255.0,
            rgba[i + 1] as f32 / 255.0,
            rgba[i + 2] as f32 / 255.0,
            rgba[i + 3] as f32 / 255.0,
        ]
    };
    let (a, b, c, d) = (
        fetch(x0, y0),
        fetch(x0 + 1.0, y0),
        fetch(x0, y0 + 1.0),
        fetch(x0 + 1.0, y0 + 1.0),
    );
    let mut o = [0.0; 4];
    for k in 0..4 {
        let top = a[k] + (b[k] - a[k]) * tx;
        let bot = c[k] + (d[k] - c[k]) * tx;
        o[k] = top + (bot - top) * ty;
    }
    o
}

impl CpuRenderer {
    fn raster(&mut self, p: &Prim, buf: &mut [[f32; 4]], width: u32, height: u32) {
        // Resolve texel source up front.
        let glyph = match p.source {
            Source::Glyph(k) => self.text.bitmap(k),
            _ => None,
        };
        let tex: Option<(&[u8], u32, u32)> = match p.source {
            Source::None => None,
            Source::Glyph(_) => glyph.as_ref().map(|g| (&g.rgba[..], g.width, g.height)),
            Source::Image(id) => self
                .images
                .get(&id)
                .map(|s| (&s.rgba[..], s.width, s.height)),
        };
        if p.source != Source::None && tex.is_none() {
            return;
        }
        let [rx, ry, rw, rh] = p.rect;
        let x0 = ((rx - p.pad).floor() as i32).max(p.clip[0]).max(0);
        let y0 = ((ry - p.pad).floor() as i32).max(p.clip[1]).max(0);
        let x1 = ((rx + rw + p.pad).ceil() as i32)
            .min(p.clip[2])
            .min(width as i32);
        let y1 = ((ry + rh + p.pad).ceil() as i32)
            .min(p.clip[3])
            .min(height as i32);
        let fill = premul(p.color);
        let border = premul(p.border_color);
        for y in y0..y1 {
            for x in x0..x1 {
                let pt = (x as f32 + 0.5, y as f32 + 0.5);
                let src: [f32; 4] = match p.kind {
                    Kind::Shadow => {
                        let sd = sd_rrect(pt, p.rect, p.radius);
                        let a = if p.blur > 0.0 {
                            1.0 - smooth(((sd + p.blur * 0.5) / p.blur).clamp(0.0, 1.0))
                        } else {
                            cover(sd)
                        };
                        fill.map(|c| c * a)
                    }
                    Kind::Quad => {
                        let outer = cover(sd_rrect(pt, p.rect, p.radius));
                        let inner = if p.border_w > 0.0 {
                            let w = p.border_w;
                            let r = [
                                rx + w,
                                ry + w,
                                (rw - 2.0 * w).max(0.0),
                                (rh - 2.0 * w).max(0.0),
                            ];
                            cover(sd_rrect(pt, r, (p.radius - w).max(0.0)))
                        } else {
                            1.0
                        };
                        let mut o = [0.0; 4];
                        for k in 0..4 {
                            o[k] = outer * (border[k] + (fill[k] - border[k]) * inner);
                        }
                        o
                    }
                    Kind::Mask => {
                        let Some((d, w, h)) = tex else { continue };
                        let u = (pt.0 - rx) / rw;
                        let v = (pt.1 - ry) / rh;
                        let a = sample(d, w, h, u, v)[3];
                        fill.map(|c| c * a)
                    }
                    Kind::Texture => {
                        let Some((d, w, h)) = tex else { continue };
                        let cov = cover(sd_rrect(pt, p.rect, 0.0));
                        let u = (pt.0 - rx) / rw;
                        let v = (pt.1 - ry) / rh;
                        sample(d, w, h, u, v).map(|c| c * p.color[3] * cov)
                    }
                };
                let dst = &mut buf[y as usize * width as usize + x as usize];
                let ia = 1.0 - src[3];
                for k in 0..4 {
                    dst[k] = src[k] + dst[k] * ia;
                }
            }
        }
    }
}
