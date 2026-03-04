use std::fs::{self, OpenOptions};

use anyhow::{Context, Result};

const GUARD_DIR: &str = "/data/adb/zeromount/guard";

pub fn record_marker(prefix: &str, threshold: u32, config: &crate::core::config::ZeroMountConfig) -> Result<()> {
    fs::create_dir_all(GUARD_DIR).context("creating guard dir")?;

    let count = count_markers(prefix);
    let next = count + 1;

    if next >= threshold {
        tracing::error!(prefix, count = next, threshold, "guard marker threshold reached");
        super::recovery::execute(config);
    }

    let path = format!("{GUARD_DIR}/{prefix}_{next}");
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("creating marker {path}"))?;

    tracing::info!(prefix, count = next, threshold, "guard marker recorded");
    Ok(())
}

pub fn clear_all() -> Result<()> {
    if let Ok(entries) = fs::read_dir(GUARD_DIR) {
        for entry in entries.flatten() {
            let _ = fs::remove_file(entry.path());
        }
    }
    tracing::info!("guard markers cleared");
    Ok(())
}

pub fn status() -> (u32, u32) {
    (count_markers("pfd"), count_markers("svc"))
}

pub fn any_triggered(threshold: u32) -> bool {
    let (pfd, svc) = status();
    pfd >= threshold || svc >= threshold
}

fn count_markers(prefix: &str) -> u32 {
    let entries = match fs::read_dir(GUARD_DIR) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let pat = format!("{prefix}_");
    entries
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&pat))
        .count() as u32
}
