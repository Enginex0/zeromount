use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, warn};

use crate::core::types::CapabilityFlags;
use crate::susfs::SusfsClient;
use crate::utils::platform;

/// SUSFS module directory names (checked under /data/adb/modules/)
const SUSFS_MODULE_IDS: &[&str] = &["susfs4ksu", "susfs"];

/// DET03: Two-phase SUSFS probe.
///
/// 1. Module state -- check for .disabled marker in SUSFS module dir.
///    If disabled, skip all SUSFS operations regardless of kernel state.
/// 2. Kernel probe -- SusfsClient::probe() queries standard features
///    (via show_enabled_features) and custom commands (kstat_redirect,
///    open_redirect_all) via supercall. Binary presence is logged but
///    does not gate detection.
pub fn probe_susfs() -> Result<CapabilityFlags> {
    let mut caps = CapabilityFlags::default();

    // Layer 1: Module state check
    if is_susfs_module_disabled() {
        debug!("SUSFS module disabled via .disabled marker, skipping");
        return Ok(caps);
    }

    // Binary location -- useful for CLI operations but not required for
    // kernel-level detection (SusfsClient::probe uses supercalls directly)
    let binary = find_susfs_binary();
    match &binary {
        Some(p) => debug!("SUSFS binary found at: {}", p.display()),
        None => debug!("SUSFS binary not found (kernel probe still proceeds)"),
    }

    // Layer 2+3: SusfsClient probes both standard features
    // (via show_enabled_features) and custom commands (via supercall probe)
    match SusfsClient::probe() {
        Ok(client) => {
            if !client.is_available() {
                debug!("SUSFS kernel supercall not responding");
                return Ok(caps);
            }

            caps.susfs_available = true;
            caps.susfs_version = client.version().map(String::from);

            let features = client.features();
            caps.susfs_kstat = features.kstat;
            caps.susfs_path = features.path;
            caps.susfs_maps = features.maps;
            caps.susfs_open_redirect = features.open_redirect;

            // Layer 3: Custom kernel ioctls (probed inside SusfsClient::probe)
            caps.susfs_kstat_redirect = features.kstat_redirect;
            caps.susfs_open_redirect_all = features.open_redirect_all;

            debug!(
                "SUSFS capabilities: kstat={}, path={}, maps={}, redirect={}, \
                 kstat_redirect={}, redirect_all={}",
                caps.susfs_kstat, caps.susfs_path, caps.susfs_maps,
                caps.susfs_open_redirect, caps.susfs_kstat_redirect,
                caps.susfs_open_redirect_all
            );
        }
        Err(e) => {
            warn!("SUSFS probe failed: {e}");
        }
    }

    Ok(caps)
}

/// Check whether SUSFS module directory has a .disabled marker.
/// DET03 layer 1: if disabled, all SUSFS operations are skipped.
fn is_susfs_module_disabled() -> bool {
    let modules_dir = Path::new("/data/adb/modules");
    for module_id in SUSFS_MODULE_IDS {
        let module_dir = modules_dir.join(module_id);
        if module_dir.exists() {
            let disabled = module_dir.join("disable");
            if disabled.exists() {
                debug!("SUSFS module {module_id} has 'disable' marker");
                return true;
            }
            // Module dir exists but not disabled -- proceed
            return false;
        }
    }
    // No SUSFS module dir found -- not disabled (may still have binary)
    false
}

/// Locate the SUSFS binary by searching platform-specific paths.
/// DET03 layer 2: search order per RootManager::susfs_binary_paths().
pub fn find_susfs_binary() -> Option<PathBuf> {
    // Try platform-specific paths first
    if let Ok(manager) = platform::detect_root_manager() {
        for path in manager.susfs_binary_paths() {
            if path.exists() && is_executable(&path) {
                return Some(path);
            }
        }
    }

    // Fallback: check common paths
    let fallback_paths = [
        "/data/adb/ksu/bin/ksu_susfs",
        "/data/adb/ap/bin/ksu_susfs",
        "/data/adb/ksu/bin/susfs",
        "/data/adb/modules/meta-zeromount/ksu_susfs",
    ];

    for path in &fallback_paths {
        let p = Path::new(path);
        if p.exists() && is_executable(p) {
            return Some(p.to_path_buf());
        }
    }

    None
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
