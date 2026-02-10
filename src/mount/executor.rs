use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::core::config::MountConfig;
use crate::core::types::{
    CapabilityFlags, MountPlan, MountResult, MountStrategy, RootMountMode, ScannedModule,
};

use super::magic::mount_magic;
use super::overlay::mount_overlay;
use super::storage::{cleanup_storage, init_storage, nuke_backing_file};

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
        MountStrategy::Vfs => {
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
    let mut storage = init_storage(capabilities, mount_config).context("storage init for overlay failed")?;

    let module_map: std::collections::HashMap<&str, &ScannedModule> =
        modules.iter().map(|m| (m.id.as_str(), m)).collect();

    let mut results = Vec::new();

    for pm in &plan.partition_mounts {
        let mut lower_dirs: Vec<PathBuf> = Vec::new();

        for mod_id in &pm.contributing_modules {
            let lower = storage.lower_dir(mod_id, &pm.partition);
            if let Some(scanned) = module_map.get(mod_id.as_str()) {
                // Copy module files into the lower dir for overlay
                if let Err(e) = prepare_lower_dir(scanned, &pm.partition, &lower) {
                    warn!(module = %mod_id, error = %e, "failed to prepare lower dir");
                    continue;
                }
                lower_dirs.push(lower);
            }
        }

        let lower_refs: Vec<&std::path::Path> =
            lower_dirs.iter().map(|p| p.as_path()).collect();
        let work = storage.work_dir(&pm.mount_point.to_string_lossy());
        let target = &pm.mount_point;

        let mount_id = pm.contributing_modules.join("+");
        let result = mount_overlay(&lower_refs, &work, target, &mount_id, &storage.overlay_source)?;

        // ME12: nuke backing file after successful mount — kernel keeps inode alive
        if result.success {
            if let Err(e) = nuke_backing_file(&storage.base_path) {
                warn!(error = %e, "nuke backing file failed (non-fatal)");
            }
        }

        results.push(result);
    }

    // Explicit cleanup before drop — prevents double-cleanup in Drop impl
    if let Err(e) = cleanup_storage(&mut storage) {
        debug!(error = %e, "storage cleanup failed (non-fatal)");
    }

    info!(mounts = results.len(), "overlay execution complete");
    Ok(results)
}

fn execute_magic_mount(
    modules: &[ScannedModule],
    capabilities: &CapabilityFlags,
    mount_config: &MountConfig,
) -> Result<Vec<MountResult>> {
    let mut storage = init_storage(capabilities, mount_config).context("storage init for magic mount failed")?;

    let mut results = Vec::new();
    for module in modules {
        let result = mount_magic(module, &storage.base_path)?;
        results.push(result);
    }

    if let Err(e) = cleanup_storage(&mut storage) {
        debug!(error = %e, "storage cleanup failed (non-fatal)");
    }

    info!(modules = results.len(), "magic mount execution complete");
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
            crate::utils::selinux::mirror_selinux_context(&src, &dst);
        } else {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }
            if src.exists() {
                fs::copy(&src, &dst).with_context(|| {
                    format!("copy {} -> {}", src.display(), dst.display())
                })?;
                crate::utils::selinux::mirror_selinux_context(&src, &dst);
            }
        }
    }

    Ok(())
}

// Prevent double-mounting when root manager also mounts on the same paths.
// Creates skip_mount in each module dir so the root manager skips its own mount.
// Tracks flagged modules for cleanup on uninstall.
pub fn manage_skip_mount_flags(modules: &[ScannedModule], mode: RootMountMode) {
    let modules_base = Path::new("/data/adb/modules");
    let mut flagged = Vec::new();

    for module in modules {
        let flag = modules_base.join(&module.id).join("skip_mount");
        match mode {
            RootMountMode::Metamodule => {
                let _ = std::fs::remove_file(&flag);
            }
            RootMountMode::BindMount => {
                let _ = std::fs::write(&flag, "");
                flagged.push(module.id.as_str());
            }
        }
    }

    if !flagged.is_empty() {
        let tracking = Path::new("/data/adb/zeromount/.skipped_modules");
        let content: String = flagged.iter().map(|id| format!("{id}\n")).collect();
        let _ = std::fs::write(tracking, content);
    }
}
