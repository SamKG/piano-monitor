//! Owns the OxiSynth instance and the CPAL output stream.
use anyhow::{Context, Result, anyhow};
use cpal::{
    BufferSize, HostId, Sample, SampleFormat, SizedSample, Stream, StreamConfig, host_from_id,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use oxisynth::{MidiEvent, SoundFont, Synth, SynthDescriptor};
use std::{fs::File, path::Path, sync::mpsc::Receiver};

pub struct AudioEngine {
    _stream: Stream,
}

impl AudioEngine {
    pub fn start(rx: Receiver<MidiEvent>, font_path: &Path) -> Result<Self> {
        // ----- prefer JACK; fall back to whatever default_host() gives -----
        let host = host_from_id(HostId::Jack).unwrap_or_else(|_| cpal::default_host());

        let device = host
            .default_output_device()
            .context("no default output device")?;

        // default config → customised StreamConfig (64-frame fixed buffer)
        let def_cfg = device.default_output_config()?;
        let sample_format = def_cfg.sample_format();
        let mut stream_cfg: StreamConfig = def_cfg.into();
        stream_cfg.buffer_size = BufferSize::Fixed(64); // <<< tiny block

        // choose the concrete stream implementation
        let stream = match sample_format {
            SampleFormat::F32 => Self::run::<f32>(&device, &stream_cfg, rx, font_path)?,
            SampleFormat::I16 => Self::run::<i16>(&device, &stream_cfg, rx, font_path)?,
            SampleFormat::U16 => Self::run::<u16>(&device, &stream_cfg, rx, font_path)?,
            _ => unreachable!("unknown sample format"),
        };

        stream.play()?;
        Ok(Self { _stream: stream })
    }

    fn run<T>(
        device: &cpal::Device,
        cfg: &StreamConfig,
        rx: Receiver<MidiEvent>,
        font_path: &Path,
    ) -> Result<Stream>
    where
        T: Sample + SizedSample + num_traits::cast::FromPrimitive,
    {
        // ---- initialise the synth ----
        let sample_rate = cfg.sample_rate.0 as f32;
        let mut synth = {
            let desc = SynthDescriptor {
                sample_rate,
                gain: 2.0,
                ..Default::default()
            };
            let mut s = Synth::new(desc).map_err(|e| anyhow!("synth init: {e:?}"))?;
            let mut file = File::open(font_path)
                .with_context(|| format!("open sound-font {:?}", font_path))?;
            let font = SoundFont::load(&mut file)
                .map_err(|_| anyhow!("load sound-font {:?}", font_path))?;
            s.add_font(font, true);
            s.set_sample_rate(sample_rate);
            s
        };

        let channels = cfg.channels as usize;
        let err_fn = |e| log::error!("audio stream error: {e}");

        let stream = device.build_output_stream(
            cfg,
            move |output: &mut [T], _| {
                // drain any pending MIDI events
                while let Ok(ev) = rx.try_recv() {
                    synth.send_event(ev).ok();
                }
                // render one buffer of audio
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
