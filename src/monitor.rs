//! Scans for new USB MIDI devices every 2 s and attaches routers.

use anyhow::Result;
use midir::MidiInput;
use oxisynth::MidiEvent;
use std::{collections::HashSet, sync::mpsc::Sender, thread, time::Duration};

use crate::midi::MidiRouter;

pub struct MidiDeviceMonitor {
    _handle: thread::JoinHandle<()>,
}

impl MidiDeviceMonitor {
    pub fn start(tx: Sender<MidiEvent>) -> Result<Self> {
        let handle = thread::spawn(move || {
            let mut seen = HashSet::<String>::new();
            let mut routers = Vec::<MidiRouter>::new(); // keep alive

            loop {
                if let Ok(inp) = MidiInput::new("piano-monitor-scan") {
                    for port in inp.ports() {
                        if let Ok(name) = inp.port_name(&port) {
                            if !seen.contains(&name) && name.to_lowercase().contains("usb") {
                                if let Ok(router) = MidiRouter::connect(&port, tx.clone()) {
                                    seen.insert(name);
                                    routers.push(router);
                                }
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_secs(2));
            }
        });
        Ok(Self { _handle: handle })
    }
}
