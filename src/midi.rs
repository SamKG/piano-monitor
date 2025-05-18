//! Translates raw MIDI bytes into `oxisynth::MidiEvent`s and forwards them.

use anyhow::{Result, anyhow};
use midir::{Ignore, MidiInput, MidiInputConnection};
use oxisynth::MidiEvent;
use std::sync::mpsc::Sender;

pub struct MidiRouter {
    _conn: MidiInputConnection<()>, // RAII – stays alive
}

impl MidiRouter {
    pub fn connect(port: &midir::MidiInputPort, tx: Sender<MidiEvent>) -> Result<Self> {
        let mut midi_in = MidiInput::new("piano-monitor")?;
        midi_in.ignore(Ignore::None);

        let name = midi_in.port_name(port)?;
        let conn = midi_in
            .connect(
                port,
                "piano-monitor",
                move |_stamp, msg, _| {
                    if let Some(ev) = decode_midi(msg) {
                        tx.send(ev).ok();
                    }
                },
                (),
            )
            .map_err(|e| anyhow!("connect {name}: {e}"))?;

        log::info!("🎹 Connected to {name}");
        Ok(Self { _conn: conn })
    }
}

// ─────────────────── helpers ─────────────────────────────────────────────────

fn decode_midi(msg: &[u8]) -> Option<MidiEvent> {
    if msg.is_empty() {
        return None;
    }
    let status = msg[0];
    let channel = status & 0x0F;

    match status & 0xF0 {
        0x80 if msg.len() >= 3 => Some(MidiEvent::NoteOff {
            channel,
            key: msg[1],
        }),
        0x90 if msg.len() >= 3 => Some(MidiEvent::NoteOn {
            channel,
            key: msg[1],
            vel: msg[2],
        }),
        0xB0 if msg.len() >= 3 => Some(MidiEvent::ControlChange {
            channel,
            ctrl: msg[1],
            value: msg[2],
        }),
        0xC0 if msg.len() >= 2 => Some(MidiEvent::ProgramChange {
            channel,
            program_id: msg[1],
        }),
        0xE0 if msg.len() >= 3 => {
            let value = ((msg[2] as u16) << 7) | msg[1] as u16; // 14-bit
            Some(MidiEvent::PitchBend { channel, value })
        }
        _ => None, // ignore SysEx etc.
    }
}
