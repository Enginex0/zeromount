use std::io::Read as _;
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use tracing::warn;

pub const CMD_TIMEOUT: Duration = Duration::from_secs(30);
const CMD_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub fn run_command_with_timeout(cmd: &mut Command, timeout: Duration) -> Result<std::process::Output> {
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn {:?}", cmd.get_program()))?;

    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout {
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(ref mut err) = child.stderr {
                    let _ = err.read_to_end(&mut stderr);
                }
                return Ok(std::process::Output { status, stdout, stderr });
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    warn!(program = ?cmd.get_program(), "command timed out, killing");
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("command {:?} timed out after {timeout:?}", cmd.get_program());
                }
                if crate::utils::signal::shutdown_requested() {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!("shutdown requested, killed {:?}", cmd.get_program());
                }
                std::thread::sleep(CMD_POLL_INTERVAL);
            }
            Err(e) => bail!("error waiting for {:?}: {e}", cmd.get_program()),
        }
    }
}
