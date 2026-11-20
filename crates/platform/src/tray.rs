//! System tray model and backend trait.
//!
//! Status: contract only. No native tray backend is implemented yet (see
//! `DECISIONS.md`); [`UnsupportedTray`] reports that honestly.

use crate::{menu::Menu, PlatformError};

/// Tray icon description.
#[derive(Debug, Clone, Default)]
pub struct TraySettings {
    /// Tooltip text.
    pub tooltip: String,
    /// RGBA8 icon pixels, row-major.
    pub icon_rgba: Vec<u8>,
    /// Icon width in pixels.
    pub icon_width: u32,
    /// Icon height in pixels.
    pub icon_height: u32,
    /// Menu shown when the icon is clicked.
    pub menu: Option<Menu>,
}

impl TraySettings {
    /// True when the icon buffer matches its stated dimensions.
    pub fn icon_valid(&self) -> bool {
        self.icon_rgba.len() == (self.icon_width as usize) * (self.icon_height as usize) * 4
    }
}

/// Creates the OS tray icon.
pub trait TrayBackend {
    /// Show the tray icon.
    fn show(&mut self, settings: &TraySettings) -> Result<(), PlatformError>;
    /// Remove it.
    fn hide(&mut self);
}

/// Placeholder backend.
pub struct UnsupportedTray;

impl TrayBackend for UnsupportedTray {
    fn show(&mut self, _: &TraySettings) -> Result<(), PlatformError> {
        Err(PlatformError("native tray not implemented yet".into()))
    }
    fn hide(&mut self) {}
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

/// Encode RGBA8 pixels as an uncompressed PNG. Used by backends whose icon API
/// takes encoded image data (macOS `NSImage`). Size is irrelevant for tray icons.
#[allow(dead_code)] // only called by the macOS backend
pub(crate) fn encode_png(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::with_capacity(rgba.len() + h as usize);
    for row in rgba.chunks((w as usize * 4).max(1)) {
        raw.push(0); // filter: none
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    while let Some(b) = blocks.next() {
        z.push(u8::from(blocks.peek().is_none()));
        z.extend((b.len() as u16).to_le_bytes());
        z.extend((!(b.len() as u16)).to_le_bytes());
        z.extend(b);
    }
    z.extend(adler32(&raw).to_be_bytes());
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_structure() {
        let png = encode_png(&[255, 0, 0, 255, 0, 255, 0, 255], 2, 1);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        // IEND chunk with its well-known CRC.
        assert_eq!(
            &png[png.len() - 12..],
            &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
        );
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn icon_validation() {
        let mut t = TraySettings {
            icon_width: 2,
            icon_height: 2,
            icon_rgba: vec![0; 16],
            ..Default::default()
        };
        assert!(t.icon_valid());
        t.icon_rgba.pop();
        assert!(!t.icon_valid());
    }
}
