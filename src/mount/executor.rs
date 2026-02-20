use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::core::config::MountConfig;
use crate::core::types::{
    CapabilityFlags, MountPlan, MountResult, MountStrategy, PartitionMount, RootMountMode,
    ScannedModule,
};

use super::magic::mount_magic;
use super::overlay::mount_overlay;
use super::storage::init_storage;
use super::decoy;

pub fn execute_plan(
    plan: &MountPlan,
    modules: &[ScannedModule],
    strategy: MountStrategy,
    capabilities: &CapabilityFlags,
    mount_config: &MountConfig,
) -> Result<Vec<MountResult>> {
    match strategy {
        MountStrategy::Overlay => execute_overlay(plan, modules, capabilities, mount_config),
        MountStrategy::MagicMount => execute_magic_mount(modules, capabilities, mount_config),
        MountStrategy::Vfs | MountStrategy::Font => {
            Ok(Vec::new())
        }
    }
}

fn execute_overlay(
    plan: &MountPlan,
    modules: &[ScannedModule],
    capabilities: &CapabilityFlags,
    mount_config: &MountConfig,
) -> Result<Vec<MountResult>> {
    let mut storage = init_storage(capabilities, mount_config)
        .context("storage init for overlay failed")?;

    // Prevent mount events from propagating to child namespaces
    {
        let c_path = std::ffi::CString::new(
            storage.base_path.to_string_lossy().as_bytes().to_vec()
        ).context("base_path contains null byte")?;
        // SAFETY: CString is non-null NUL-terminated; null pointers for unused mount(2) args are valid.
        let ret = unsafe {
            libc::mount(
                std::ptr::null(),
                c_path.as_ptr(),
                std::ptr::null(),
                libc::MS_PRIVATE,
                std::ptr::null(),
            )
        };
        if ret != 0 {
            warn!(error = %std::io::Error::last_os_error(), "MS_PRIVATE failed (non-fatal)");
        }
    }

    // Set up decoy lowerdir for detection evasion
    let decoy = decoy::setup_decoy();

    let module_map: std::collections::HashMap<&str, &ScannedModule> =
        modules.iter().map(|m| (m.id.as_str(), m)).collect();

    // Phase 1: Stage lower dirs directly (no .tmp_ rename — the two-phase
    // approach already guarantees no mounts happen until all staging succeeds).
    let mut staged: Vec<(&PartitionMount, Vec<PathBuf>)> = Vec::new();

    for pm in &plan.partition_mounts {
        let mut lower_dirs: Vec<PathBuf> = Vec::new();

        for mod_id in &pm.contributing_modules {
            let lower = storage.lower_dir(mod_id, &pm.partition);

            if let Some(scanned) = module_map.get(mod_id.as_str()) {
                if let Err(e) = prepare_lower_dir(scanned, &pm.partition, &lower) {
                    warn!(module = %mod_id, error = %e, "staging failed");
                    anyhow::bail!("overlay staging failed for module {mod_id}: {e}");
                }
                lower_dirs.push(lower);
            }
        }

        staged.push((pm, lower_dirs));
    }

    // Phase 2: All staging succeeded — mount overlays.
    // Lower dirs are partition-level (e.g., .../viperfxmod/system/) but mount points
    // may be subdirectories (e.g., /system/etc). Append the relative suffix so overlay
    // only exposes files belonging to that mount point.
    let mut results = Vec::new();

    for (pm, lower_dirs) in &staged {
        let adjusted: Vec<PathBuf> = lower_dirs
            .iter()
            .map(|d| if pm.staging_rel.as_os_str().is_empty() { d.clone() } else { d.join(&pm.staging_rel) })
            .filter(|d| d.exists())
            .collect();

        if adjusted.is_empty() {
            continue;
        }

        let lower_refs: Vec<&std::path::Path> =
            adjusted.iter().map(|p| p.as_path()).collect();
        let target = &pm.mount_point;
        let mount_id = pm.contributing_modules.join("+");

        // Compute decoy subdir for this mount target
        let decoy_subdir = decoy.as_ref().map(|d| {
            let sub = d.join(target.strip_prefix("/").unwrap_or(target));
            let _ = std::fs::create_dir_all(&sub);
            decoy::mirror_decoy_selinux(d, target);
            sub
        });
        let decoy_ref = decoy_subdir.as_deref();

        let result = match mount_overlay(&lower_refs, target, &mount_id, &storage.overlay_source, decoy_ref) {
            Ok(r) => r,
            Err(e) => {
                warn!(target = %target.display(), error = %e, "overlay mount failed");
                MountResult {
                    module_id: mount_id.clone(),
                    strategy_used: MountStrategy::Overlay,
                    success: false,
                    rules_applied: 0,
                    rules_failed: 1,
                    error: Some(format!("{e}")),
                    mount_paths: Vec::new(),
                }
            }
        };
        results.push(result);
    }

    storage.detach_staging();

    // Tear down decoy tmpfs -- overlay keeps inode references alive
    if let Some(ref d) = decoy {
        decoy::teardown_decoy(d);
    }

    info!(mounts = results.len(), "overlay execution complete");
    Ok(results)
}

fn execute_magic_mount(
    modules: &[ScannedModule],
    capabilities: &CapabilityFlags,
    mount_config: &MountConfig,
) -> Result<Vec<MountResult>> {
    let mut storage = init_storage(capabilities, mount_config)
        .context("storage init for magic mount failed")?;

    // Prevent mount events from propagating to child namespaces
    {
        let c_path = std::ffi::CString::new(
            storage.base_path.to_string_lossy().as_bytes().to_vec()
        ).context("base_path contains null byte")?;
        // SAFETY: CString is non-null NUL-terminated; null pointers for unused mount(2) args are valid.
        let ret = unsafe {
            libc::mount(
                std::ptr::null(),
                c_path.as_ptr(),
                std::ptr::null(),
                libc::MS_PRIVATE,
                std::ptr::null(),
            )
        };
        if ret != 0 {
            warn!(error = %std::io::Error::last_os_error(), "MS_PRIVATE failed (non-fatal)");
        }
    }

    let results = mount_magic(modules, &storage.base_path, &storage.overlay_source)?;

    storage.detach_staging();

    info!(mounts = results.len(), "magic mount execution complete");
    Ok(results)
}

/// Copy module files for a specific partition into the overlay lower directory.
fn prepare_lower_dir(
    module: &ScannedModule,
    partition: &str,
    lower_dir: &std::path::Path,
) -> Result<()> {
    use std::fs;

    fs::create_dir_all(lower_dir)
        .with_context(|| format!("cannot create lower dir: {}", lower_dir.display()))?;

    let prefix = format!("{}/", partition);
    for file in &module.files {
        let rel_str = file.relative_path.to_string_lossy();
        if !rel_str.starts_with(&prefix) {
            continue;
        }
        let sub = &rel_str[prefix.len()..];
        if sub.is_empty() {
            continue;
        }

        let src = module.path.join(&file.relative_path);
        let dst = lower_dir.join(sub);

        if src.is_dir() {
            fs::create_dir_all(&dst)?;
            crate::utils::selinux::copy_selinux_context(&src, &dst);
        } else {
            if let Some(parent) = dst.parent() {
                ensure_parent_dirs_with_context(lower_dir, parent, partition)?;
            }
            if src.exists() {
                fs::copy(&src, &dst).with_context(|| {
                    format!("copy {} -> {}", src.display(), dst.display())
                })?;
                crate::utils::selinux::copy_selinux_context(&src, &dst);
            }
        }
    }

    // Mark directories with .replace as opaque in the overlay
    let system_dir = module.path.join(partition);
    if system_dir.is_dir() {
        if let Err(e) = super::opaque::mark_opaque_dirs(&system_dir, lower_dir) {
            warn!(module = %module.id, error = %e, "opaque dir marking failed (non-fatal)");
        }
    }

    Ok(())
}

/// Create intermediate directories one level at a time, mirroring SELinux
/// context from the real filesystem. Prevents tmpfs-default labels on dirs
/// that overlayfs exposes in merged directory listings.
fn ensure_parent_dirs_with_context(
    lower_dir: &std::path::Path,
    target_parent: &std::path::Path,
    partition: &str,
) -> Result<()> {
    use std::fs;

    let rel = match target_parent.strip_prefix(lower_dir) {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };

    let mut current = lower_dir.to_path_buf();
    let partition_root = PathBuf::from(format!("/{}", partition));

    for component in rel.components() {
        current.push(component);
        if !current.exists() {
            fs::create_dir_all(&current)?;
            let real_path = partition_root.join(
                current.strip_prefix(lower_dir).unwrap_or(Path::new("")),
            );
            // copy_selinux_context handles missing real_path: falls back to
            // u:object_r:system_file:s0, preventing module-created dirs from
            // inheriting tmpfs/ext4 default labels that cause AVC denials.
            crate::utils::selinux::copy_selinux_context(&real_path, &current);
        }
    }

    Ok(())
}

// KSU/APatch metamodules own all mounting — skip_mount flags are irrelevant.
// BindMount (Magisk) needs flags so the root manager doesn't double-mount.
pub fn manage_skip_mount_flags(modules: &[ScannedModule], mode: RootMountMode) {
    if mode == RootMountMode::Metamodule {
        return;
    }

    let modules_base = Path::new("/data/adb/modules");
    let mut flagged = Vec::new();

    for module in modules {
        let flag = modules_base.join(&module.id).join("skip_mount");
        let _ = std::fs::write(&flag, "");
        flagged.push(module.id.as_str());
    }

    if !flagged.is_empty() {
        let tracking = Path::new("/data/adb/zeromount/.skipped_modules");
        let content: String = flagged.iter().map(|id| format!("{id}\n")).collect();
        let _ = std::fs::write(tracking, content);
    }
}
