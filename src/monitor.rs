//! Scans for USB MIDI devices every 2 s, attaching **and** detaching routers as
//! keyboards appear or disappear.
use anyhow::Result;
use midir::MidiInput;
use oxisynth::MidiEvent;
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::Sender,
    thread,
    time::Duration,
};

use crate::midi::MidiRouter;

/// Background watcher that keeps `MidiRouter`s in sync with the current set of
/// connected USB MIDI devices.
pub struct MidiDeviceMonitor {
    _handle: thread::JoinHandle<()>, // keeps the monitor thread alive
}

impl MidiDeviceMonitor {
    pub fn start(tx: Sender<MidiEvent>) -> Result<Self> {
        let handle = thread::spawn(move || {
            // name->router so we can drop routers when the device disappears
            let mut routers: HashMap<String, MidiRouter> = HashMap::new();

            loop {
                // New MidiInput each pass so the port list is up-to-date
                if let Ok(inp) = MidiInput::new("piano-monitor-scan") {
                    let mut present = HashSet::new();

                    for port in inp.ports() {
                        if let Ok(name) = inp.port_name(&port) {
                            // Crude filter: treat anything with “usb” in the name as a keyboard
                            if name.to_lowercase().contains("usb") {
                                present.insert(name.clone());

                                // New device → create router
                                if !routers.contains_key(&name) {
                                    match MidiRouter::connect(&port, tx.clone()) {
                                        Ok(router) => {
                                            routers.insert(name.clone(), router);
                                        }
                                        Err(e) => {
                                            log::warn!("Failed to connect to {name}: {e:#}");
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Device vanished → drop its router (connection closes automatically)
                    routers.retain(|name, _| {
                        if present.contains(name) {
                            true
                        } else {
                            log::info!("🎹 Disconnected {name}");
                            false
                        }
                    });
                }

                thread::sleep(Duration::from_secs(2));
            }
        });

        Ok(Self { _handle: handle })
    }
}
