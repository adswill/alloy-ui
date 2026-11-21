//! Golden-image test helpers. Usable from any crate's tests.
//!
//! `assert_golden(dir, name, &image, tol)` compares against `dir/name.png`.
//! Set `UPDATE_GOLDEN=1` to (re)write goldens instead of comparing. On mismatch
//! the actual image is written to `$TMPDIR/alloy-golden/<name>.actual.png`.

use std::path::{Path, PathBuf};

use crate::image::Image;
#[cfg(feature = "gpu")]
use crate::GpuHeadlessRenderer;
use crate::{CpuRenderer, HeadlessRenderer};

/// Comparison tolerance.
#[derive(Debug, Clone, Copy)]
pub struct Tolerance {
    /// Largest allowed per-channel absolute difference for a pixel to count as equal.
    pub channel: u8,
    /// Largest allowed fraction of pixels exceeding `channel`.
    pub bad_fraction: f32,
}

impl Tolerance {
    /// Tight tolerance for the deterministic CPU rasteriser.
    pub const CPU: Self = Self {
        channel: 1,
        bad_fraction: 0.0,
    };
    /// Looser tolerance for comparing GPU output with CPU-made goldens.
    pub const GPU: Self = Self {
        channel: 6,
        bad_fraction: 0.005,
    };
}

/// Result of comparing two images.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Diff {
    /// Largest per-channel difference seen.
    pub max_channel: u8,
    /// Fraction of pixels whose difference exceeded the tolerance channel value.
    pub bad_fraction: f32,
}

/// Compare two images of equal size. Returns `None` when sizes differ.
pub fn compare(a: &Image, b: &Image, tol: Tolerance) -> Option<Diff> {
    if a.width != b.width || a.height != b.height {
        return None;
    }
    let mut max = 0u8;
    let mut bad = 0usize;
    for (pa, pb) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        let d = pa
            .iter()
            .zip(pb)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        max = max.max(d);
        if d > tol.channel {
            bad += 1;
        }
    }
    let total = (a.width as usize * a.height as usize).max(1);
    Some(Diff {
        max_channel: max,
        bad_fraction: bad as f32 / total as f32,
    })
}

/// Directory holding this crate's goldens (`crates/render/tests/golden`).
pub fn default_golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Assert `actual` matches `dir/name.png` within `tol`, or write it when
/// `UPDATE_GOLDEN=1` (or the golden does not exist and `UPDATE_GOLDEN` is set).
///
/// # Panics
/// On mismatch or a missing golden.
pub fn assert_golden(dir: &Path, name: &str, actual: &Image, tol: Tolerance) {
    let path = dir.join(format!("{name}.png"));
    let update = std::env::var("UPDATE_GOLDEN").is_ok_and(|v| !v.is_empty() && v != "0");
    if update {
        std::fs::create_dir_all(dir).expect("create golden dir");
        std::fs::write(&path, actual.encode_png().expect("encode png")).expect("write golden");
        eprintln!("updated golden {}", path.display());
        return;
    }
    let bytes = std::fs::read(&path).unwrap_or_else(|_| {
        panic!(
            "missing golden {} (run with UPDATE_GOLDEN=1 to create it)",
            path.display()
        )
    });
    let expected = Image::decode_png(&bytes).expect("decode golden");
    let fail = |why: String| -> ! {
        let out = std::env::temp_dir().join("alloy-golden");
        let _ = std::fs::create_dir_all(&out);
        let p = out.join(format!("{name}.actual.png"));
        if let Ok(b) = actual.encode_png() {
            let _ = std::fs::write(&p, b);
        }
        panic!(
            "golden `{name}` mismatch: {why}; actual written to {}",
            p.display()
        );
    };
    match compare(actual, &expected, tol) {
        None => fail(format!(
            "size {}x{} vs golden {}x{}",
            actual.width, actual.height, expected.width, expected.height
        )),
        Some(d) if d.bad_fraction > tol.bad_fraction => fail(format!(
            "{:.4}% pixels differ (max channel diff {})",
            d.bad_fraction * 100.0,
            d.max_channel
        )),
        Some(_) => {}
    }
}

/// A headless renderer for tests: GPU when an adapter exists, else `None`.
/// Logs a skip message on stderr when no adapter is found.
#[cfg(feature = "gpu")]
pub fn gpu_or_skip(width: u32, height: u32) -> Option<GpuHeadlessRenderer> {
    match GpuHeadlessRenderer::try_new(width, height) {
        Ok(r) => {
            eprintln!("GPU adapter: {}", r.adapter_name());
            Some(r)
        }
        Err(e) => {
            eprintln!("SKIP: no graphics adapter available ({e})");
            None
        }
    }
}

/// The CPU reference renderer, boxed as a [`HeadlessRenderer`].
pub fn cpu(width: u32, height: u32) -> Box<dyn HeadlessRenderer> {
    Box::new(CpuRenderer::new(width, height))
}
