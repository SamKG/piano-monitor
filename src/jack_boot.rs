use anyhow::{Context, Result, anyhow};
use jack::{Client, ClientOptions};
use std::{
    process::{Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

/// Try to open a throw-away JACK client **without** starting the server.
/// Success ⇒ server is up and ready to accept clients.
fn jack_is_running() -> bool {
    matches!(
        Client::new("jack-probe", ClientOptions::NO_START_SERVER),
        Ok(_) // server answered
    )
}

/// Start a JACK server with the command you like.  Return as soon as the
/// daemon process itself has spawned (not when the graph is ready).
fn spawn_jackd() -> Result<()> {
    log::info!("Starting JACK server...");
    // Example command line – customise to taste or pull from a config file.
    let cmd = [
        "jackd", "-P85", // RT priority
        "-dalsa", "-dhw:0", // your interface; “-dhw:PCH” etc. if needed
        "-r48000", "-p64", "-n2",
    ];

    // We redirect stdout/stderr to null because JACK is chatty when it starts.
    Command::new(cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| "failed to spawn jackd")?;

    Ok(())
}

/// Ensure a JACK server is running.  If not, launch it and wait until it is
/// ready (or bail after `timeout`).
pub fn ensure_jack_running(timeout: Duration) -> Result<()> {
    if jack_is_running() {
        log::info!("JACK server is already running.");
        return Ok(());
    }

    // ── start it ─────────────────────────────────────────────────────────────
    spawn_jackd()?;
    let start = Instant::now();

    // ── poll until the server answers ────────────────────────────────────────
    loop {
        log::info!("Waiting for JACK server...");
        if jack_is_running() {
            return Ok(());
        }
        if start.elapsed() > timeout {
            return Err(anyhow!("JACK did not start within {timeout:?}"));
        }
        sleep(Duration::from_millis(100));
    }
}
