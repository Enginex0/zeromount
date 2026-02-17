use std::ffi::CString;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, trace, warn};

/// Walk a module's system directory looking for `.replace` markers.
/// For each directory with a `.replace` file, set `trusted.overlay.opaque=y`
/// on the corresponding directory in the staging lower dir.
pub fn mark_opaque_dirs(module_system_dir: &Path, lower_dir: &Path) -> Result<()> {
    mark_opaque_recursive(module_system_dir, module_system_dir, lower_dir)
}

fn mark_opaque_recursive(base: &Path, current: &Path, lower_dir: &Path) -> Result<()> {
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.join(".replace").exists() {
            if let Ok(rel) = path.strip_prefix(base) {
                let staging_dir = lower_dir.join(rel);
                if staging_dir.is_dir() {
                    if let Err(e) = set_opaque_xattr(&staging_dir) {
                        warn!(
                            dir = %staging_dir.display(),
                            error = %e,
                            "failed to set overlay.opaque"
                        );
                    } else {
                        debug!(dir = %staging_dir.display(), "set overlay.opaque");
                    }
                }
            }
        }
        mark_opaque_recursive(base, &path, lower_dir)?;
    }

    Ok(())
}

fn set_opaque_xattr(dir: &Path) -> Result<()> {
    let path_cstr =
        CString::new(dir.to_string_lossy().as_bytes()).context("invalid path for xattr")?;
    let name = CString::new("trusted.overlay.opaque").unwrap();
    let val = b"y";
    let ret = unsafe {
        libc::lsetxattr(
            path_cstr.as_ptr(),
            name.as_ptr(),
            val.as_ptr() as *const libc::c_void,
            val.len(),
            0,
        )
    };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        anyhow::bail!("lsetxattr trusted.overlay.opaque on {}: {err}", dir.display());
    }
    Ok(())
}
