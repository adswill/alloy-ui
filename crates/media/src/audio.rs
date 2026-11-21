//! Streaming audio decode with symphonia, and the mixer the sound card pulls from.
//!
//! A background thread decodes ahead of the playing position into a bounded queue, so memory
//! stays at a few seconds of samples however long the file is. The [`Engine`] is independent of
//! any sound card: [`Engine::fill`] is the body of the output callback, which makes the decode,
//! seek, resample and clock logic testable on machines with no audio device.

use crate::{device::Output, error::MediaError};
use alloy_ui_core::{Point, Rectangle, Size};
use alloy_ui_widgets::{Frame, MediaSource};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL},
    errors::Error as SymError,
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::Time,
};

/// Slots in the scrolling level meter.
pub const LEVELS: usize = 48;
/// Seconds of decoded audio kept ahead of the playing position.
const AHEAD_SECS: u32 = 3;

struct State {
    /// Interleaved source samples; the first frame is the one currently playing.
    buf: VecDeque<f32>,
    /// Source position of the first frame in `buf`, in ms, plus frames consumed since.
    base_ms: u64,
    consumed: u64,
    /// Output phase between the first and second buffered frame (resampling).
    frac: f64,
    /// A seek the decoder thread has not performed yet.
    seek: Option<u64>,
    /// Bumped on every seek so a packet decoded before it is discarded.
    generation: u64,
    eof: bool,
    playing: bool,
    volume: f32,
    levels: VecDeque<f32>,
    slice_energy: f64,
    slice_frames: u32,
}

struct Shared {
    state: Mutex<State>,
    cv: Condvar,
    stop: AtomicBool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// A decoded, seekable, resampling audio stream with a play/pause clock.
pub struct Engine {
    shared: Arc<Shared>,
    rate: u32,
    channels: u16,
    duration_ms: u64,
    thread: Mutex<Option<JoinHandle<()>>>,
}

struct Decoding {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
}

/// An opened stream and what the container says about it: frames, rate, channels.
type Opened = (Decoding, Option<u64>, Option<u32>, Option<usize>);

impl Decoding {
    fn open(path: &Path) -> Result<Opened, MediaError> {
        let file = File::open(path).map_err(|e| MediaError::Io(path.display().to_string(), e))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| MediaError::Unsupported(e.to_string()))?;
        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| MediaError::Unsupported("no audio track".into()))?;
        let params = track.codec_params.clone();
        let decoder = symphonia::default::get_codecs()
            .make(&params, &DecoderOptions::default())
            .map_err(|e| MediaError::Unsupported(e.to_string()))?;
        let track_id = track.id;
        let frames = params.n_frames;
        let rate = params.sample_rate;
        let channels = params.channels.map(|c| c.count());
        Ok((
            Self {
                format,
                decoder,
                track_id,
            },
            frames,
            rate,
            channels,
        ))
    }

    /// Decode the next packet of our track into interleaved samples, or `None` at the end.
    fn next(&mut self) -> Option<(Vec<f32>, u32, usize)> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => return None,
            };
            if packet.track_id() != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                    sb.copy_interleaved_ref(decoded);
                    return Some((sb.samples().to_vec(), spec.rate, spec.channels.count()));
                }
                Err(SymError::DecodeError(_)) | Err(SymError::IoError(_)) => continue,
                Err(_) => return None,
            }
        }
    }

    /// Seek to `ms`; returns the number of frames to drop after the seek because the format
    /// could only land earlier.
    fn seek(&mut self, ms: u64, rate: u32) -> u64 {
        let time = Time::new(ms / 1000, (ms % 1000) as f64 / 1000.0);
        let r = self.format.seek(
            SeekMode::Accurate,
            SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            },
        );
        self.decoder.reset();
        match r {
            Ok(to) => {
                // Timestamps are in the track's time base; for the formats we read it is frames.
                let _ = rate;
                to.required_ts.saturating_sub(to.actual_ts)
            }
            Err(_) => 0,
        }
    }
}

impl Engine {
    /// Open `path` and start decoding ahead. Fails when no audio track can be decoded.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MediaError> {
        let path = path.as_ref();
        let (mut dec, n_frames, rate, channels) = Decoding::open(path)?;
        // Decode the first packet now: it settles the real rate and channel count.
        let (first, frate, fch) = dec
            .next()
            .ok_or_else(|| MediaError::Decode("no decodable audio".into()))?;
        let rate = rate.filter(|r| *r > 0).unwrap_or(frate).max(1);
        let ch = channels.filter(|c| *c > 0).unwrap_or(fch).max(1);
        let duration_ms = match n_frames {
            Some(n) => n * 1000 / rate as u64,
            None => 0,
        };
        let mut buf = VecDeque::with_capacity((rate as usize) * ch * AHEAD_SECS as usize);
        buf.extend(first);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                buf,
                base_ms: 0,
                consumed: 0,
                frac: 0.0,
                seek: None,
                generation: 0,
                eof: false,
                playing: false,
                volume: 1.0,
                levels: VecDeque::from(vec![0.0; LEVELS]),
                slice_energy: 0.0,
                slice_frames: 0,
            }),
            cv: Condvar::new(),
            stop: AtomicBool::new(false),
        });
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("alloy-audio-decode".into())
                .spawn(move || decode_loop(shared, dec, rate, ch))
                .map_err(|e| MediaError::Decode(e.to_string()))?
        };
        let engine = Self {
            shared,
            rate,
            channels: ch as u16,
            duration_ms,
            thread: Mutex::new(Some(thread)),
        };
        Ok(engine)
    }

    /// Length of the stream in milliseconds (0 when the file does not say).
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
    /// Source sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
    /// Source channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }
    /// Start (or resume) playing: [`Engine::fill`] now consumes samples.
    pub fn play(&self) {
        self.shared.lock().playing = true;
    }
    /// Pause: [`Engine::fill`] writes silence and the position holds.
    pub fn pause(&self) {
        self.shared.lock().playing = false;
    }
    /// True while playing.
    pub fn playing(&self) -> bool {
        self.shared.lock().playing
    }
    /// Output gain, 0.0 to 1.0.
    pub fn set_volume(&self, v: f32) {
        self.shared.lock().volume = v.clamp(0.0, 1.0);
    }
    /// Jump to `ms`. The queue is emptied at once; the decoder refills it.
    pub fn seek(&self, ms: u64) {
        let mut s = self.shared.lock();
        s.buf.clear();
        s.base_ms = ms;
        s.consumed = 0;
        s.frac = 0.0;
        s.eof = false;
        s.seek = Some(ms);
        s.generation += 1;
        self.shared.cv.notify_all();
    }
    /// How far playback has got, in milliseconds.
    pub fn position_ms(&self) -> u64 {
        let s = self.shared.lock();
        s.base_ms + s.consumed * 1000 / self.rate as u64
    }
    /// True once the whole stream has been played.
    pub fn finished(&self) -> bool {
        let s = self.shared.lock();
        s.eof && s.buf.len() < 2 * self.channels as usize
    }
    /// Seconds of audio decoded and waiting, for tests and diagnostics.
    pub fn buffered_ms(&self) -> u64 {
        let s = self.shared.lock();
        (s.buf.len() / self.channels as usize) as u64 * 1000 / self.rate as u64
    }
    /// The most recent loudness slices, oldest first, each 0.0 to 1.0.
    pub fn levels(&self) -> Vec<f32> {
        self.shared.lock().levels.iter().copied().collect()
    }

    /// The body of the output callback: write `out` (interleaved, `out_channels` wide, at
    /// `out_rate`) from the queue, resampling linearly and mapping channels. Writes silence
    /// while paused or when the decoder has not caught up.
    pub fn fill(&self, out: &mut [f32], out_channels: u16, out_rate: u32) {
        let och = out_channels.max(1) as usize;
        let sch = self.channels as usize;
        let step = self.rate as f64 / out_rate.max(1) as f64;
        let slice_len = (out_rate / LEVELS as u32).max(1);
        let mut s = self.shared.lock();
        let gain = s.volume;
        let playing = s.playing;
        let mut woke = false;
        for frame in out.chunks_exact_mut(och) {
            let have = s.buf.len() / sch;
            let need = if s.frac > 0.0 { 2 } else { 1 };
            if !playing || have < need {
                frame.fill(0.0);
                continue;
            }
            let w = s.frac as f32;
            let mut mono = 0.0f32;
            for (c, o) in frame.iter_mut().enumerate() {
                let get = |i: usize, k: usize| s.buf[i * sch + k];
                let v = if och == 1 && sch > 1 {
                    // Fold down: average every source channel.
                    let at = |i: usize| (0..sch).map(|k| get(i, k)).sum::<f32>() / sch as f32;
                    let a = at(0);
                    if w > 0.0 {
                        a + (at(1) - a) * w
                    } else {
                        a
                    }
                } else {
                    let k = c.min(sch - 1);
                    let a = get(0, k);
                    if w > 0.0 {
                        a + (get(1, k) - a) * w
                    } else {
                        a
                    }
                };
                *o = v * gain;
                mono += v;
            }
            let m = mono / och as f32;
            s.slice_energy += (m * m) as f64;
            s.slice_frames += 1;
            if s.slice_frames >= slice_len {
                let rms = (s.slice_energy / s.slice_frames as f64).sqrt() as f32;
                s.levels.pop_front();
                s.levels.push_back((rms * 2.4).clamp(0.0, 1.0));
                s.slice_energy = 0.0;
                s.slice_frames = 0;
            }
            s.frac += step;
            while s.frac >= 1.0 && s.buf.len() >= sch {
                s.buf.drain(..sch);
                s.consumed += 1;
                s.frac -= 1.0;
                woke = true;
            }
        }
        if !playing && s.levels.iter().any(|l| *l > 0.0) {
            s.levels.pop_front();
            s.levels.push_back(0.0);
        }
        drop(s);
        if woke {
            self.shared.cv.notify_all();
        }
    }
}

fn decode_loop(shared: Arc<Shared>, mut dec: Decoding, rate: u32, ch: usize) {
    let target = rate as usize * ch * AHEAD_SECS as usize;
    let mut skip_frames = 0u64;
    loop {
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        // Wait for room, or for a seek.
        let (generation, seek) = {
            let mut s = shared.lock();
            loop {
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
                if s.seek.is_some() || (s.buf.len() < target && !s.eof) {
                    break;
                }
                let (g, _) = shared
                    .cv
                    .wait_timeout(s, Duration::from_millis(100))
                    .unwrap_or_else(|e| e.into_inner());
                s = g;
            }
            (s.generation, s.seek.take())
        };
        if let Some(ms) = seek {
            skip_frames = dec.seek(ms, rate);
        }
        match dec.next() {
            Some((mut samples, _r, c)) if c == ch => {
                if skip_frames > 0 {
                    let drop_n = ((skip_frames as usize) * ch).min(samples.len());
                    samples.drain(..drop_n);
                    skip_frames -= (drop_n / ch) as u64;
                }
                let mut s = shared.lock();
                if s.generation == generation {
                    s.buf.extend(samples);
                }
            }
            Some(_) => {} // a stream that changes channel count mid-way: skip the packet
            None => {
                let mut s = shared.lock();
                if s.generation == generation && s.seek.is_none() {
                    s.eof = true;
                }
            }
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.cv.notify_all();
        if let Some(t) = self.thread.lock().ok().and_then(|mut t| t.take()) {
            let _ = t.join();
        }
    }
}

/// An audio file as a [`MediaSource`]: decoded by symphonia, played through the default
/// output device, drawn as a live level meter.
pub struct Sound {
    engine: Arc<Engine>,
    output: RefCell<Option<Output>>,
    output_failed: RefCell<bool>,
}

impl Sound {
    /// Open an audio file. Playback starts when the player starts it; opening opens no device.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MediaError> {
        Ok(Self {
            engine: Arc::new(Engine::open(path)?),
            output: RefCell::new(None),
            output_failed: RefCell::new(false),
        })
    }
    /// The decoder and clock, for tests or for routing the samples elsewhere.
    pub fn engine(&self) -> &Arc<Engine> {
        &self.engine
    }
    fn ensure_output(&self) {
        if self.output.borrow().is_some() || *self.output_failed.borrow() {
            return;
        }
        match Output::start(self.engine.clone()) {
            Ok(o) => *self.output.borrow_mut() = Some(o),
            Err(e) => {
                log::warn!("no audio output ({e}); playing silently");
                *self.output_failed.borrow_mut() = true;
            }
        }
    }
}

impl MediaSource for Sound {
    fn duration_ms(&self) -> u64 {
        self.engine.duration_ms()
    }
    fn aspect(&self) -> Option<f32> {
        None
    }
    fn sync(&self, t_ms: u64, playing: bool, volume: f32) {
        let e = &self.engine;
        e.set_volume(volume);
        if e.position_ms().abs_diff(t_ms) > 300 {
            e.seek(t_ms);
        }
        if playing {
            e.play();
            self.ensure_output();
        } else {
            e.pause();
        }
    }
    fn paint(&self, f: &mut Frame, _t_ms: u64) {
        let s = f.size();
        let levels = self.engine.levels();
        let n = levels.len().max(1);
        let w = (s.width - 48.0) / n as f32;
        let acc = f.palette().accent;
        for (i, l) in levels.iter().enumerate() {
            let h = (s.height * 0.62 * l).max(4.0);
            f.round_rect(
                Rectangle::new(
                    Point::new(24.0 + i as f32 * w + 1.0, s.height * 0.75 - h),
                    Size::new((w - 2.0).max(2.0), h),
                ),
                2.0,
                acc.with_alpha(0.3 + 0.7 * l),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Write a 16-bit PCM WAV of `secs` seconds of a 440 Hz tone whose amplitude steps up each
    /// second, so a position can be read back from the samples.
    pub(crate) fn wav(path: &Path, rate: u32, channels: u16, secs: u32) {
        let frames = rate * secs;
        let mut data = Vec::new();
        for i in 0..frames {
            let sec = i / rate;
            let amp = 0.1 + 0.1 * sec as f32; // 0.1, 0.2, 0.3 ...
            let v = ((i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin()
                * amp
                * i16::MAX as f32) as i16;
            for _ in 0..channels {
                data.extend_from_slice(&v.to_le_bytes());
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        out.extend_from_slice(&(channels * 2).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        std::fs::write(path, out).unwrap();
    }

    pub(crate) fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("alloy-media-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    fn wait_buffered(e: &Engine, ms: u64) {
        for _ in 0..200 {
            if e.buffered_ms() >= ms {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("decoder never buffered {ms} ms (has {})", e.buffered_ms());
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0, |a, b| a.max(b.abs()))
    }

    #[test]
    fn decodes_reports_duration_and_plays_in_order() {
        let p = tmp("tone.wav");
        wav(&p, 8000, 1, 3);
        let e = Engine::open(&p).unwrap();
        assert_eq!((e.sample_rate(), e.channels()), (8000, 1));
        assert_eq!(e.duration_ms(), 3000);
        wait_buffered(&e, 1500);
        // Paused: silence, and the clock does not move.
        let mut out = vec![1.0; 800];
        e.fill(&mut out, 1, 8000);
        assert!(out.iter().all(|v| *v == 0.0));
        assert_eq!(e.position_ms(), 0);
        e.play();
        e.fill(&mut out, 1, 8000);
        assert!(
            peak(&out) > 0.05 && peak(&out) < 0.15,
            "first second is the quiet one"
        );
        assert_eq!(e.position_ms(), 100, "800 frames at 8 kHz");
    }

    #[test]
    fn seek_lands_on_the_requested_second() {
        let p = tmp("seek.wav");
        wav(&p, 8000, 1, 3);
        let e = Engine::open(&p).unwrap();
        e.play();
        e.seek(2000);
        assert_eq!(e.position_ms(), 2000);
        wait_buffered(&e, 500);
        let mut out = vec![0.0; 800];
        e.fill(&mut out, 1, 8000);
        // The third second has amplitude 0.3.
        assert!(
            peak(&out) > 0.25 && peak(&out) < 0.35,
            "peak {}",
            peak(&out)
        );
        assert_eq!(e.position_ms(), 2100);
        // And back to the start.
        e.seek(0);
        wait_buffered(&e, 500);
        e.fill(&mut out, 1, 8000);
        assert!(peak(&out) < 0.15);
    }

    #[test]
    fn resamples_and_maps_channels() {
        let p = tmp("stereo.wav");
        wav(&p, 8000, 2, 2);
        let e = Engine::open(&p).unwrap();
        e.play();
        wait_buffered(&e, 1000);
        // 16 kHz stereo -> 4,000 frames consume 2,000 source frames = 250 ms.
        let mut out = vec![0.0; 4000 * 2];
        e.fill(&mut out, 2, 16000);
        assert!(
            (e.position_ms() as i64 - 250).abs() <= 1,
            "pos {}",
            e.position_ms()
        );
        assert!(
            out.chunks(2).all(|f| (f[0] - f[1]).abs() < 1e-6),
            "both channels carry the tone"
        );
        // Into mono and into 6 channels without panicking, with the tone present.
        let mut mono = vec![0.0; 800];
        e.fill(&mut mono, 1, 8000);
        assert!(peak(&mono) > 0.05);
        let mut six = vec![0.0; 6 * 100];
        e.fill(&mut six, 6, 8000);
        assert!(peak(&six) > 0.05);
    }

    #[test]
    fn volume_scales_output_and_levels_move() {
        let p = tmp("vol.wav");
        wav(&p, 8000, 1, 2);
        let e = Engine::open(&p).unwrap();
        e.play();
        wait_buffered(&e, 800);
        e.set_volume(0.5);
        let mut out = vec![0.0; 800];
        e.fill(&mut out, 1, 8000);
        assert!(peak(&out) < 0.08, "half of ~0.1");
        assert!(e.levels().iter().any(|l| *l > 0.0), "the meter saw sound");
    }

    #[test]
    fn plays_to_the_end_and_reports_finished() {
        let p = tmp("end.wav");
        wav(&p, 8000, 1, 1);
        let e = Engine::open(&p).unwrap();
        e.play();
        let mut out = vec![0.0; 800];
        for _ in 0..40 {
            wait_buffered(&e, 1);
            e.fill(&mut out, 1, 8000);
            if e.finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(e.finished());
        assert!(e.position_ms() >= 950, "pos {}", e.position_ms());
    }

    #[test]
    fn rejects_files_that_are_not_audio() {
        let p = tmp("nope.wav");
        std::fs::write(&p, b"this is not audio at all").unwrap();
        assert!(Engine::open(&p).is_err());
        assert!(Engine::open(tmp("missing.wav")).is_err());
    }
}
