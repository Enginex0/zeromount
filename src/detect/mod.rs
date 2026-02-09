pub mod kernel;
pub mod susfs;
pub mod watcher;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::core::types::{CapabilityFlags, DetectionResult, Scenario};

const DETECTION_JSON_PATH: &str = "/data/adb/zeromount/.detection.json";

/// Run full detection pipeline and return the result.
/// DET01: Determines one of 4 scenarios based on kernel + SUSFS probes.
/// DET02+DET03: Kernel probe + three-layer SUSFS probe.
pub fn detect_scenario() -> Result<DetectionResult> {
    let vfs = kernel::probe_vfs_driver()?;
    let susfs_caps = susfs::probe_susfs()?;

    debug!(
        vfs_driver = vfs.vfs_driver,
        susfs_available = susfs_caps.susfs_available,
        susfs_kstat = susfs_caps.susfs_kstat,
        susfs_path = susfs_caps.susfs_path,
        "probe results"
    );

    // DET01: Scenario selection — SUSFS is independent of VFS driver
    let scenario = match (vfs.vfs_driver, susfs_caps.susfs_available) {
        (true, true) if susfs_caps.susfs_kstat && susfs_caps.susfs_path => Scenario::Full,
        (true, true) => Scenario::SusfsFrontend,
        (true, false) => Scenario::KernelOnly,
        (false, true) => Scenario::SusfsOnly,
        (false, false) => Scenario::None,
    };

    info!(scenario = ?scenario, "detection complete");

    // Merge capabilities from both probes
    let capabilities = CapabilityFlags {
        vfs_driver: vfs.vfs_driver,
        vfs_version: vfs.vfs_version,
        vfs_status_ioctl: vfs.vfs_status_ioctl,
        susfs_available: susfs_caps.susfs_available,
        susfs_version: susfs_caps.susfs_version,
        susfs_kstat: susfs_caps.susfs_kstat,
        susfs_path: susfs_caps.susfs_path,
        susfs_maps: susfs_caps.susfs_maps,
        susfs_open_redirect: susfs_caps.susfs_open_redirect,
        susfs_kstat_redirect: susfs_caps.susfs_kstat_redirect,
        susfs_open_redirect_all: susfs_caps.susfs_open_redirect_all,
        overlay_supported: vfs.overlay_supported,
        erofs_supported: vfs.erofs_supported,
        tmpfs_xattr: vfs.tmpfs_xattr,
    };

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    Ok(DetectionResult {
        scenario,
        capabilities,
        driver_version: vfs.vfs_version,
        timestamp,
    })
}

/// Run detection and persist result to JSON for the mount phase.
/// Called by `zeromount detect` (from post-fs-data.sh).
pub fn detect_and_persist() -> Result<DetectionResult> {
    let result = detect_scenario()?;

    let json = serde_json::to_string_pretty(&result)
        .context("serializing detection result")?;

    // Ensure parent directory exists
    let path = Path::new(DETECTION_JSON_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .context("creating zeromount data directory")?;
    }

    std::fs::write(path, &json)
        .context("writing detection JSON")?;

    debug!("detection result written to {DETECTION_JSON_PATH}");
    Ok(result)
}

/// Read persisted detection result from JSON.
/// Used by the mount phase to avoid re-probing.
pub fn load_detection() -> Result<DetectionResult> {
    let path = Path::new(DETECTION_JSON_PATH);
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading {DETECTION_JSON_PATH} -- was `zeromount detect` run first?"))?;
    let result: DetectionResult = serde_json::from_str(&content)
        .context("parsing detection JSON")?;
    Ok(result)
}
