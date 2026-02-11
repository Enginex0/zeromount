use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::core::types::{
    ModuleFileType, MountPlan, MountResult, MountStrategy, ScannedModule,
};
use crate::susfs::kstat::apply_kstat_redirect_or_static;
use crate::susfs::SusfsClient;

use super::VfsDriver;

/// VFS mount executor -- injects rules, applies SUSFS protections, enables engine.
///
/// Pipeline ordering (fixes BUG-M3 / CO03):
///   1. INJECT  -- add_rule for each module file
///   2. SUSFS   -- kstat spoof + path hide per injected file
///   3. ENABLE  -- activate the VFS engine
///   4. REFRESH -- force dcache update
pub struct VfsExecutor {
    driver: VfsDriver,
    susfs: Option<SusfsClient>,
}

impl VfsExecutor {
    pub fn new(driver: VfsDriver, susfs: Option<SusfsClient>) -> Self {
        Self { driver, susfs }
    }

    /// Execute the full VFS pipeline for a set of scanned modules.
    pub fn execute(
        &self,
        _plan: &MountPlan,
        modules: &[ScannedModule],
    ) -> Result<Vec<MountResult>> {
        let mut results = Vec::with_capacity(modules.len());

        // Phase 1: Inject all rules for all modules
        info!(modules = modules.len(), "phase 1: injecting VFS rules");
        for module in modules {
            let result = self.inject_module_rules(module);
            results.push(result);
        }

        let total_applied: u32 = results.iter().map(|r| r.rules_applied).sum();
        let total_failed: u32 = results.iter().map(|r| r.rules_failed).sum();
        info!(applied = total_applied, failed = total_failed, "rule injection complete");

        // Phase 2: SUSFS protections (if available)
        if let Some(ref susfs) = self.susfs {
            if susfs.is_available() {
                info!("phase 2: applying SUSFS protections");
                for module in modules {
                    self.apply_susfs_protections(susfs, module);
                }
            } else {
                debug!("phase 2: SUSFS not available, skipping protections");
            }
        } else {
            debug!("phase 2: no SUSFS client, skipping protections");
        }

        // Phase 3: Enable engine
        info!("phase 3: enabling VFS engine");
        self.driver.enable().context("failed to enable VFS engine")?;

        // Phase 4: Refresh dcache
        info!("phase 4: refreshing dcache");
        self.driver.refresh().context("failed to refresh dcache")?;

        Ok(results)
    }

    /// Hot-reload: CLEAR_ALL + re-inject all rules.
    ///
    /// CO01: Both del_rule AND clear_all leak dirs_ht entries. Hot-reload uses
    /// CLEAR_ALL + full re-inject, which is acceptable since hot-reload is rare.
    /// CO03: CLEAR_ALL + re-inject eliminates stale ghost directory entries.
    #[allow(dead_code)] // Wired when watcher triggers VFS re-inject
    pub fn hot_reload(
        &self,
        plan: &MountPlan,
        modules: &[ScannedModule],
    ) -> Result<Vec<MountResult>> {
        info!("hot-reload: clearing all rules before re-inject");

        // Disable engine before modifying rules -- bail if this fails to avoid
        // clearing rules from a still-active engine (detection window)
        self.driver.disable().context("disable before hot-reload failed")?;

        self.driver.clear_all().context("CLEAR_ALL failed during hot-reload")?;

        // Re-run the full pipeline
        self.execute(plan, modules)
    }

    /// Inject VFS rules for a single module. Returns per-module result.
    fn inject_module_rules(&self, module: &ScannedModule) -> MountResult {
        let mut applied = 0u32;
        let mut failed = 0u32;
        let mut mount_paths = Vec::new();
        let mut error = None;

        for file in &module.files {
            // Skip whiteouts, opaque dirs, and other non-regular types for VFS rules.
            // VFS redirection only applies to regular files, directories, and symlinks.
            match file.file_type {
                ModuleFileType::Regular
                | ModuleFileType::Directory
                | ModuleFileType::Symlink
                | ModuleFileType::RedirectXattr => {}
                _ => continue,
            }

            let is_dir = file.file_type == ModuleFileType::Directory;

            // source: the module's file on disk
            let source = module.path.join(&file.relative_path);

            // target: where it should appear in the real filesystem (strip the partition prefix)
            let target = match resolve_target_path(&file.relative_path) {
                Some(t) => t,
                None => {
                    debug!(
                        module = %module.id,
                        path = %file.relative_path.display(),
                        "cannot resolve target path, skipping"
                    );
                    failed += 1;
                    continue;
                }
            };

            match self.driver.add_rule(&source, &target, is_dir) {
                Ok(()) => {
                    applied += 1;
                    mount_paths.push(target.display().to_string());
                }
                Err(e) => {
                    debug!(
                        module = %module.id,
                        source = %source.display(),
                        target = %target.display(),
                        error = %e,
                        "add_rule failed"
                    );
                    failed += 1;
                    if error.is_none() {
                        error = Some(format!("first rule failure: {e}"));
                    }
                }
            }
        }

        debug!(
            module = %module.id,
            applied,
            failed,
            "module rule injection done"
        );

        MountResult {
            module_id: module.id.clone(),
            strategy_used: MountStrategy::Vfs,
            success: failed == 0 && applied > 0,
            rules_applied: applied,
            rules_failed: failed,
            error,
            mount_paths,
        }
    }

    /// Apply SUSFS protections for a module's injected files.
    /// Phase 2: kstat spoofing + path hiding.
    fn apply_susfs_protections(&self, susfs: &SusfsClient, module: &ScannedModule) {
        apply_module_susfs_protections(susfs, module);
    }
}

/// Apply SUSFS kstat spoofing + path hiding for a single module's files.
///
/// This is the standalone entry point used by both the VFS pipeline (via
/// `VfsExecutor::apply_susfs_protections`) and the CLI deferred-retry path.
pub fn apply_module_susfs_protections(susfs: &SusfsClient, module: &ScannedModule) {
    let features = susfs.features();

    for file in &module.files {
        match file.file_type {
            ModuleFileType::Regular
            | ModuleFileType::Directory
            | ModuleFileType::Symlink
            | ModuleFileType::RedirectXattr => {}
            _ => continue,
        }

        let source = module.path.join(&file.relative_path);
        let target = match resolve_target_path(&file.relative_path) {
            Some(t) => t,
            None => continue,
        };

        let source_str = source.display().to_string();
        let target_str = target.display().to_string();

        // Kstat spoofing: make stat() on the VFS-redirected path return
        // the original file's metadata instead of the module file's metadata
        if features.kstat {
            if let Err(e) = apply_kstat_redirect_or_static(
                susfs,
                &target_str,
                &source_str,
            ) {
                debug!(
                    module = %module.id,
                    target = %target_str,
                    error = %e,
                    "kstat spoofing failed"
                );
            }
        }

        // Path hiding: hide the module source path so it doesn't appear
        // in directory listings
        if features.path {
            if let Err(e) = susfs.add_sus_path(&source_str) {
                debug!(
                    module = %module.id,
                    path = %source_str,
                    error = %e,
                    "path hiding failed"
                );
            }
        }
    }
}

/// Resolve a module's relative path (e.g. system/bin/foo) to the absolute
/// filesystem target (e.g. /system/bin/foo).
///
/// The relative_path from ScannedModule starts with the partition name
/// (system, vendor, etc.), so we prepend "/" to get the real path.
fn resolve_target_path(relative: &Path) -> Option<PathBuf> {
    let s = relative.to_str()?;
    if s.is_empty() {
        return None;
    }
    Some(PathBuf::from(format!("/{s}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_target_system_bin() {
        let rel = PathBuf::from("system/bin/foo");
        let target = resolve_target_path(&rel);
        assert_eq!(target, Some(PathBuf::from("/system/bin/foo")));
    }

    #[test]
    fn resolve_target_vendor_lib() {
        let rel = PathBuf::from("vendor/lib64/libbar.so");
        let target = resolve_target_path(&rel);
        assert_eq!(target, Some(PathBuf::from("/vendor/lib64/libbar.so")));
    }

    #[test]
    fn resolve_target_empty_returns_none() {
        let rel = PathBuf::from("");
        assert_eq!(resolve_target_path(&rel), None);
    }
}
