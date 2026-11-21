//! The sound card: a cpal output stream that pulls from an [`Engine`](crate::Engine).

use crate::audio::Engine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use std::sync::Arc;

/// A running output stream. Dropping it stops the sound.
pub struct Output {
    _stream: cpal::Stream,
}

impl Output {
    /// Open the default output device and start pulling from `engine`.
    pub fn start(engine: Arc<Engine>) -> Result<Self, String> {
        let host = cpal::default_host();
        let dev = host
            .default_output_device()
            .ok_or("no default output device")?;
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let (rate, channels) = (cfg.sample_rate().0, cfg.channels());
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(&dev, &cfg.into(), engine, rate, channels),
            cpal::SampleFormat::I16 => build::<i16>(&dev, &cfg.into(), engine, rate, channels),
            cpal::SampleFormat::U16 => build::<u16>(&dev, &cfg.into(), engine, rate, channels),
            other => Err(format!("unsupported sample format {other:?}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Self { _stream: stream })
    }
}

fn build<T: SizedSample + FromSample<f32>>(
    dev: &cpal::Device,
    cfg: &cpal::StreamConfig,
    engine: Arc<Engine>,
    rate: u32,
    channels: u16,
) -> Result<cpal::Stream, String> {
    let mut tmp: Vec<f32> = Vec::new();
    dev.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            tmp.resize(data.len(), 0.0);
            engine.fill(&mut tmp, channels, rate);
            for (d, s) in data.iter_mut().zip(&tmp) {
                *d = T::from_sample(*s);
            }
        },
        |e| log::warn!("audio stream error: {e}"),
        None,
    )
    .map_err(|e| e.to_string())
}
