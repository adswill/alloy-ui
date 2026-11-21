//! Streaming textures: a pixel buffer an app updates by rows or rectangles every frame
//! (a waterfall, a video frame, a spectrogram) and draws with
//! [`DrawCommand::StreamImage`](alloy_ui_core::DrawCommand::StreamImage).
//!
//! The pixels live in a process-wide registry, so app and widget code never needs a handle to
//! the renderer. Each renderer uploads only the rectangle that changed since its last frame.
//! Pixels are premultiplied RGBA8; for opaque content that is the same as straight RGBA.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Shared pixel store behind one [`StreamTexture`].
pub(crate) struct Shared {
    pub(crate) width: u32,
    pub(crate) height: u32,
    state: Mutex<State>,
}

struct State {
    rgba: Vec<u8>,
    /// Bumped on every write; a renderer compares it with the one it last uploaded.
    version: u64,
    /// Union of the rectangles written after version `dirty_since`: `[x0, y0, x1, y1]`.
    dirty: Option<[u32; 4]>,
    dirty_since: u64,
}

/// A snapshot a renderer takes of a stream before drawing.
pub(crate) struct Snapshot {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) version: u64,
    /// Region `[x0, y0, x1, y1]` to upload; `rgba` holds its rows tightly packed.
    pub(crate) rect: [u32; 4],
    pub(crate) rgba: Vec<u8>,
}

fn registry() -> &'static Mutex<HashMap<u64, Arc<Shared>>> {
    static REG: OnceLock<Mutex<HashMap<u64, Arc<Shared>>>> = OnceLock::new();
    REG.get_or_init(Default::default)
}

pub(crate) fn lookup(id: u64) -> Option<Arc<Shared>> {
    registry().lock().ok()?.get(&id).cloned()
}

impl Shared {
    /// The pixels a renderer that has uploaded up to version `seen` (0 = never) must upload now:
    /// the union of the rectangles written since, or the whole image when it cannot tell what
    /// changed (a new renderer, or a second window that missed an upload). `None` when it is
    /// already up to date.
    pub(crate) fn snapshot(&self, seen: u64) -> Option<Snapshot> {
        let mut st = self.state.lock().ok()?;
        if st.version == seen {
            return None;
        }
        let full = [0, 0, self.width, self.height];
        let rect = if seen == 0 || seen < st.dirty_since {
            full
        } else {
            st.dirty.unwrap_or(full)
        };
        let w = self.width as usize;
        let [x0, y0, x1, y1] = rect;
        let mut rgba = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
        for y in y0..y1 {
            let a = (y as usize * w + x0 as usize) * 4;
            let b = (y as usize * w + x1 as usize) * 4;
            rgba.extend_from_slice(&st.rgba[a..b]);
        }
        st.dirty = None;
        st.dirty_since = st.version;
        Some(Snapshot {
            width: self.width,
            height: self.height,
            version: st.version,
            rect,
            rgba,
        })
    }

    /// Copy of all pixels (the CPU rasteriser samples this).
    pub(crate) fn pixels(&self) -> Vec<u8> {
        self.state
            .lock()
            .map(|s| s.rgba.clone())
            .unwrap_or_default()
    }
}

/// A texture the app writes to and the renderer draws. Dropping it frees the pixels.
pub struct StreamTexture {
    id: u64,
    shared: Arc<Shared>,
}

impl StreamTexture {
    /// A `width` by `height` texture, transparent black.
    pub fn new(width: u32, height: u32) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let (width, height) = (width.max(1), height.max(1));
        let shared = Arc::new(Shared {
            width,
            height,
            state: Mutex::new(State {
                rgba: vec![0; width as usize * height as usize * 4],
                version: 1,
                dirty: Some([0, 0, width, height]),
                dirty_since: 0,
            }),
        });
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut r) = registry().lock() {
            r.insert(id, shared.clone());
        }
        Self { id, shared }
    }

    /// The id to draw with `DrawCommand::StreamImage`.
    pub fn id(&self) -> u64 {
        self.id
    }
    /// Width in pixels.
    pub fn width(&self) -> u32 {
        self.shared.width
    }
    /// Height in pixels.
    pub fn height(&self) -> u32 {
        self.shared.height
    }

    /// Overwrite the rectangle at (`x`, `y`) of size `w` by `h` with `rgba` (`w * h * 4` bytes,
    /// premultiplied). Parts outside the texture are ignored.
    pub fn write(&self, x: u32, y: u32, w: u32, h: u32, rgba: &[u8]) {
        let (tw, th) = (self.shared.width, self.shared.height);
        if w == 0 || h == 0 || x >= tw || y >= th || rgba.len() < w as usize * h as usize * 4 {
            return;
        }
        let cw = w.min(tw - x);
        let ch = h.min(th - y);
        let Ok(mut st) = self.shared.state.lock() else {
            return;
        };
        for row in 0..ch as usize {
            let src = row * w as usize * 4;
            let dst = ((y as usize + row) * tw as usize + x as usize) * 4;
            st.rgba[dst..dst + cw as usize * 4].copy_from_slice(&rgba[src..src + cw as usize * 4]);
        }
        let r = [x, y, x + cw, y + ch];
        st.dirty = Some(match st.dirty {
            Some(d) => [
                d[0].min(r[0]),
                d[1].min(r[1]),
                d[2].max(r[2]),
                d[3].max(r[3]),
            ],
            None => r,
        });
        st.version += 1;
    }
}

impl Drop for StreamTexture {
    fn drop(&mut self) {
        if let Ok(mut r) = registry().lock() {
            r.remove(&self.id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploads_only_what_changed() {
        let t = StreamTexture::new(8, 4);
        let sh = lookup(t.id()).unwrap();
        // A new renderer gets everything, then is up to date.
        let first = sh.snapshot(0).unwrap();
        assert_eq!(first.rect, [0, 0, 8, 4]);
        assert!(sh.snapshot(first.version).is_none());
        // Two row writes between frames upload their union, not the whole texture.
        t.write(0, 1, 8, 1, &[255; 32]);
        t.write(0, 3, 8, 1, &[255; 32]);
        let snap = sh.snapshot(first.version).unwrap();
        assert_eq!(snap.rect, [0, 1, 8, 4]);
        assert_eq!(snap.rgba.len(), 8 * 3 * 4);
        // A renderer that missed an upload (a second window) falls back to the whole texture.
        let late = sh.snapshot(first.version).unwrap();
        assert_eq!(late.rect, [0, 0, 8, 4]);
    }

    #[test]
    fn dropped_texture_is_gone() {
        let id = {
            let t = StreamTexture::new(2, 2);
            t.id()
        };
        assert!(lookup(id).is_none());
    }
}
