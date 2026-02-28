use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, warn};

use crate::core::types::{CapabilityFlags, ExternalSusfsModule, SusfsMode};
use crate::susfs::SusfsClient;
use crate::utils::platform;

/// DET03: Kernel-first SUSFS probe.
///
/// Probes the kernel supercall to determine SUSFS availability and feature set.
pub fn probe_susfs() -> Result<CapabilityFlags> {
    let mut caps = CapabilityFlags::default();

    let binary = find_susfs_binary();
    match &binary {
        Some(p) => debug!("SUSFS binary found at: {}", p.display()),
        None => debug!("SUSFS binary not found (kernel probe still proceeds)"),
    }

    // Kernel probe FIRST — ground truth
    let kernel_has_susfs = match SusfsClient::probe() {
        Ok(client) if client.is_available() => {
            caps.susfs_available = true;
            caps.susfs_version = client.version().map(String::from);

            let features = client.features();
            caps.susfs_kstat = features.kstat;
            caps.susfs_path = features.path;
            caps.susfs_maps = features.maps;
            caps.susfs_kstat_redirect = features.kstat_redirect;

            debug!(
                "SUSFS capabilities: kstat={}, path={}, maps={}, kstat_redirect={}",
                caps.susfs_kstat, caps.susfs_path, caps.susfs_maps,
                caps.susfs_kstat_redirect
            );
            true
        }
        Ok(_) => {
            debug!("SUSFS kernel supercall not responding");
            false
        }
        Err(e) => {
            warn!("SUSFS probe failed: {e}");
            false
        }
    };

    let external_module = detect_external_module();
    let binary_found = binary.is_some();

    caps.external_susfs_module = external_module;
    caps.susfs_binary_found = binary_found;

    // SukiSU/ReSukiSU bundle ksu_susfs in the manager — no external module needed
    let sukisu = std::env::var("KSU_SUKISU").map(|v| v == "true").unwrap_or(false);

    caps.susfs_mode = if kernel_has_susfs
        && (external_module != ExternalSusfsModule::None || (sukisu && binary_found))
    {
        SusfsMode::Enhanced
    } else if kernel_has_susfs {
        SusfsMode::Embedded
    } else {
        SusfsMode::Absent
    };

    debug!(
        "SUSFS mode: {:?} (kernel={}, external={:?}, binary={}, sukisu={})",
        caps.susfs_mode, kernel_has_susfs, external_module, binary_found, sukisu
    );

    Ok(caps)
}

fn module_is_active(module_dir: &Path) -> bool {
    module_dir.exists()
        && !module_dir.join("disable").exists()
        && !module_dir.join("remove").exists()
}

fn detect_susfs4ksu_active() -> bool {
    let base = Path::new("/data/adb/modules");
    ["susfs4ksu", "susfs4ksu_next"]
        .iter()
        .any(|id| module_is_active(&base.join(id)))
}

fn detect_brene_active() -> bool {
    module_is_active(Path::new("/data/adb/modules/brene"))
}

// susfs4ksu wins if both somehow active (BRENE disables susfs4ksu on install)
fn detect_external_module() -> ExternalSusfsModule {
    if detect_susfs4ksu_active() {
        ExternalSusfsModule::Susfs4ksu
    } else if detect_brene_active() {
        ExternalSusfsModule::Brene
    } else {
        ExternalSusfsModule::None
    }
}

/// Locate the SUSFS binary by searching platform-specific paths.
/// DET03 layer 2: search order per RootManager::susfs_binary_paths().
fn find_susfs_binary() -> Option<PathBuf> {
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
