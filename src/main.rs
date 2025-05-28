mod audio;
mod jack_boot;
mod midi;
mod monitor;

use anyhow::{Context, Result};
use env_logger::Env;
use std::path::PathBuf;
use std::sync::mpsc::channel;

fn main() -> Result<()> {
    jack_boot::ensure_jack_running(std::time::Duration::from_secs(3)).context("starting JACK")?;
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let sf_path: PathBuf = std::env::var("SOUNDFONT")
        .map(Into::into)
        .unwrap_or("/usr/share/soundfonts/FluidR3_GM.sf2".into());
    log::info!("🖖 Using sound-font {:?}", sf_path);

    // pipeline: MIDI → synth
    let (tx, rx) = channel::<oxisynth::MidiEvent>();

    let _audio = audio::AudioEngine::start(rx, &sf_path)?;
    let _mon = monitor::MidiDeviceMonitor::start(tx)?;

    std::thread::park();
    Ok(())
}
