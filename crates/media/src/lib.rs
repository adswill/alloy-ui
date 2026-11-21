//! Real audio and video playback for Alloy's [`MediaPlayer`](alloy_ui_widgets::MediaPlayer).
//!
//! The widget crate ships the player (picture area, seek bar, volume, keys) and a
//! [`MediaSource`](alloy_ui_widgets::MediaSource) trait; this crate supplies sources that
//! actually decode:
//!
//! * [`Sound`] (feature `audio`, on by default): files that symphonia reads (MP3, FLAC, WAV,
//!   Ogg Vorbis, AAC, ALAC, MP4 and Matroska audio) decoded in pure Rust and played through the
//!   OS audio API with cpal. The picture area shows a live level meter computed from the
//!   samples that are really playing.
//! * [`Video`] (feature `ffmpeg`, on by default): any file the `ffmpeg` command-line tool can
//!   read. Frames come over a pipe, so the app links no codec library and ships no patent-
//!   encumbered code; the user needs `ffmpeg` on their `PATH`. Frames reach the screen through
//!   [`PixelSurface`](alloy_ui_render::PixelSurface), so it works on the GPU renderer, the CPU
//!   fallback and in headless tests. When the `audio` feature is on, the sound track plays too.
//! * [`open`] picks one for a path.
//!
//! ```no_run
//! use alloy_ui_media::open;
//! let source = open("clip.mp4").expect("readable media");
//! // media(source, playback) in the Alloy DSL, or MediaPlayer::new in the widgets crate.
//! ```
//!
//! What is verified where: the decode, seek, resample and clock logic is covered by tests that
//! run on every OS in CI. Real output through a sound card and real video decoding need a
//! device or an `ffmpeg` binary, so those tests skip themselves when they are missing.

#[cfg(not(any(feature = "audio", feature = "ffmpeg")))]
compile_error!("alloy-ui-media needs the `audio` feature, the `ffmpeg` feature, or both");

#[cfg(feature = "audio")]
mod audio;
#[cfg(feature = "audio")]
mod device;
mod error;
#[cfg(feature = "ffmpeg")]
mod video;

pub use error::MediaError;

#[cfg(feature = "audio")]
pub use audio::{Engine, Sound};
#[cfg(feature = "ffmpeg")]
pub use video::{ffmpeg_available, Video};

use alloy_ui_widgets::{Frame, MediaSource};
use std::path::Path;

/// Whichever source can play a file, behind one type.
pub enum Media {
    /// A file with video, played through ffmpeg.
    #[cfg(feature = "ffmpeg")]
    Video(Box<Video>),
    /// An audio file.
    #[cfg(feature = "audio")]
    Sound(Box<Sound>),
}

/// Open `path` for playback: video through ffmpeg when it is installed and the file has a
/// picture, otherwise audio through symphonia.
pub fn open(path: impl AsRef<Path>) -> Result<Media, MediaError> {
    let path = path.as_ref();
    #[cfg(feature = "ffmpeg")]
    {
        match Video::open(path) {
            Ok(v) => return Ok(Media::Video(Box::new(v))),
            Err(e) => log::debug!("not opened as video: {e}"),
        }
    }
    #[cfg(feature = "audio")]
    {
        return Sound::open(path).map(|s| Media::Sound(Box::new(s)));
    }
    #[allow(unreachable_code)]
    Err(MediaError::Unsupported(
        "built without the `audio` and `ffmpeg` features".into(),
    ))
}

impl MediaSource for Media {
    fn duration_ms(&self) -> u64 {
        match self {
            #[cfg(feature = "ffmpeg")]
            Media::Video(v) => v.duration_ms(),
            #[cfg(feature = "audio")]
            Media::Sound(s) => s.duration_ms(),
        }
    }
    fn aspect(&self) -> Option<f32> {
        match self {
            #[cfg(feature = "ffmpeg")]
            Media::Video(v) => v.aspect(),
            #[cfg(feature = "audio")]
            Media::Sound(s) => s.aspect(),
        }
    }
    fn paint(&self, frame: &mut Frame, t_ms: u64) {
        match self {
            #[cfg(feature = "ffmpeg")]
            Media::Video(v) => v.paint(frame, t_ms),
            #[cfg(feature = "audio")]
            Media::Sound(s) => s.paint(frame, t_ms),
        }
    }
    fn sync(&self, t_ms: u64, playing: bool, volume: f32) {
        match self {
            #[cfg(feature = "ffmpeg")]
            Media::Video(v) => v.sync(t_ms, playing, volume),
            #[cfg(feature = "audio")]
            Media::Sound(s) => s.sync(t_ms, playing, volume),
        }
    }
}
