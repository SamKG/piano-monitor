//! Owns the OxiSynth instance and the CPAL output stream.
//! Latency-tuned: 32-frame request, zero-copy fast path, no assumptions about
//! actual buffer length.

use anyhow::{Context, Result, anyhow};
use cpal::{
    BufferSize, HostId, Sample, SampleFormat, SizedSample, Stream, StreamConfig, host_from_id,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use oxisynth::{MidiEvent, SoundFont, Synth, SynthDescriptor};
use std::{fs::File, path::Path, sync::mpsc::Receiver};

/// We *request* 32 frames (≈0.67 ms @ 48 kHz) but must cope with anything.
const REQUESTED_FRAMES: u32 = 32;

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn start(rx: Receiver<MidiEvent>, font_path: &Path) -> Result<Self> {
        // Prefer JACK; fall back to default host.
        let host = host_from_id(HostId::Jack).unwrap_or_else(|_| cpal::default_host());
        let device = host
            .default_output_device()
            .context("no default output device")?;

        let def_cfg = device.default_output_config()?;
        let sample_format = def_cfg.sample_format();
        let mut stream_cfg: StreamConfig = def_cfg.into();
        stream_cfg.buffer_size = BufferSize::Fixed(REQUESTED_FRAMES);

        let stream = match sample_format {
            SampleFormat::F32 => Self::run_f32(&device, &stream_cfg, rx, font_path)?,
            SampleFormat::I16 | SampleFormat::U16 => {
                Self::run_generic::<i16>(&device, &stream_cfg, rx, font_path)?
            }
            _ => unreachable!("unexpected CPAL sample format"),
        };

        stream.play()?;
        Ok(Self { _stream: stream })
    }

    // ─────────────────────────── f32 fast path ─────────────────────────── //

    fn run_f32(
        device: &cpal::Device,
        cfg: &StreamConfig,
        rx: Receiver<MidiEvent>,
        font_path: &Path,
    ) -> Result<Stream> {
        let channels = cfg.channels as usize;
        let mut synth = new_synth(cfg.sample_rate.0 as f32, font_path)?;

        let err_fn = |e| log::error!("audio stream error: {e}");
        let stream = device.build_output_stream(
            cfg,
            move |output: &mut [f32], _| {
                while let Ok(ev) = rx.try_recv() {
                    synth.send_event(ev).ok();
                }

                for frame in output.chunks_mut(channels) {
                    let (l, r) = synth.read_next();
                    frame[0] = l;
                    if channels > 1 {
                        frame[1] = r;
                    }
                }
            },
            err_fn,
            None,
        )?;
        Ok(stream)
    }

    // ─────────────── generic path for I16 / U16 sample formats ─────────── //

    fn run_generic<T>(
        device: &cpal::Device,
        cfg: &StreamConfig,
        rx: Receiver<MidiEvent>,
        font_path: &Path,
    ) -> Result<Stream>
    where
        T: Sample + SizedSample + num_traits::cast::FromPrimitive,
    {
        let channels = cfg.channels as usize;
        let mut synth = new_synth(cfg.sample_rate.0 as f32, font_path)?;

        let err_fn = |e| log::error!("audio stream error: {e}");
        let stream = device.build_output_stream(
            cfg,
            move |output: &mut [T], _| {
                while let Ok(ev) = rx.try_recv() {
                    synth.send_event(ev).ok();
                }

                for frame in output.chunks_mut(channels) {
                    let (l, r) = synth.read_next();
                    frame[0] = T::from_f32(l).unwrap();
                    if channels > 1 {
                        frame[1] = T::from_f32(r).unwrap();
                    }
                }
            },
            err_fn,
            None,
        )?;
        Ok(stream)
    }
}

// ───────────────────────────── helpers ─────────────────────────────────── //

fn new_synth(sample_rate: f32, font_path: &Path) -> Result<Synth> {
    let desc = SynthDescriptor {
        sample_rate,
        gain: 2.0,
        ..Default::default()
    };
    let mut synth = Synth::new(desc).map_err(|e| anyhow!("synth init: {e:?}"))?;

    let mut file =
        File::open(font_path).with_context(|| format!("open sound-font {:?}", font_path))?;
    let font =
        SoundFont::load(&mut file).map_err(|_| anyhow!("load sound-font {:?}", font_path))?;
    synth.add_font(font, true);
    synth.set_sample_rate(sample_rate);
    Ok(synth)
}
