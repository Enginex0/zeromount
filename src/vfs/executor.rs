use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::core::types::{
    ModuleFileType, MountPlan, MountResult, MountStrategy, ScannedModule,
};
use super::VfsDriver;

pub struct VfsExecutor {
    driver: VfsDriver,
}

impl VfsExecutor {
    pub fn new(driver: VfsDriver) -> Self {
        Self { driver }
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

        // Phase 3: Enable engine
        info!("phase 3: enabling VFS engine");
        self.driver.enable().context("failed to enable VFS engine")?;

        // Phase 4: Refresh dcache
        info!("phase 4: refreshing dcache");
        self.driver.refresh().context("failed to refresh dcache")?;

        Ok(results)
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

            // Directory rules redirect opendir() to the module's directory, hiding
            // stock content (GPU drivers, linker libs). The kernel's auto_inject_parent()
            // handles directory visibility in readdir via dirs_ht — no rule needed.
            if is_dir {
                continue;
            }

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

            match self.driver.add_rule(&target, &source, is_dir) {
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

        let (strategy, success, rules) =
            (MountStrategy::Vfs, failed == 0 && applied > 0, applied);

        debug!(
            module = %module.id,
            applied,
            failed,
            "module rule injection done"
        );

        MountResult {
            module_id: module.id.clone(),
            strategy_used: strategy,
            success,
            rules_applied: rules,
            rules_failed: failed,
            error,
            mount_paths,
        }
    }

}

const BRENE_OWNED_TARGET_PREFIXES: &[&str] = &["/system/fonts/"];

pub(crate) fn is_brene_owned_target(target: &Path) -> bool {
    let s = match target.to_str() {
        Some(s) => s,
        None => return false,
    };
    BRENE_OWNED_TARGET_PREFIXES
        .iter()
        .any(|prefix| s.starts_with(prefix) || s == &prefix[..prefix.len() - 1])
}

// On SAR (System-as-Root) Android, /system/vendor is a symlink to /vendor.
// VFS rules targeting /system/vendor/... create dirs_ht entries sharing inodes
// with /vendor/..., corrupting directory lookups for GPU drivers and other
// critical partition content. Canonicalize these alias paths upfront.
const SAR_ALIAS_PARTITIONS: &[&str] = &[
    "system/vendor",
    "system/product",
    "system/system_ext",
    "system/odm",
];

/// Resolve a module's relative path to the absolute filesystem target,
/// canonicalizing SAR alias paths (system/vendor → /vendor, etc.).
pub(crate) fn resolve_target_path(relative: &Path) -> Option<PathBuf> {
    let s = relative.to_str()?;
    if s.is_empty() {
        return None;
    }
    for alias in SAR_ALIAS_PARTITIONS {
        let canonical = &alias["system/".len()..];
        if s == *alias {
            return Some(PathBuf::from(format!("/{canonical}")));
        }
        if let Some(rest) = s.strip_prefix(alias).and_then(|r| r.strip_prefix('/')) {
            return Some(PathBuf::from(format!("/{canonical}/{rest}")));
        }
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

    #[test]
    fn resolve_target_sar_system_vendor_file() {
        let rel = PathBuf::from("system/vendor/lib64/soundfx/libv4a_fx.so");
        assert_eq!(
            resolve_target_path(&rel),
            Some(PathBuf::from("/vendor/lib64/soundfx/libv4a_fx.so"))
        );
    }

    #[test]
    fn resolve_target_sar_system_vendor_bare_dir() {
        let rel = PathBuf::from("system/vendor");
        assert_eq!(resolve_target_path(&rel), Some(PathBuf::from("/vendor")));
    }

    #[test]
    fn resolve_target_sar_system_product() {
        let rel = PathBuf::from("system/product/app/SomeApp.apk");
        assert_eq!(
            resolve_target_path(&rel),
            Some(PathBuf::from("/product/app/SomeApp.apk"))
        );
    }

    #[test]
    fn resolve_target_sar_system_ext() {
        let rel = PathBuf::from("system/system_ext/lib/libfoo.so");
        assert_eq!(
            resolve_target_path(&rel),
            Some(PathBuf::from("/system_ext/lib/libfoo.so"))
        );
    }

    #[test]
    fn resolve_target_non_sar_system_paths_unaffected() {
        // system/bin, system/app, system/etc etc. must remain under /system/
        assert_eq!(
            resolve_target_path(&PathBuf::from("system/bin/ls")),
            Some(PathBuf::from("/system/bin/ls"))
        );
        assert_eq!(
            resolve_target_path(&PathBuf::from("system/etc/audio_effects.conf")),
            Some(PathBuf::from("/system/etc/audio_effects.conf"))
        );
    }

    #[test]
    fn brene_owns_system_fonts() {
        assert!(is_brene_owned_target(Path::new("/system/fonts/Roboto.ttf")));
        assert!(is_brene_owned_target(Path::new("/system/fonts/")));
        assert!(is_brene_owned_target(Path::new("/system/fonts")));
        assert!(is_brene_owned_target(Path::new("/system/fonts/NotoEmoji.ttc")));
    }

    #[test]
    fn brene_does_not_own_non_font_paths() {
        assert!(!is_brene_owned_target(Path::new("/system/bin/ls")));
        assert!(!is_brene_owned_target(Path::new("/vendor/fonts/custom.ttf")));
        assert!(!is_brene_owned_target(Path::new("/system/app/SomeApp.apk")));
        assert!(!is_brene_owned_target(Path::new("")));
    }
}
