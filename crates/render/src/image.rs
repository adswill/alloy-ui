//! CPU-side image type and PNG helpers.

use crate::error::RenderError;

/// RGBA8 pixels, row-major, straight (non-premultiplied) sRGB-encoded values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Image {
    /// Create an image filled with a single RGBA colour.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let mut data = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width as usize * height as usize) {
            data.extend_from_slice(&rgba);
        }
        Self {
            width,
            height,
            rgba: data,
        }
    }

    /// The RGBA value of pixel `(x, y)`. Panics when out of bounds.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    /// Encode as an 8-bit RGBA PNG.
    pub fn encode_png(&self) -> Result<Vec<u8>, RenderError> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc
                .write_header()
                .map_err(|e| RenderError::Codec(e.to_string()))?;
            w.write_image_data(&self.rgba)
                .map_err(|e| RenderError::Codec(e.to_string()))?;
        }
        Ok(out)
    }

    /// Decode a PNG (any colour type, expanded to 8-bit RGBA).
    pub fn decode_png(bytes: &[u8]) -> Result<Self, RenderError> {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec
            .read_info()
            .map_err(|e| RenderError::Codec(e.to_string()))?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buf)
            .map_err(|e| RenderError::Codec(e.to_string()))?;
        buf.truncate(info.buffer_size());
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf,
            png::ColorType::Rgb => buf
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect(),
            png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::GrayscaleAlpha => buf
                .chunks_exact(2)
                .flat_map(|c| [c[0], c[0], c[0], c[1]])
                .collect(),
            png::ColorType::Indexed => {
                return Err(RenderError::Codec("unexpanded indexed PNG".into()))
            }
        };
        Ok(Self {
            width: info.width,
            height: info.height,
            rgba,
        })
    }

    /// Premultiplied copy of the pixel data (used for texture upload/sampling).
    pub(crate) fn premultiplied(&self) -> Vec<u8> {
        self.rgba
            .chunks_exact(4)
            .flat_map(|p| {
                let a = p[3] as u32;
                let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
                [m(p[0]), m(p[1]), m(p[2]), p[3]]
            })
            .collect()
    }
}
