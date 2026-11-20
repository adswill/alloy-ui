//! Text shaping and glyph rasterisation (cosmic-text), shared by the GPU and
//! CPU renderers so both place identical glyph bitmaps.

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, FontSystem, Metrics, Shaping, SwashCache, SwashContent,
};

/// Line height as a multiple of font size.
const LINE_HEIGHT: f32 = 1.25;

/// A rasterised glyph: premultiplied RGBA8, plus placement relative to the pen.
#[derive(Debug)]
pub struct GlyphBitmap {
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Horizontal offset from the pen position to the bitmap's left edge.
    pub left: i32,
    /// Vertical offset from the baseline up to the bitmap's top edge.
    pub top: i32,
    /// True for colour glyphs (emoji); false for coverage masks.
    pub color: bool,
    /// Premultiplied RGBA. For masks the alpha channel holds coverage.
    pub rgba: Vec<u8>,
}

/// A glyph positioned in physical pixels (pen position on the baseline).
#[derive(Debug, Clone, Copy)]
pub struct PlacedGlyph {
    /// Cache key identifying the glyph image.
    pub key: CacheKey,
    /// Pen x in physical pixels.
    pub x: i32,
    /// Baseline y in physical pixels.
    pub y: i32,
}

/// Owns the font database, shaper and glyph bitmap cache.
pub struct TextSystem {
    fonts: FontSystem,
    swash: SwashCache,
    bitmaps: HashMap<CacheKey, Option<Arc<GlyphBitmap>>>,
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    /// Create a text system that loads the operating system's fonts.
    pub fn new() -> Self {
        Self::from_font_system(FontSystem::new())
    }

    /// Create a text system using only the given font files (TTF/OTF bytes).
    /// Use this for deterministic output, e.g. golden tests.
    pub fn with_fonts(fonts: impl IntoIterator<Item = Vec<u8>>) -> Self {
        let sources = fonts
            .into_iter()
            .map(|d| cosmic_text::fontdb::Source::Binary(Arc::new(d)));
        Self::from_font_system(FontSystem::new_with_fonts(sources))
    }

    fn from_font_system(fonts: FontSystem) -> Self {
        Self {
            fonts,
            swash: SwashCache::new(),
            bitmaps: HashMap::new(),
        }
    }

    /// Number of font faces available. Zero means text cannot be drawn.
    pub fn face_count(&self) -> usize {
        self.fonts.db().faces().count()
    }

    /// Shape and lay out `content` at `size_px` (physical pixels), with the
    /// text box's top-left at `origin` (physical pixels).
    pub fn layout(
        &mut self,
        content: &str,
        size_px: f32,
        max_width_px: Option<f32>,
        origin: (f32, f32),
    ) -> Vec<PlacedGlyph> {
        if size_px <= 0.0 || content.is_empty() || self.face_count() == 0 {
            return Vec::new();
        }
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size_px, size_px * LINE_HEIGHT));
        buffer.set_size(&mut self.fonts, max_width_px, None);
        buffer.set_text(&mut self.fonts, content, &Attrs::new(), Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.fonts, false);
        let ox = origin.0.round() as i32;
        let oy = origin.1.round() as i32;
        let mut out = Vec::new();
        for run in buffer.layout_runs() {
            let line_y = run.line_y.round() as i32;
            for g in run.glyphs {
                let p = g.physical((0.0, 0.0), 1.0);
                out.push(PlacedGlyph {
                    key: p.cache_key,
                    x: ox + p.x,
                    y: oy + line_y + p.y,
                });
            }
        }
        out
    }

    /// The rasterised bitmap for a glyph, if it has one (spaces do not).
    pub fn bitmap(&mut self, key: CacheKey) -> Option<Arc<GlyphBitmap>> {
        if let Some(b) = self.bitmaps.get(&key) {
            return b.clone();
        }
        let made = self
            .swash
            .get_image(&mut self.fonts, key)
            .as_ref()
            .filter(|img| img.placement.width > 0 && img.placement.height > 0)
            .map(|img| {
                let (w, h) = (img.placement.width, img.placement.height);
                let (rgba, color) = match img.content {
                    SwashContent::Mask => (
                        img.data.iter().flat_map(|&a| [a, a, a, a]).collect(),
                        false,
                    ),
                    SwashContent::Color => (
                        img.data
                            .chunks_exact(4)
                            .flat_map(|p| {
                                let a = p[3] as u32;
                                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                                [m(p[0]), m(p[1]), m(p[2]), p[3]]
                            })
                            .collect(),
                        true,
                    ),
                    SwashContent::SubpixelMask => (
                        img.data
                            .chunks_exact(3)
                            .flat_map(|p| {
                                let a = ((p[0] as u32 + p[1] as u32 + p[2] as u32) / 3) as u8;
                                [a, a, a, a]
                            })
                            .collect(),
                        false,
                    ),
                };
                Arc::new(GlyphBitmap {
                    width: w,
                    height: h,
                    left: img.placement.left,
                    top: img.placement.top,
                    color,
                    rgba,
                })
            });
        self.bitmaps.insert(key, made.clone());
        made
    }
}
