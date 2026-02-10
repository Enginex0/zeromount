use std::fs;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use tracing::{debug, info, warn};

use crate::core::types::{ModuleFile, ModuleFileType, ModuleProp, ScannedModule};
use super::rules::detect_conflicts;

pub const SUPPORTED_PARTITIONS: &[&str] = &[
    "system",
    "vendor",
    "product",
    "system_ext",
    "odm",
    "oem",
    "my_bigball",
    "my_carrier",
    "my_company",
    "my_engineering",
    "my_heytap",
    "my_manifest",
    "my_preload",
    "my_product",
    "my_region",
    "my_stock",
    "mi_ext",
    "cust",
    "optics",
    "prism",
    "oem_dlkm",
    "system_dlkm",
    "vendor_dlkm",
];

const BLACKLISTED_NAMES: &[&str] = &["meta-zeromount", ".", "..", "lost+found"];

/// Scan /data/adb/modules/ for active modules, classify files, detect conflicts.
/// Returns modules sorted reverse-alphabetically (last-installed wins on conflict).
pub fn scan_modules(modules_dir: &Path) -> Result<Vec<ScannedModule>> {
    let entries: Vec<PathBuf> = fs::read_dir(modules_dir)
        .with_context(|| format!("cannot read modules directory: {}", modules_dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => return false,
            };
            !BLACKLISTED_NAMES.contains(&name)
        })
        .filter(|p| is_module_enabled(p) && !has_skip_mount(p))
        .collect();

    // Filesystem corruption can create duplicate directory entries (same inode).
    // Dedup by inode to avoid scanning the same module twice.
    let entries = {
        let mut seen_inodes = std::collections::HashSet::new();
        entries
            .into_iter()
            .filter(|p| {
                std::fs::metadata(p)
                    .map(|m| seen_inodes.insert(m.ino()))
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>()
    };

    let mut modules: Vec<ScannedModule> = entries
        .par_iter()
        .filter_map(|path| match scan_single_module(path) {
            Ok(Some(m)) => Some(m),
            Ok(None) => None,
            Err(e) => {
                warn!(
                    module = %path.display(),
                    error = %e,
                    "failed to scan module, skipping"
                );
                None
            }
        })
        .collect();

    // Reverse-alphabetical: Z before A, so last-installed wins on conflict
    modules.sort_by(|a, b| b.id.cmp(&a.id));

    detect_conflicts(&modules);

    info!(count = modules.len(), "module scan complete");
    Ok(modules)
}

fn scan_single_module(module_dir: &Path) -> Result<Option<ScannedModule>> {
    let id = module_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let prop = parse_module_prop(&module_dir.join("module.prop")).unwrap_or_default();

    let has_service_sh = module_dir.join("service.sh").exists();
    let has_post_fs_data_sh = module_dir.join("post-fs-data.sh").exists();

    let mut files = Vec::new();

    for &partition in SUPPORTED_PARTITIONS {
        let partition_dir = module_dir.join(partition);
        if !partition_dir.is_dir() {
            continue;
        }
        walk_module_tree(&partition_dir, module_dir, &id, &mut files)?;
    }

    if files.is_empty() {
        debug!(module = %id, "module has no mountable files");
        return Ok(None);
    }

    Ok(Some(ScannedModule {
        id,
        path: module_dir.to_path_buf(),
        files,
        has_service_sh,
        has_post_fs_data_sh,
        prop,
    }))
}

/// Recursively walk a module's partition tree and classify each entry.
fn walk_module_tree(
    dir: &Path,
    module_root: &Path,
    module_id: &str,
    files: &mut Vec<ModuleFile>,
) -> Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!(dir = %dir.display(), error = %e, "cannot read directory");
            return Ok(());
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_path_buf();

        let file_type = classify_file(&path);

        files.push(ModuleFile {
            relative_path: relative.clone(),
            file_type: file_type.clone(),
            source_module: module_id.to_string(),
        });

        // Recurse into directories (but not opaque ones -- their contents
        // are irrelevant since the entire subtree is replaced)
        if path.is_dir() && file_type != ModuleFileType::OpaqueDir {
            walk_module_tree(&path, module_root, module_id, files)?;
        }
    }

    Ok(())
}

/// Classify a filesystem entry into a ModuleFileType.
fn classify_file(path: &Path) -> ModuleFileType {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return ModuleFileType::Regular,
    };

    let ft = metadata.file_type();

    if ft.is_symlink() {
        return ModuleFileType::Symlink;
    }

    if ft.is_char_device() {
        // Whiteout char device: major=0, minor=0
        if is_zero_char_device(path) {
            return ModuleFileType::WhiteoutCharDev;
        }
        return ModuleFileType::Regular;
    }

    if ft.is_dir() {
        if has_xattr(path, "trusted.overlay.opaque", "y") {
            return ModuleFileType::OpaqueDir;
        }
        return ModuleFileType::Directory;
    }

    // Regular file checks
    if ft.is_file() {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // AUFS whiteout: .wh.* prefix
        if name.starts_with(".wh.") {
            return ModuleFileType::WhiteoutAufs;
        }

        // Xattr whiteout: zero-size + trusted.overlay.whiteout=y
        if metadata.len() == 0 && has_xattr(path, "trusted.overlay.whiteout", "y") {
            return ModuleFileType::WhiteoutXattr;
        }

        // Redirect xattr
        if has_xattr_present(path, "trusted.overlay.redirect") {
            return ModuleFileType::RedirectXattr;
        }

        return ModuleFileType::Regular;
    }

    ModuleFileType::Regular
}

/// Check if a character device has major=0, minor=0.
fn is_zero_char_device(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match fs::metadata(path) {
        Ok(m) => {
            let dev = m.rdev();
            // major and minor are packed in rdev
            let major = libc::major(dev as _);
            let minor = libc::minor(dev as _);
            major == 0 && minor == 0
        }
        Err(_) => false,
    }
}

/// Check if a path has a specific xattr with a specific value.
fn has_xattr(path: &Path, attr_name: &str, expected_value: &str) -> bool {
    let c_path = match std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let c_attr = match std::ffi::CString::new(attr_name) {
        Ok(a) => a,
        Err(_) => return false,
    };

    let mut buf = [0u8; 256];
    let len = unsafe {
        libc::lgetxattr(
            c_path.as_ptr(),
            c_attr.as_ptr(),
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
        )
    };

    if len <= 0 {
        return false;
    }

    let value = &buf[..len as usize];
    // xattr values may or may not be null-terminated
    let trimmed = if value.last() == Some(&0) {
        &value[..value.len() - 1]
    } else {
        value
    };
    trimmed == expected_value.as_bytes()
}

/// Check if a path has a specific xattr (any value).
fn has_xattr_present(path: &Path, attr_name: &str) -> bool {
    let c_path = match std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let c_attr = match std::ffi::CString::new(attr_name) {
        Ok(a) => a,
        Err(_) => return false,
    };

    let len = unsafe {
        libc::lgetxattr(
            c_path.as_ptr(),
            c_attr.as_ptr(),
            std::ptr::null_mut(),
            0,
        )
    };

    len >= 0
}

/// Parse module.prop key=value format.
pub fn parse_module_prop(path: &Path) -> Result<ModuleProp> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("cannot read module.prop: {}", path.display()))?;

    let mut prop = ModuleProp::default();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "id" => prop.id = value.to_string(),
                "name" => prop.name = value.to_string(),
                "version" => prop.version = value.to_string(),
                "versionCode" => {
                    prop.version_code = value.parse().unwrap_or(0);
                }
                "author" => prop.author = value.to_string(),
                "description" => prop.description = value.to_string(),
                _ => {}
            }
        }
    }

    Ok(prop)
}

pub fn is_module_enabled(module_dir: &Path) -> bool {
    !module_dir.join("disable").exists() && !module_dir.join("remove").exists()
}

pub fn has_skip_mount(module_dir: &Path) -> bool {
    module_dir.join("skip_mount").exists()
}

