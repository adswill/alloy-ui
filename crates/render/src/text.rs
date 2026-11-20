//! Text shaping and glyph rasterisation (cosmic-text), shared by the GPU and
//! CPU renderers so both place identical glyph bitmaps.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use alloy_ui_core::Size;
use cosmic_text::{
    Attrs, Buffer, CacheKey, FontSystem, Metrics, Shaping, SwashCache, SwashContent, Weight,
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

struct Inner {
    fonts: FontSystem,
    swash: SwashCache,
    bitmaps: HashMap<CacheKey, Option<Arc<GlyphBitmap>>>,
}

/// Owns the font database, shaper and glyph bitmap cache. Cloning is cheap
/// and yields a handle to the same shared state, so a [`TextMeasurer`] and a
/// renderer use one font database.
#[derive(Clone)]
pub struct TextSystem {
    inner: Arc<Mutex<Inner>>,
}

/// Cloneable, thread-safe text measurement handle sharing the renderer's
/// fonts and shaper, so measured sizes match what is drawn.
#[derive(Clone)]
pub struct TextMeasurer {
    sys: TextSystem,
}

impl TextMeasurer {
    /// A measurer over a fresh system that loads the OS fonts.
    pub fn new() -> Self {
        Self {
            sys: TextSystem::new(),
        }
    }

    /// A measurer sharing `sys`'s fonts and caches.
    pub fn from_text_system(sys: &TextSystem) -> Self {
        Self { sys: sys.clone() }
    }

    /// Size in logical pixels of `text` at `size` (logical px) and `weight`
    /// (100..=900), wrapped at `max_width` if given. Height uses the default
    /// 1.25x line height per line.
    pub fn measure(&self, text: &str, size: f32, weight: u16, max_width: Option<f32>) -> Size {
        self.measure_with_line_height(text, size, weight, None, max_width)
    }

    /// As [`TextMeasurer::measure`] with an explicit line height (logical px).
    pub fn measure_with_line_height(
        &self,
        text: &str,
        size: f32,
        weight: u16,
        line_height: Option<f32>,
        max_width: Option<f32>,
    ) -> Size {
        let (w, h) = self
            .sys
            .clone()
            .extent(text, size, weight, line_height, max_width);
        Size {
            width: w,
            height: h,
        }
    }
}

impl Default for TextMeasurer {
    fn default() -> Self {
        Self::new()
    }
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
            inner: Arc::new(Mutex::new(Inner {
                fonts,
                swash: SwashCache::new(),
                bitmaps: HashMap::new(),
            })),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A measurement handle sharing this system's fonts.
    pub fn measurer(&self) -> TextMeasurer {
        TextMeasurer::from_text_system(self)
    }

    /// Logical extent `(width, height)` of laid-out text; see
    /// [`TextMeasurer::measure`].
    pub fn extent(
        &mut self,
        content: &str,
        size: f32,
        weight: u16,
        line_height: Option<f32>,
        max_width: Option<f32>,
    ) -> (f32, f32) {
        let mut g = self.lock();
        let inner = &mut *g;
        if size <= 0.0 || content.is_empty() || inner.fonts.db().faces().count() == 0 {
            return (0.0, 0.0);
        }
        let buffer = shaped(inner, content, size, weight, line_height, max_width);
        let (mut w, mut h) = (0.0f32, 0.0f32);
        for run in buffer.layout_runs() {
            w = w.max(run.line_w);
            h = h.max(run.line_top + run.line_height);
        }
        (w, h)
    }

    /// Number of font faces available. Zero means text cannot be drawn.
    pub fn face_count(&self) -> usize {
        self.lock().fonts.db().faces().count()
    }

    /// Shape and lay out `content` at `size_px` (physical pixels), with the
    /// text box's top-left at `origin` (physical pixels).
    pub fn layout(
        &mut self,
        content: &str,
        size_px: f32,
        weight: u16,
        line_height_px: Option<f32>,
        max_width_px: Option<f32>,
        origin: (f32, f32),
    ) -> Vec<PlacedGlyph> {
        let mut g = self.lock();
        let inner = &mut *g;
        if size_px <= 0.0 || content.is_empty() || inner.fonts.db().faces().count() == 0 {
            return Vec::new();
        }
        let buffer = shaped(
            inner,
            content,
            size_px,
            weight,
            line_height_px,
            max_width_px,
        );
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
        let mut g = self.lock();
        let Inner {
            fonts,
            swash,
            bitmaps,
        } = &mut *g;
        if let Some(b) = bitmaps.get(&key) {
            return b.clone();
        }
        let made = swash
            .get_image(fonts, key)
            .as_ref()
            .filter(|img| img.placement.width > 0 && img.placement.height > 0)
            .map(|img| {
                let (w, h) = (img.placement.width, img.placement.height);
                let (rgba, color) = match img.content {
                    SwashContent::Mask => {
                        (img.data.iter().flat_map(|&a| [a, a, a, a]).collect(), false)
                    }
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
        bitmaps.insert(key, made.clone());
        made
    }
}

fn shaped(
    inner: &mut Inner,
    content: &str,
    size: f32,
    weight: u16,
    line_height: Option<f32>,
    max_width: Option<f32>,
) -> Buffer {
    let lh = line_height
        .filter(|l| *l > 0.0)
        .unwrap_or(size * LINE_HEIGHT);
    let mut buffer = Buffer::new(&mut inner.fonts, Metrics::new(size, lh));
    buffer.set_size(&mut inner.fonts, max_width, None);
    let attrs = Attrs::new().weight(Weight(weight.clamp(1, 1000)));
    buffer.set_text(&mut inner.fonts, content, &attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut inner.fonts, false);
    buffer
}
