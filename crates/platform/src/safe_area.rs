//! Safe-area insets: the parts of the window that system chrome (notch, status
//! bar, navigation bar, home indicator, rounded corners) may cover.
//!
//! All values are logical pixels. Desktop windows report zero. The pure
//! conversion helpers live here so they are testable on any OS; the per-OS
//! query is in `os/android.rs` and `os/ios.rs`.

/// Distances from each window edge that content should keep clear.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    /// Space at the top (status bar, notch).
    pub top: f32,
    /// Space at the left (landscape notch, rounded corners).
    pub left: f32,
    /// Space at the bottom (navigation bar, home indicator).
    pub bottom: f32,
    /// Space at the right.
    pub right: f32,
}

impl Insets {
    /// No insets.
    pub const ZERO: Insets = Insets {
        top: 0.0,
        left: 0.0,
        bottom: 0.0,
        right: 0.0,
    };

    /// True when every side is (almost) zero.
    pub fn is_zero(&self) -> bool {
        self.top.abs() < 0.01
            && self.left.abs() < 0.01
            && self.bottom.abs() < 0.01
            && self.right.abs() < 0.01
    }

    /// Insets of the window (physical pixels, `win_w` x `win_h`) that lie
    /// outside `content` = `(left, top, right, bottom)` in physical pixels,
    /// converted to logical pixels with `scale`. This is how Android reports
    /// the area its system bars leave free. The rectangle is clamped to the
    /// window, so a stale rect from before a rotation cannot give negatives.
    pub fn from_content_rect(
        win_w: u32,
        win_h: u32,
        content: (i32, i32, i32, i32),
        scale: f64,
    ) -> Insets {
        let (l, t, r, b) = content;
        if r <= l || b <= t || scale <= 0.0 {
            return Insets::ZERO;
        }
        let px = |v: i64| (v.max(0) as f64 / scale) as f32;
        Insets {
            left: px(l as i64),
            top: px(t as i64),
            right: px(win_w as i64 - r as i64),
            bottom: px(win_h as i64 - b as i64),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_rect_to_insets() {
        // 1080x2400 window, 3x scale; 72px status bar, 144px nav bar.
        let i = Insets::from_content_rect(1080, 2400, (0, 72, 1080, 2256), 3.0);
        assert_eq!(
            i,
            Insets {
                top: 24.0,
                left: 0.0,
                right: 0.0,
                bottom: 48.0
            }
        );
    }

    #[test]
    fn stale_or_empty_rect_is_zero() {
        assert!(Insets::from_content_rect(100, 100, (0, 0, 0, 0), 1.0).is_zero());
        assert!(Insets::from_content_rect(100, 100, (0, 0, 100, 100), 0.0).is_zero());
    }

    #[test]
    fn rect_larger_than_window_never_goes_negative() {
        let i = Insets::from_content_rect(100, 100, (-5, -5, 120, 120), 1.0);
        assert_eq!(i, Insets::ZERO);
    }
}
