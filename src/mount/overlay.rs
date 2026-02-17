use std::ffi::CString;
use std::path::Path;

use anyhow::{bail, Result};
use tracing::{debug, info};

use crate::core::types::{MountResult, MountStrategy};

// Syscall numbers for the new mount API (Linux 5.2+)
#[cfg(target_arch = "aarch64")]
mod syscall_nr {
    pub const SYS_FSOPEN: libc::c_long = 430;
    pub const SYS_FSCONFIG: libc::c_long = 431;
    pub const SYS_FSMOUNT: libc::c_long = 432;
    pub const SYS_MOVE_MOUNT: libc::c_long = 429;
}

#[cfg(target_arch = "x86_64")]
mod syscall_nr {
    pub const SYS_FSOPEN: libc::c_long = 430;
    pub const SYS_FSCONFIG: libc::c_long = 431;
    pub const SYS_FSMOUNT: libc::c_long = 432;
    pub const SYS_MOVE_MOUNT: libc::c_long = 429;
}

#[cfg(target_arch = "arm")]
mod syscall_nr {
    pub const SYS_FSOPEN: libc::c_long = 430;
    pub const SYS_FSCONFIG: libc::c_long = 431;
    pub const SYS_FSMOUNT: libc::c_long = 432;
    pub const SYS_MOVE_MOUNT: libc::c_long = 429;
}

#[cfg(target_arch = "x86")]
mod syscall_nr {
    pub const SYS_FSOPEN: libc::c_long = 430;
    pub const SYS_FSCONFIG: libc::c_long = 431;
    pub const SYS_FSMOUNT: libc::c_long = 432;
    pub const SYS_MOVE_MOUNT: libc::c_long = 429;
}

// fsconfig command constants
const FSCONFIG_SET_STRING: libc::c_uint = 1;
const FSCONFIG_CMD_CREATE: libc::c_uint = 6;

// move_mount flags
const MOVE_MOUNT_F_EMPTY_PATH: libc::c_uint = 0x00000004;

/// Mount a read-only overlay filesystem at `target` with the given lower directories.
///
/// Tries the new mount API first (fsopen/fsconfig/fsmount/move_mount), falling
/// back to legacy mount(2) if the syscalls aren't available (ME02).
/// Uses lowerdir-only mode (no upperdir/workdir) — module content merges
/// on top of the original filesystem without write support.
pub fn mount_overlay(
    lower_dirs: &[&Path],
    target: &Path,
    module_id: &str,
    overlay_source: &str,
) -> Result<MountResult> {
    if lower_dirs.is_empty() {
        return Ok(MountResult {
            module_id: module_id.to_string(),
            strategy_used: MountStrategy::Overlay,
            success: false,
            rules_applied: 0,
            rules_failed: 0,
            error: Some("no lower directories provided".to_string()),
            mount_paths: Vec::new(),
        });
    }

    if !target.exists() {
        return Ok(MountResult {
            module_id: module_id.to_string(),
            strategy_used: MountStrategy::Overlay,
            success: false,
            rules_applied: 0,
            rules_failed: 1,
            error: Some(format!("overlay target does not exist: {}", target.display())),
            mount_paths: Vec::new(),
        });
    }

    let lowerdir = build_lowerdir_string(lower_dirs, target);

    // Try new mount API first, fall back to legacy
    let result = match mount_overlay_new_api(&lowerdir, target, overlay_source) {
        Ok(()) => {
            info!(
                target = %target.display(),
                module = module_id,
                api = "new",
                "overlay mounted"
            );
            Ok(())
        }
        Err(e) => {
            debug!(
                error = %e,
                "new mount API failed, trying legacy mount(2)"
            );
            mount_overlay_legacy(&lowerdir, target, overlay_source)
                .map(|()| {
                    info!(
                        target = %target.display(),
                        module = module_id,
                        api = "legacy",
                        "overlay mounted"
                    );
                })
        }
    };

    match result {
        Ok(()) => {
            Ok(MountResult {
                module_id: module_id.to_string(),
                strategy_used: MountStrategy::Overlay,
                success: true,
                rules_applied: 1,
                rules_failed: 0,
                error: None,
                mount_paths: vec![target.to_string_lossy().to_string()],
            })
        }
        Err(e) => Ok(MountResult {
            module_id: module_id.to_string(),
            strategy_used: MountStrategy::Overlay,
            success: false,
            rules_applied: 0,
            rules_failed: 1,
            error: Some(format!("overlay mount failed: {e}")),
            mount_paths: Vec::new(),
        }),
    }
}

/// Build the lowerdir= option string. The target (original) directory goes last
/// so module files take precedence in the overlay stack.
fn build_lowerdir_string(lower_dirs: &[&Path], target: &Path) -> String {
    let mut parts: Vec<String> = lower_dirs
        .iter()
        .map(|p| escape_overlay_path(&p.to_string_lossy()))
        .collect();

    // Original filesystem content as the bottom layer
    parts.push(escape_overlay_path(&target.to_string_lossy()));

    parts.join(":")
}

/// Escape colons and backslashes in overlay path options (legacy API requirement).
fn escape_overlay_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace(',', "\\,")
}

/// New mount API: fsopen -> fsconfig -> fsmount -> move_mount (Linux 5.2+).
/// Provides structured error reporting vs the legacy single-call approach.
fn mount_overlay_new_api(lowerdir: &str, target: &Path, overlay_source: &str) -> Result<()> {
    let c_fstype = CString::new("overlay")?;

    // fsopen("overlay", 0)
    let fs_fd = unsafe { libc::syscall(syscall_nr::SYS_FSOPEN, c_fstype.as_ptr(), 0x01u32) };
    if fs_fd < 0 {
        bail!(
            "fsopen(overlay): {}",
            std::io::Error::last_os_error()
        );
    }
    let fs_fd = fs_fd as libc::c_int;

    // Ensure we close fs_fd on any error path
    let result = (|| -> Result<()> {
        // fsconfig(fs_fd, FSCONFIG_SET_STRING, "source", "KSU", 0)
        fsconfig_set_string(fs_fd, "source", overlay_source)?;

        // fsconfig(fs_fd, FSCONFIG_SET_STRING, "lowerdir", lowerdir, 0)
        fsconfig_set_string(fs_fd, "lowerdir", lowerdir)?;

        // lowerdir-only overlay: no upperdir/workdir needed (read-only merge)

        // fsconfig(fs_fd, FSCONFIG_CMD_CREATE, NULL, NULL, 0) -- finalize
        let ret = unsafe {
            libc::syscall(
                syscall_nr::SYS_FSCONFIG,
                fs_fd,
                FSCONFIG_CMD_CREATE,
                std::ptr::null::<libc::c_char>(),
                std::ptr::null::<libc::c_char>(),
                0i32,
            )
        };
        if ret < 0 {
            bail!("fsconfig(CMD_CREATE): {}", std::io::Error::last_os_error());
        }

        // fsmount(fs_fd, FSMOUNT_CLOEXEC, MOUNT_ATTR_RDONLY)
        // MOUNT_ATTR_RDONLY = 0x1: match stock overlay VFS flags (ro,relatime)
        let mnt_fd = unsafe {
            libc::syscall(syscall_nr::SYS_FSMOUNT, fs_fd, 0x00000001u32, 0x00000001u32)
        };
        if mnt_fd < 0 {
            bail!("fsmount: {}", std::io::Error::last_os_error());
        }
        let mnt_fd = mnt_fd as libc::c_int;

        // move_mount(mnt_fd, "", AT_FDCWD, target, MOVE_MOUNT_F_EMPTY_PATH)
        let c_empty = CString::new("")?;
        let c_target = CString::new(target.as_os_str().as_encoded_bytes())?;
        let ret = unsafe {
            libc::syscall(
                syscall_nr::SYS_MOVE_MOUNT,
                mnt_fd,
                c_empty.as_ptr(),
                libc::AT_FDCWD,
                c_target.as_ptr(),
                MOVE_MOUNT_F_EMPTY_PATH,
            )
        };

        unsafe { libc::close(mnt_fd) };

        if ret < 0 {
            bail!("move_mount: {}", std::io::Error::last_os_error());
        }

        Ok(())
    })();

    unsafe { libc::close(fs_fd) };
    result
}

fn fsconfig_set_string(fs_fd: libc::c_int, key: &str, value: &str) -> Result<()> {
    let c_key = CString::new(key)?;
    let c_value = CString::new(value)?;

    let ret = unsafe {
        libc::syscall(
            syscall_nr::SYS_FSCONFIG,
            fs_fd,
            FSCONFIG_SET_STRING,
            c_key.as_ptr(),
            c_value.as_ptr(),
            0i32,
        )
    };

    if ret < 0 {
        bail!(
            "fsconfig({key}={value}): {}",
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}


/// Legacy mount(2) fallback for kernels without the new mount API.
fn mount_overlay_legacy(lowerdir: &str, target: &Path, overlay_source: &str) -> Result<()> {
    let c_source = CString::new(overlay_source)?;
    let c_target = CString::new(target.as_os_str().as_encoded_bytes())?;
    let c_fstype = CString::new("overlay")?;

    let data = format!("lowerdir={}", lowerdir);
    let c_data = CString::new(data)?;

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            libc::MS_RDONLY,
            c_data.as_ptr() as *const libc::c_void,
        )
    };

    if ret != 0 {
        bail!(
            "mount(overlay) at {}: {}",
            target.display(),
            std::io::Error::last_os_error()
        );
    }

    Ok(())
}
