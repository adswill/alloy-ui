//! Golden-image test helpers. Usable from any crate's tests.
//!
//! `assert_golden(dir, name, &image, tol)` compares against `dir/name.png`.
//! Set `UPDATE_GOLDEN=1` to (re)write goldens instead of comparing. On mismatch four
//! files are written to `$ALLOY_DIFF_DIR` (default `$TMPDIR/alloy-golden`) and named in the
//! panic message: `<name>.expected.png`, `<name>.actual.png`, `<name>.diff.png` (the
//! differing pixels in red over a faded copy of the golden) and `<name>.compare.png`
//! (expected, actual and diff side by side). `alloy test` sets the directory for you.

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
        let out = std::env::var_os("ALLOY_DIFF_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("alloy-golden"));
        let report = write_failure_report(&out, name, actual, &expected, tol);
        panic!("golden `{name}` mismatch: {why}\n{report}");
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

/// Where the differing pixels are, as `(x0, y0, x1, y1)` inclusive, and how many there are.
/// `None` when the images differ in size or no pixel exceeds the tolerance.
pub fn diff_region(a: &Image, b: &Image, tol: Tolerance) -> Option<((u32, u32, u32, u32), usize)> {
    if a.width != b.width || a.height != b.height {
        return None;
    }
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    let mut n = 0;
    for (i, (pa, pb)) in a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .enumerate()
    {
        let d = pa
            .iter()
            .zip(pb)
            .map(|(x, y)| x.abs_diff(*y))
            .max()
            .unwrap_or(0);
        if d > tol.channel {
            let (x, y) = (i as u32 % a.width, i as u32 / a.width);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            n += 1;
        }
    }
    (n > 0).then_some(((x0, y0, x1, y1), n))
}

/// The golden faded towards white with every pixel that exceeds `tol` painted red, so a
/// reviewer sees at a glance what moved. Images of different sizes give a plain faded golden.
pub fn diff_image(actual: &Image, expected: &Image, tol: Tolerance) -> Image {
    let mut out = Image::filled(expected.width, expected.height, [255, 255, 255, 255]);
    let same = actual.width == expected.width && actual.height == expected.height;
    for (i, px) in out.rgba.chunks_exact_mut(4).enumerate() {
        let e = &expected.rgba[i * 4..i * 4 + 4];
        let bad = same
            && actual.rgba[i * 4..i * 4 + 4]
                .iter()
                .zip(e)
                .map(|(x, y)| x.abs_diff(*y))
                .max()
                .unwrap_or(0)
                > tol.channel;
        if bad {
            px.copy_from_slice(&[230, 30, 40, 255]);
        } else {
            for c in 0..3 {
                px[c] = ((e[c] as u16 + 2 * 255) / 3) as u8;
            }
        }
    }
    out
}

/// Three images side by side with a 6 px gutter: expected, actual, diff.
fn side_by_side(parts: &[&Image]) -> Image {
    let gutter = 6u32;
    let h = parts.iter().map(|p| p.height).max().unwrap_or(0);
    let w: u32 = parts.iter().map(|p| p.width).sum::<u32>() + gutter * (parts.len() as u32 - 1);
    let mut out = Image::filled(w, h, [128, 128, 128, 255]);
    let mut x0 = 0;
    for p in parts {
        for y in 0..p.height {
            let src = (y * p.width * 4) as usize;
            let dst = ((y * w + x0) * 4) as usize;
            out.rgba[dst..dst + p.width as usize * 4]
                .copy_from_slice(&p.rgba[src..src + p.width as usize * 4]);
        }
        x0 += p.width + gutter;
    }
    out
}

/// Write the four failure files and describe them; used by [`assert_golden`].
fn write_failure_report(
    dir: &Path,
    name: &str,
    actual: &Image,
    expected: &Image,
    tol: Tolerance,
) -> String {
    let _ = std::fs::create_dir_all(dir);
    let diff = diff_image(actual, expected, tol);
    let mut lines = Vec::new();
    let mut put = |suffix: &str, img: &Image| {
        let p = dir.join(format!("{name}.{suffix}.png"));
        if img
            .encode_png()
            .map(|b| std::fs::write(&p, b))
            .is_ok_and(|r| r.is_ok())
        {
            lines.push(format!("  {suffix:<9} {}", p.display()));
        }
    };
    put("expected", expected);
    put("actual", actual);
    put("diff", &diff);
    put("compare", &side_by_side(&[expected, actual, &diff]));
    let mut report = lines.join("\n");
    if let Some(((x0, y0, x1, y1), n)) = diff_region(actual, expected, tol) {
        report.push_str(&format!(
            "\n  {n} pixels differ, inside x {x0}..{x1}, y {y0}..{y1} (device pixels)"
        ));
    }
    report.push_str("\n  accept the new look with UPDATE_GOLDEN=1 (or `alloy test --update`)");
    report
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

#[cfg(test)]
mod report_tests {
    use super::*;

    #[test]
    fn diff_marks_changed_pixels_and_reports_the_region() {
        let a = Image::filled(8, 6, [10, 10, 10, 255]);
        let mut b = a.clone();
        let i = (3 * 8 + 5) * 4;
        b.rgba[i..i + 3].copy_from_slice(&[200, 10, 10]);
        let tol = Tolerance::CPU;
        assert_eq!(diff_region(&b, &a, tol), Some(((5, 3, 5, 3), 1)));
        assert_eq!(diff_region(&a, &a, tol), None);
        let d = diff_image(&b, &a, tol);
        assert_eq!(d.pixel(5, 3), [230, 30, 40, 255]);
        assert_ne!(d.pixel(0, 0), [230, 30, 40, 255]);
    }

    #[test]
    fn failure_writes_expected_actual_diff_and_compare() {
        let dir = std::env::temp_dir().join(format!("alloy-diff-test-{}", std::process::id()));
        let a = Image::filled(4, 4, [0, 0, 0, 255]);
        let b = Image::filled(4, 4, [255, 255, 255, 255]);
        let msg = write_failure_report(&dir, "case", &a, &b, Tolerance::CPU);
        for part in ["expected", "actual", "diff", "compare"] {
            assert!(dir.join(format!("case.{part}.png")).exists(), "{part}");
        }
        assert!(msg.contains("16 pixels differ") && msg.contains("UPDATE_GOLDEN"));
        let cmp = Image::decode_png(&std::fs::read(dir.join("case.compare.png")).unwrap()).unwrap();
        assert_eq!((cmp.width, cmp.height), (4 * 3 + 12, 4));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
