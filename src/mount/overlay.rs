use std::ffi::CString;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use tracing::{debug, info, warn};

use crate::core::types::{MountResult, MountStrategy};

// Kernel overlayfs layer limit varies: 64 on older kernels, 128 on 5.10+.
// Use a conservative threshold that leaves room for the target + decoy layers.
const MAX_LOWER_LAYERS: usize = 50;

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
    decoy_dir: Option<&Path>,
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

    // Collapse excess layers into staging overlays to stay under kernel limits.
    // Each staging mount merges a chunk of lowerdirs into one read-only overlay,
    // reducing the final layer count.
    let (owned_dirs, _staging_guard) = if lower_dirs.len() > MAX_LOWER_LAYERS {
        let (dirs, guard) = collapse_excess_layers(lower_dirs, target, overlay_source)?;
        (dirs, Some(guard))
    } else {
        (lower_dirs.iter().map(|p| p.to_path_buf()).collect(), None)
    };
    let effective_dirs: Vec<&Path> = owned_dirs.iter().map(|p| p.as_path()).collect();

    let lowerdir = build_lowerdir_string(&effective_dirs, target, decoy_dir);

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

fn build_lowerdir_string(lower_dirs: &[&Path], target: &Path, decoy_dir: Option<&Path>) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(decoy) = decoy_dir {
        parts.push(escape_overlay_path(&decoy.to_string_lossy()));
    }

    parts.extend(
        lower_dirs
            .iter()
            .map(|p| escape_overlay_path(&p.to_string_lossy())),
    );

    parts.push(escape_overlay_path(&target.to_string_lossy()));

    parts.join(":")
}

// ovl_split_lowerdirs (5.10) and ovl_parse_param_split_lowerdirs (6.6) both
// consume backslash escapes — same string works for legacy mount(2) and fsopen.
fn escape_overlay_path(path: &str) -> String {
    path.replace('\\', "\\\\").replace(':', "\\:").replace(',', "\\,")
}

/// New mount API: fsopen -> fsconfig -> fsmount -> move_mount (Linux 5.2+).
/// Provides structured error reporting vs the legacy single-call approach.
fn mount_overlay_new_api(lowerdir: &str, target: &Path, overlay_source: &str) -> Result<()> {
    let c_fstype = CString::new("overlay")?;

    // SAFETY: CString is non-null NUL-terminated; syscall args are valid constants.
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
        // SAFETY: fs_fd is a valid open fd from fsopen; CStrings and constants are valid.
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
        // SAFETY: fs_fd is a valid open fd from fsopen; flags are valid constants.
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
        // SAFETY: mnt_fd is a valid fd from fsmount; CStrings are NUL-terminated.
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

        // SAFETY: mnt_fd is a valid open fd from fsmount above.
        unsafe { libc::close(mnt_fd) };

        if ret < 0 {
            bail!("move_mount: {}", std::io::Error::last_os_error());
        }

        Ok(())
    })();

    // SAFETY: fs_fd is a valid open fd from fsopen above.
    unsafe { libc::close(fs_fd) };
    result
}

fn fsconfig_set_string(fs_fd: libc::c_int, key: &str, value: &str) -> Result<()> {
    let c_key = CString::new(key)?;
    let c_value = CString::new(value)?;

    // SAFETY: fs_fd is a valid open fd from fsopen; CStrings are NUL-terminated.
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

    // SAFETY: CStrings are non-null NUL-terminated; mount flags are valid constants.
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

/// Collapse excess lower directories into intermediate staging overlays.
///
/// Splits the bottom layers into chunks, mounts each chunk as a read-only
/// overlay on a tmpfs staging dir, then returns the staging dirs + remaining
/// top layers as the new effective lowerdir list. The StagingGuard unmounts
/// and removes staging dirs on drop.
fn collapse_excess_layers(
    lower_dirs: &[&Path],
    target: &Path,
    overlay_source: &str,
) -> Result<(Vec<PathBuf>, StagingGuard)> {
    let mut staging_dirs: Vec<PathBuf> = Vec::new();
    let mut remaining: Vec<PathBuf> = lower_dirs.iter().map(|p| p.to_path_buf()).collect();

    let mut pass = 0u32;
    while remaining.len() > MAX_LOWER_LAYERS {
        let split_at = remaining.len() - MAX_LOWER_LAYERS + 1;
        let bottom_chunk: Vec<PathBuf> = remaining.drain(..split_at).collect();

        let staging_dir = PathBuf::from(format!(
            "/dev/zeromount_staging_{}_{}",
            target.file_name().unwrap_or_default().to_string_lossy(),
            pass,
        ));
        std::fs::create_dir_all(&staging_dir)?;

        let chunk_refs: Vec<&Path> = bottom_chunk.iter().map(|p| p.as_path()).collect();
        let lowerdir = build_lowerdir_string(&chunk_refs, target, None);

        match mount_overlay_new_api(&lowerdir, &staging_dir, overlay_source) {
            Ok(()) => {}
            Err(e) => {
                debug!(error = %e, "staging: new API failed, trying legacy");
                mount_overlay_legacy(&lowerdir, &staging_dir, overlay_source)?;
            }
        }

        info!(
            pass,
            collapsed = bottom_chunk.len(),
            staging = %staging_dir.display(),
            "overlay layer staging complete"
        );

        remaining.insert(0, staging_dir.clone());
        staging_dirs.push(staging_dir);
        pass += 1;
    }

    warn!(
        layers = lower_dirs.len(),
        staging_mounts = staging_dirs.len(),
        "collapsed overlay layers to fit kernel limit"
    );

    Ok((remaining, StagingGuard { dirs: staging_dirs }))
}

/// Unmounts and removes staging overlay directories when dropped.
struct StagingGuard {
    dirs: Vec<PathBuf>,
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        for dir in self.dirs.iter().rev() {
            let c_path = match CString::new(dir.as_os_str().as_encoded_bytes()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            unsafe { libc::umount2(c_path.as_ptr(), libc::MNT_DETACH) };
            let _ = std::fs::remove_dir(dir);
        }
    }
}
