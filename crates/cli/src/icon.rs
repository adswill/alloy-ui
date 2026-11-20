//! Icon handling: read PNG/ICNS/ICO inputs and synthesise the formats each
//! platform needs. PNG data is embedded as-is into `.icns` and `.ico`
//! containers, so no image decoding is required.

use crate::error::{io, Error, Result};
use std::path::{Path, PathBuf};

/// A square PNG icon.
#[derive(Debug, Clone)]
pub struct PngIcon {
    /// Edge length in pixels.
    pub size: u32,
    /// Raw PNG bytes.
    pub data: Vec<u8>,
}

/// The icons an app provides, grouped by kind.
#[derive(Debug, Clone, Default)]
pub struct IconSet {
    /// PNG icons, ascending by size.
    pub pngs: Vec<PngIcon>,
    /// A ready-made `.icns`, if one was supplied.
    pub icns: Option<Vec<u8>>,
    /// A ready-made `.ico`, if one was supplied.
    pub ico: Option<Vec<u8>>,
}

/// Width and height from a PNG header, or `None` if `data` is not a PNG.
pub fn png_size(data: &[u8]) -> Option<(u32, u32)> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if data.len() < 24 || data[..8] != SIG || &data[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes(data[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(data[20..24].try_into().ok()?);
    Some((w, h))
}

/// `.icns` PNG-payload entry types by pixel size.
const ICNS_TYPES: [(u32, &[u8; 4]); 7] = [
    (16, b"icp4"),
    (32, b"icp5"),
    (64, b"icp6"),
    (128, b"ic07"),
    (256, b"ic08"),
    (512, b"ic09"),
    (1024, b"ic10"),
];

impl IconSet {
    /// Load icons from `paths`, dispatching on file extension.
    pub fn load(paths: &[PathBuf]) -> Result<Self> {
        let mut set = IconSet::default();
        for p in paths {
            let data = std::fs::read(p).map_err(io(format!("reading icon {}", p.display())))?;
            match ext(p).as_str() {
                "png" => {
                    let (w, h) = png_size(&data).ok_or_else(|| {
                        Error::Config(format!("{} is not a valid PNG", p.display()))
                    })?;
                    if w != h {
                        return Err(Error::Config(format!(
                            "icon {} is {w}x{h}; icons must be square",
                            p.display()
                        )));
                    }
                    if !set.pngs.iter().any(|i| i.size == w) {
                        set.pngs.push(PngIcon { size: w, data });
                    }
                }
                "icns" => {
                    if !data.starts_with(b"icns") {
                        return Err(Error::Config(format!("{} is not an .icns", p.display())));
                    }
                    set.icns = Some(data);
                }
                "ico" => {
                    if data.len() < 6 || data[..4] != [0, 0, 1, 0] {
                        return Err(Error::Config(format!("{} is not an .ico", p.display())));
                    }
                    set.ico = Some(data);
                }
                other => {
                    return Err(Error::Config(format!(
                        "unsupported icon type `.{other}` for {} (use png, icns or ico)",
                        p.display()
                    )))
                }
            }
        }
        set.pngs.sort_by_key(|i| i.size);
        Ok(set)
    }

    /// True when no icon of any kind was provided.
    pub fn is_empty(&self) -> bool {
        self.pngs.is_empty() && self.icns.is_none() && self.ico.is_none()
    }

    /// The largest PNG, used for AppImage and generic fallbacks.
    pub fn largest_png(&self) -> Option<&PngIcon> {
        self.pngs.last()
    }

    /// `.icns` bytes: the supplied file, else built from PNGs whose size is
    /// one of 16, 32, 64, 128, 256, 512 or 1024.
    pub fn icns_bytes(&self) -> Option<Vec<u8>> {
        if let Some(i) = &self.icns {
            return Some(i.clone());
        }
        let entries: Vec<(&[u8; 4], &PngIcon)> = self
            .pngs
            .iter()
            .filter_map(|p| {
                ICNS_TYPES
                    .iter()
                    .find(|(s, _)| *s == p.size)
                    .map(|(_, t)| (*t, p))
            })
            .collect();
        if entries.is_empty() {
            return None;
        }
        let total: usize = 8 + entries.iter().map(|(_, p)| 8 + p.data.len()).sum::<usize>();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"icns");
        out.extend_from_slice(&(total as u32).to_be_bytes());
        for (ty, p) in entries {
            out.extend_from_slice(ty);
            out.extend_from_slice(&((8 + p.data.len()) as u32).to_be_bytes());
            out.extend_from_slice(&p.data);
        }
        Some(out)
    }

    /// `.ico` bytes: the supplied file, else built from PNGs up to 256 px.
    pub fn ico_bytes(&self) -> Option<Vec<u8>> {
        if let Some(i) = &self.ico {
            return Some(i.clone());
        }
        let entries: Vec<&PngIcon> = self.pngs.iter().filter(|p| p.size <= 256).collect();
        if entries.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        out.extend_from_slice(&[0, 0, 1, 0]);
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        let mut offset = 6 + 16 * entries.len() as u32;
        for p in &entries {
            let dim = if p.size == 256 { 0 } else { p.size as u8 };
            out.extend_from_slice(&[dim, dim, 0, 0]);
            out.extend_from_slice(&1u16.to_le_bytes()); // planes
            out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
            out.extend_from_slice(&(p.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&offset.to_le_bytes());
            offset += p.data.len() as u32;
        }
        for p in entries {
            out.extend_from_slice(&p.data);
        }
        Some(out)
    }
}

fn ext(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn test_png(size: u32) -> Vec<u8> {
    // Header-only PNG: enough for size parsing and container tests.
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13];
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&size.to_be_bytes());
    v.extend_from_slice(&size.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(sizes: &[u32]) -> IconSet {
        IconSet {
            pngs: sizes
                .iter()
                .map(|&s| PngIcon {
                    size: s,
                    data: test_png(s),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn reads_png_size() {
        assert_eq!(png_size(&test_png(48)), Some((48, 48)));
        assert_eq!(png_size(b"not a png at all, definitely not"), None);
        assert_eq!(png_size(&[]), None);
    }

    #[test]
    fn icns_layout() {
        let s = set(&[128, 256]);
        let b = s.icns_bytes().unwrap();
        assert_eq!(&b[..4], b"icns");
        assert_eq!(
            u32::from_be_bytes(b[4..8].try_into().unwrap()) as usize,
            b.len()
        );
        assert_eq!(&b[8..12], b"ic07");
        let first_len = u32::from_be_bytes(b[12..16].try_into().unwrap()) as usize;
        assert_eq!(&b[8 + first_len..12 + first_len], b"ic08");
    }

    #[test]
    fn icns_skips_unsupported_sizes() {
        assert!(set(&[48]).icns_bytes().is_none());
        assert!(set(&[48, 512]).icns_bytes().is_some());
    }

    #[test]
    fn ico_layout() {
        let s = set(&[32, 256, 512]);
        let b = s.ico_bytes().unwrap();
        assert_eq!(&b[..4], &[0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([b[4], b[5]]), 2); // 512 excluded
        assert_eq!(b[6], 32);
        assert_eq!(b[6 + 16], 0); // 256 encoded as 0
        let off = u32::from_le_bytes(b[6 + 12..6 + 16].try_into().unwrap()) as usize;
        assert_eq!(off, 6 + 32);
        assert_eq!(png_size(&b[off..]), Some((32, 32)));
    }

    #[test]
    fn load_rejects_bad_inputs() {
        let dir = std::env::temp_dir().join(format!("alloy-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.png");
        std::fs::write(&bad, b"junk").unwrap();
        assert!(IconSet::load(&[bad]).is_err());
        let rect = dir.join("rect.png");
        let mut p = test_png(16);
        p[20..24].copy_from_slice(&32u32.to_be_bytes());
        std::fs::write(&rect, p).unwrap();
        assert!(IconSet::load(&[rect]).is_err());
        let gif = dir.join("x.gif");
        std::fs::write(&gif, b"GIF89a").unwrap();
        assert!(IconSet::load(&[gif]).is_err());
        let ok = dir.join("ok.png");
        std::fs::write(&ok, test_png(64)).unwrap();
        let s = IconSet::load(&[ok.clone(), ok]).unwrap();
        assert_eq!(s.pngs.len(), 1);
        std::fs::remove_dir_all(dir).ok();
    }
}
