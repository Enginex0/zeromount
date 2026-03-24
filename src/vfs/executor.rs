use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::core::types::{
    ModuleFileType, MountPlan, MountResult, MountStrategy, ScannedModule,
};
use crate::susfs::SusfsClient;
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

        let susfs = SusfsClient::probe()
            .ok()
            .filter(|c| c.is_available() && c.features().path);

        if susfs.is_some() {
            info!("SUSFS path hiding available for whiteout processing");
        }

        // Phase 1: Inject all rules for all modules
        info!(modules = modules.len(), "phase 1: injecting VFS rules");
        for module in modules {
            let result = self.inject_module_rules(module, susfs.as_ref());
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

    fn ensure_ancestor_dirs(
        &self,
        relative: &Path,
        module: &ScannedModule,
        injected_dirs: &mut HashSet<PathBuf>,
        mount_paths: &mut Vec<String>,
    ) -> (u32, u32) {
        let mut applied = 0u32;
        let mut failed = 0u32;
        let mut missing = Vec::new();

        let mut ancestor = relative.parent();
        while let Some(rel) = ancestor {
            if rel.as_os_str().is_empty() {
                break;
            }
            let target = match resolve_target_path(rel) {
                Some(t) => t,
                None => break,
            };
            if injected_dirs.contains(&target) || target.exists() {
                break;
            }
            missing.push((rel.to_path_buf(), target));
            ancestor = rel.parent();
        }

        for (rel, target) in missing.into_iter().rev() {
            let source = module.path.join(&rel);
            if !source.exists() {
                continue;
            }
            if let Err(e) = self.driver.add_rule(&target, &source, true) {
                debug!(
                    module = %module.id,
                    target = %target.display(),
                    error = %e,
                    "ancestor dir rule failed"
                );
                failed += 1;
                continue;
            }
            injected_dirs.insert(target.clone());
            mount_paths.push(target.display().to_string());
            applied += 1;
        }

        (applied, failed)
    }

    fn inject_module_rules(
        &self,
        module: &ScannedModule,
        susfs: Option<&SusfsClient>,
    ) -> MountResult {
        let mut applied = 0u32;
        let mut failed = 0u32;
        let mut mount_paths = Vec::new();
        let mut error = None;
        let mut injected_dirs = HashSet::new();

        for file in &module.files {
            // Whiteouts → hide the original path via SUSFS
            if matches!(
                file.file_type,
                ModuleFileType::WhiteoutCharDev
                    | ModuleFileType::WhiteoutXattr
                    | ModuleFileType::WhiteoutAufs
            ) {
                let Some(client) = susfs else {
                    // No SUSFS: whiteouts can't be processed in VFS mode
                    if failed == 0 {
                        warn!(
                            module = %module.id,
                            "SUSFS unavailable, whiteouts will not be hidden in VFS mode"
                        );
                    }
                    failed += 1;
                    continue;
                };

                let relative = whiteout_target_relative(&file.relative_path, &file.file_type);
                let target = match resolve_target_path(&relative) {
                    Some(t) => t,
                    None => {
                        failed += 1;
                        continue;
                    }
                };
                let target_str = target.display().to_string();
                match client.add_sus_path(&target_str) {
                    Ok(()) => {
                        applied += 1;
                        mount_paths.push(target_str);
                    }
                    Err(e) => {
                        debug!(
                            module = %module.id,
                            target = %target_str,
                            error = %e,
                            "SUSFS hide failed for whiteout"
                        );
                        failed += 1;
                        if error.is_none() {
                            error = Some(format!("SUSFS hide failed: {e}"));
                        }
                    }
                }
                continue;
            }

            // VFS redirect: regular files, symlinks, redirect xattrs
            match file.file_type {
                ModuleFileType::Regular
                | ModuleFileType::Symlink
                | ModuleFileType::RedirectXattr => {}
                ModuleFileType::Directory => {
                    let source = module.path.join(&file.relative_path);
                    let target = match resolve_target_path(&file.relative_path) {
                        Some(t) => t,
                        None => continue,
                    };
                    if target.exists() {
                        injected_dirs.insert(target);
                        continue;
                    }
                    let (a, f) = self.ensure_ancestor_dirs(
                        &file.relative_path, module, &mut injected_dirs, &mut mount_paths,
                    );
                    applied += a;
                    failed += f;
                    match self.driver.add_rule(&target, &source, true) {
                        Ok(()) => {
                            injected_dirs.insert(target.clone());
                            applied += 1;
                            mount_paths.push(target.display().to_string());
                        }
                        Err(e) => {
                            debug!(
                                module = %module.id,
                                target = %target.display(),
                                error = %e,
                                "dir rule failed"
                            );
                            failed += 1;
                            if error.is_none() {
                                error = Some(format!("dir rule failure: {e}"));
                            }
                        }
                    }
                    continue;
                }
                ModuleFileType::OpaqueDir => {
                    let source = module.path.join(&file.relative_path);
                    if let Some(target) = resolve_target_path(&file.relative_path) {
                        let (a, f) = self.ensure_ancestor_dirs(
                            &file.relative_path, module, &mut injected_dirs, &mut mount_paths,
                        );
                        applied += a;
                        failed += f;
                        match self.driver.add_rule(&target, &source, true) {
                            Ok(()) => {
                                applied += 1;
                                mount_paths.push(target.display().to_string());
                            }
                            Err(e) => {
                                debug!(
                                    module = %module.id,
                                    target = %target.display(),
                                    error = %e,
                                    "opaque dir redirect failed"
                                );
                                failed += 1;
                            }
                        }
                    }
                    continue;
                }
                _ => continue,
            }

            let source = module.path.join(&file.relative_path);
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

            let (a, f) = self.ensure_ancestor_dirs(
                &file.relative_path, module, &mut injected_dirs, &mut mount_paths,
            );
            applied += a;
            failed += f;

            match self.driver.add_rule(&target, &source, false) {
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

}

// AUFS whiteouts encode the target name as `.wh.<name>`. Strip the prefix
// to recover the real path to hide. CharDev/Xattr whiteouts use the target
// name directly.
fn whiteout_target_relative(relative: &Path, file_type: &ModuleFileType) -> PathBuf {
    if *file_type == ModuleFileType::WhiteoutAufs {
        if let Some(name) = relative.file_name().and_then(|n| n.to_str()) {
            if let Some(real_name) = name.strip_prefix(".wh.") {
                if let Some(parent) = relative.parent() {
                    return parent.join(real_name);
                }
            }
        }
    }
    relative.to_path_buf()
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

    #[test]
    fn whiteout_target_chardev_unchanged() {
        let rel = PathBuf::from("product/app/YouTube");
        assert_eq!(
            whiteout_target_relative(&rel, &ModuleFileType::WhiteoutCharDev),
            PathBuf::from("product/app/YouTube")
        );
    }

    #[test]
    fn whiteout_target_xattr_unchanged() {
        let rel = PathBuf::from("system/app/Gmail");
        assert_eq!(
            whiteout_target_relative(&rel, &ModuleFileType::WhiteoutXattr),
            PathBuf::from("system/app/Gmail")
        );
    }

    #[test]
    fn whiteout_target_aufs_strips_prefix() {
        let rel = PathBuf::from("system/app/.wh.Gmail");
        assert_eq!(
            whiteout_target_relative(&rel, &ModuleFileType::WhiteoutAufs),
            PathBuf::from("system/app/Gmail")
        );
    }

    #[test]
    fn whiteout_target_aufs_sar_path() {
        let rel = PathBuf::from("system/product/app/.wh.YouTube");
        let target_rel = whiteout_target_relative(&rel, &ModuleFileType::WhiteoutAufs);
        assert_eq!(target_rel, PathBuf::from("system/product/app/YouTube"));
        assert_eq!(
            resolve_target_path(&target_rel),
            Some(PathBuf::from("/product/app/YouTube"))
        );
    }

    #[test]
    fn whiteout_target_aufs_no_prefix_noop() {
        let rel = PathBuf::from("system/app/NormalFile");
        assert_eq!(
            whiteout_target_relative(&rel, &ModuleFileType::WhiteoutAufs),
            PathBuf::from("system/app/NormalFile")
        );
    }
}
