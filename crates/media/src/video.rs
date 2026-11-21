//! Video through the `ffmpeg` command-line tool.
//!
//! ffmpeg decodes, scales and resamples the frame rate; we read raw RGBA frames from its
//! standard output on a worker thread into a short queue. The picture for a time `t` is the
//! newest queued frame not later than `t`, uploaded to a [`PixelSurface`] only when it changes.
//! Seeking restarts ffmpeg at the new time with `-ss`, which lands on the exact frame.
//!
//! Spawning a process instead of linking libavcodec keeps the app binary free of codecs (and
//! their licences) and works the same on every OS; the cost is that the user must have ffmpeg
//! installed. [`ffmpeg_available`] says whether it is.

use crate::error::MediaError;
use alloy_ui_core::{Color, Point, Rectangle};
use alloy_ui_render::PixelSurface;
use alloy_ui_widgets::{Frame, MediaSource};
use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Duration;

/// Frames per second we ask ffmpeg to deliver.
const FPS: u64 = 30;
/// Widest picture we decode; wider sources are scaled down.
const MAX_WIDTH: u32 = 960;
/// Frames queued ahead of the clock.
const QUEUE: usize = 6;

/// True when `ffmpeg` and `ffprobe` run on this machine. The answer is cached.
pub fn ffmpeg_available() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        ["ffmpeg", "ffprobe"].iter().all(|t| {
            command(t)
                .arg("-version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
    })
}

fn command(tool: &str) -> Command {
    let mut c = Command::new(tool);
    c.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashes up beside the app.
        c.creation_flags(0x0800_0000);
    }
    c
}

struct Info {
    width: u32,
    height: u32,
    duration_ms: u64,
}

fn probe(path: &Path) -> Result<Info, MediaError> {
    if !ffmpeg_available() {
        return Err(MediaError::NoFfmpeg(
            "ffmpeg/ffprobe not found on PATH".into(),
        ));
    }
    let out = command("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height:stream_disposition=attached_pic:format=duration",
            "-of",
            "default=noprint_wrappers=1",
        ])
        .arg(path)
        .stderr(Stdio::null())
        .output()
        .map_err(|e| MediaError::NoFfmpeg(e.to_string()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (mut w, mut h, mut dur, mut cover) = (0u32, 0u32, 0.0f64, false);
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "width" => w = v.trim().parse().unwrap_or(0),
            "height" => h = v.trim().parse().unwrap_or(0),
            "duration" => dur = v.trim().parse().unwrap_or(0.0),
            k if k.ends_with("attached_pic") => cover = v.trim() == "1",
            _ => {}
        }
    }
    if w == 0 || h == 0 || cover {
        return Err(MediaError::Unsupported("no video stream".into()));
    }
    Ok(Info {
        width: w,
        height: h,
        duration_ms: (dur * 1000.0).max(0.0) as u64,
    })
}

/// One run of ffmpeg from `start_ms`, and the frames it has produced.
struct Session {
    start_ms: u64,
    frames: Mutex<VecDeque<(u64, Vec<u8>)>>,
    cv: Condvar,
    cancel: AtomicBool,
    done: AtomicBool,
    child: Mutex<Option<Child>>,
}

impl Session {
    fn start(path: &Path, start_ms: u64, w: u32, h: u32) -> Arc<Self> {
        let s = Arc::new(Self {
            start_ms,
            frames: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            cancel: AtomicBool::new(false),
            done: AtomicBool::new(false),
            child: Mutex::new(None),
        });
        let child = command("ffmpeg")
            .args(["-v", "error", "-ss"])
            .arg(format!("{:.3}", start_ms as f64 / 1000.0))
            .arg("-i")
            .arg(path)
            .args(["-an", "-sn", "-vf"])
            .arg(format!("scale={w}:{h}:flags=fast_bilinear,fps={FPS}"))
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                log::warn!("cannot start ffmpeg: {e}");
                s.done.store(true, Ordering::Relaxed);
                return s;
            }
        };
        let Some(mut stdout) = child.stdout.take() else {
            s.done.store(true, Ordering::Relaxed);
            return s;
        };
        *s.child.lock().unwrap_or_else(|e| e.into_inner()) = Some(child);
        let worker = s.clone();
        let size = (w as usize) * (h as usize) * 4;
        let spawned = std::thread::Builder::new()
            .name("alloy-video-read".into())
            .spawn(move || {
                let mut index = 0u64;
                loop {
                    let mut frame = vec![0u8; size];
                    if stdout.read_exact(&mut frame).is_err() {
                        break;
                    }
                    let t = worker.start_ms + index * 1000 / FPS;
                    index += 1;
                    let mut q = worker.frames.lock().unwrap_or_else(|e| e.into_inner());
                    while q.len() >= QUEUE && !worker.cancel.load(Ordering::Relaxed) {
                        q = worker
                            .cv
                            .wait_timeout(q, Duration::from_millis(50))
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                    if worker.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    q.push_back((t, frame));
                }
                worker.done.store(true, Ordering::Relaxed);
            });
        if spawned.is_err() {
            s.stop();
        }
        s
    }

    /// Ask the worker to finish and kill ffmpeg so its pipe closes.
    fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cv.notify_all();
        if let Some(mut c) = self.child.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

struct Playing {
    session: Arc<Session>,
    /// Time of the frame currently on the surface, if any.
    shown: Option<u64>,
}

/// A video file as a [`MediaSource`].
pub struct Video {
    path: PathBuf,
    info: Info,
    out: (u32, u32),
    surface: PixelSurface,
    now: Mutex<Playing>,
    #[cfg(feature = "audio")]
    sound: Option<crate::Sound>,
}

impl Video {
    /// Open a video file. Fails with [`MediaError::NoFfmpeg`] when ffmpeg is missing and with
    /// [`MediaError::Unsupported`] when the file has no picture.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MediaError> {
        let path = path.as_ref().to_path_buf();
        std::fs::metadata(&path).map_err(|e| MediaError::Io(path.display().to_string(), e))?;
        let info = probe(&path)?;
        let (mut w, mut h) = (info.width, info.height);
        if w > MAX_WIDTH {
            h = (h as u64 * MAX_WIDTH as u64 / w as u64) as u32;
            w = MAX_WIDTH;
        }
        let out = (w.max(2) & !1, h.max(2) & !1);
        let session = Session::start(&path, 0, out.0, out.1);
        Ok(Self {
            #[cfg(feature = "audio")]
            sound: crate::Sound::open(&path).ok(),
            surface: PixelSurface::new(out.0, out.1),
            now: Mutex::new(Playing {
                session,
                shown: None,
            }),
            out,
            info,
            path,
        })
    }

    /// Picture size as decoded (the source scaled to at most 960 wide).
    pub fn size(&self) -> (u32, u32) {
        self.out
    }

    /// Move the decoder to `t_ms`, or advance to the frame for it. Returns the frame time that is
    /// on the surface after the call.
    fn advance(&self, t_ms: u64) -> Option<u64> {
        let mut st = self.now.lock().unwrap_or_else(|e| e.into_inner());
        let newest = {
            let q = st.session.frames.lock().unwrap_or_else(|e| e.into_inner());
            q.back().map(|f| f.0)
        };
        let head = newest.or(st.shown).unwrap_or(st.session.start_ms);
        let before = t_ms + 150 < st.session.start_ms || st.shown.is_some_and(|s| t_ms + 150 < s);
        let far_ahead = t_ms > head + 2000 && !st.session.done.load(Ordering::Relaxed);
        let eof_gap = st.session.done.load(Ordering::Relaxed) && t_ms > head + 2000;
        if (before || far_ahead) && !eof_gap {
            st.session.stop();
            st.session = Session::start(&self.path, t_ms, self.out.0, self.out.1);
            st.shown = None;
        }
        let mut latest = None;
        {
            let mut q = st.session.frames.lock().unwrap_or_else(|e| e.into_inner());
            while q.front().is_some_and(|f| f.0 <= t_ms) {
                latest = q.pop_front();
            }
            if latest.is_none() && st.shown.is_none() {
                // Nothing at or before `t` yet: show the first frame as soon as there is one.
                if let Some(first) = q.front() {
                    if first.0 <= t_ms + 1000 {
                        latest = q.pop_front();
                    }
                }
            }
        }
        st.session.cv.notify_all();
        if let Some((t, rgba)) = latest {
            self.surface.update_opaque(self.out.0, self.out.1, &rgba);
            st.shown = Some(t);
        }
        st.shown
    }
}

impl MediaSource for Video {
    fn duration_ms(&self) -> u64 {
        self.info.duration_ms
    }
    fn aspect(&self) -> Option<f32> {
        Some(self.info.width as f32 / self.info.height as f32)
    }
    fn sync(&self, t_ms: u64, playing: bool, volume: f32) {
        #[cfg(feature = "audio")]
        if let Some(s) = &self.sound {
            s.sync(t_ms, playing, volume);
        }
        let _ = (t_ms, playing, volume);
    }
    fn paint(&self, f: &mut Frame, t_ms: u64) {
        let full = Rectangle::new(Point::default(), f.size());
        f.rect(full, Color::hex(0x000000));
        // Frames arrive from another thread; give a fresh decoder a moment on the first call.
        let mut shown = self.advance(t_ms);
        if shown.is_none() {
            for _ in 0..20 {
                std::thread::sleep(Duration::from_millis(10));
                shown = self.advance(t_ms);
                if shown.is_some() {
                    break;
                }
            }
        }
        if shown.is_some() {
            f.image(full, self.surface.id());
        }
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        if let Ok(st) = self.now.lock() {
            st.session.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_ui_core::DrawCommand;
    use alloy_ui_render::{CpuRenderer, HeadlessRenderer, PIXEL_ID_BASE};
    use alloy_ui_theme::Theme;
    use alloy_ui_widgets::Ui;

    /// A 2 s clip whose picture changes every frame (ffmpeg's `testsrc` counter and sweep).
    fn clip() -> Option<PathBuf> {
        if !ffmpeg_available() {
            eprintln!("SKIP: ffmpeg is not installed");
            return None;
        }
        let d = std::env::temp_dir().join(format!("alloy-video-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("testsrc.mp4");
        if !p.exists() {
            let ok = command("ffmpeg")
                .args([
                    "-v",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=size=320x180:rate=30",
                ])
                .args(["-t", "2", "-pix_fmt", "yuv420p", "-y"])
                .arg(&p)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            assert!(ok, "ffmpeg could not make the test clip");
        }
        Some(p)
    }

    /// Paint at `t` and render the result to an image; returns every pixel.
    fn picture(v: &Video, t: u64) -> Vec<u8> {
        let ui = Ui::new(Theme::light());
        let mut f = Frame::new(
            &ui,
            Point::default(),
            alloy_ui_core::Size::new(320.0, 180.0),
        );
        v.paint(&mut f, t);
        let cmds = f.finish();
        assert!(
            cmds.iter()
                .any(|c| matches!(c, DrawCommand::Image { id, .. } if *id >= PIXEL_ID_BASE)),
            "the frame is an image draw"
        );
        let mut list = alloy_ui_core::DrawList::default();
        for c in cmds {
            list.push(c);
        }
        let img = CpuRenderer::new(320, 180).render_to_image(&list, 320, 180, 1.0);
        img.rgba
    }

    #[test]
    fn opens_reports_shape_and_decodes_real_frames() {
        let Some(p) = clip() else { return };
        let v = Video::open(&p).unwrap();
        assert!(
            v.duration_ms() >= 1900 && v.duration_ms() <= 2100,
            "{}",
            v.duration_ms()
        );
        assert!((v.aspect().unwrap() - 16.0 / 9.0).abs() < 0.01);
        assert_eq!(v.size(), (320, 180));
        let a = picture(&v, 0);
        assert!(
            a.chunks(4).any(|p| p[0] != p[1] || p[1] != p[2]),
            "testsrc is colourful"
        );
        assert!(a.chunks(4).all(|p| p[3] == 255));
    }

    #[test]
    fn playing_forward_and_seeking_both_change_the_picture() {
        let Some(p) = clip() else { return };
        let v = Video::open(&p).unwrap();
        let a = picture(&v, 0);
        // Play: advance the clock a little at a time, like the UI does.
        let mut last = a.clone();
        for t in (33..=500).step_by(33) {
            std::thread::sleep(Duration::from_millis(5));
            last = picture(&v, t);
        }
        assert_ne!(a, last, "frames advance with the clock");
        // Seek far ahead: restarts ffmpeg at 1.5 s.
        let far = picture(&v, 1500);
        assert_ne!(far, last);
        // And back to the start: identical to the first frame.
        let back = picture(&v, 0);
        assert_eq!(a, back, "seeking to 0 shows frame 0 again");
    }

    #[test]
    fn audio_only_and_missing_files_are_not_video() {
        if !ffmpeg_available() {
            return;
        }
        assert!(Video::open("/definitely/not/here.mp4").is_err());
        let d = std::env::temp_dir().join(format!("alloy-video-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("tone.wav");
        let ok = command("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-y",
            ])
            .arg(&p)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(ok);
        assert!(matches!(Video::open(&p), Err(MediaError::Unsupported(_))));
    }
}
