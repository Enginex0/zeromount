use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};

use crate::core::config::{MountConfig, StorageMode as ConfigStorageMode};
use crate::core::types::CapabilityFlags;
use crate::utils::command::{run_command_with_timeout, CMD_TIMEOUT};

const TMPFS_SOURCE_POOL: &[&str] = &["tmpfs", "none", "shmem", "shm"];
const APEX_SPOOF_NAME: &str = "com.android.mntservice";
const RANDOM_PATH_LEN: usize = 12;
const FIXED_PATH_NAME: &str = "zeromount";
const MODULES_DIR_PATH: &str = "/data/adb/modules";
const MIN_EXT4_SIZE_MB: u64 = 64;
const LKM_DIR: &str = "lkm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Erofs,
    Tmpfs,
    Ext4,
}

/// Handle to a prepared storage area. Drop-safe: cleanup on drop.
#[derive(Debug)]
pub struct StorageHandle {
    pub mode: StorageMode,
    pub base_path: PathBuf,
    /// Per-module lower directories live under base_path/<module_id>/
    cleaned_up: bool,
    pub overlay_source: String,
    pub apex_mounts: Option<(PathBuf, PathBuf)>,
}

impl StorageHandle {
    /// Get the lower directory path for a specific module's partition content.
    pub fn lower_dir(&self, module_id: &str, partition: &str) -> PathBuf {
        self.base_path.join(module_id).join(partition)
    }

    /// Keep staging alive — overlay lowerdirs and magic skeleton bind mounts
    /// reference data inside this tmpfs for the lifetime of the mounts.
    pub fn suppress_cleanup(&mut self) {
        self.cleaned_up = true;
    }
}

impl Drop for StorageHandle {
    fn drop(&mut self) {
        if !self.cleaned_up {
            if let Err(e) = cleanup_storage_inner(&self.base_path, self.mode, self.apex_mounts.as_ref()) {
                warn!(error = %e, "storage cleanup failed during drop");
            }
        }
    }
}

/// ME01: Initialize storage respecting config preference, falling back via cascade.
/// ME11: Random or fixed mount path under /mnt/.
pub fn init_storage(capabilities: &CapabilityFlags, mount_config: &MountConfig) -> Result<StorageHandle> {
    let base_path = if mount_config.random_mount_paths {
        generate_random_path()
    } else {
        generate_fixed_path()
    };

    info!(path = %base_path.display(), random = mount_config.random_mount_paths, "staging path selected");

    let staging_source = resolve_staging_source(&mount_config.mount_source);
    info!(source = %staging_source, "staging mount source resolved");

    let overlay_source = resolve_overlay_source(&mount_config.overlay_source);
    info!(source = %overlay_source, "overlay mount source resolved");

    fs::create_dir_all(&base_path)
        .with_context(|| format!("cannot create staging dir: {}", base_path.display()))?;

    // If user forced a specific mode, try it first
    match mount_config.storage_mode {
        ConfigStorageMode::Erofs => {
            if let Some(mut handle) = try_mode_erofs(&base_path, capabilities) {
                handle.overlay_source = overlay_source;
                return Ok(handle);
            }
            warn!("forced EROFS failed, falling back to cascade");
        }
        ConfigStorageMode::Tmpfs => {
            if let Some(mut handle) = try_mode_tmpfs(&base_path, capabilities, &staging_source) {
                handle.overlay_source = overlay_source;
                return Ok(handle);
            }
            warn!("forced tmpfs failed, falling back to cascade");
        }
        ConfigStorageMode::Ext4 => {
            if let Some(handle) = try_mode_ext4(&base_path, &overlay_source) {
                return Ok(handle);
            }
            warn!("forced ext4 failed, falling back to cascade");
        }
        ConfigStorageMode::Auto => {}
    }

    // Cascade: EROFS -> tmpfs+xattr -> ext4 -> bare tmpfs
    if let Some(mut handle) = try_mode_erofs(&base_path, capabilities) {
        handle.overlay_source = overlay_source;
        return Ok(handle);
    }
    if let Some(mut handle) = try_mode_tmpfs(&base_path, capabilities, &staging_source) {
        handle.overlay_source = overlay_source;
        return Ok(handle);
    }
    if let Some(handle) = try_mode_ext4(&base_path, &overlay_source) {
        return Ok(handle);
    }

    // Bare tmpfs fallback (no xattr guarantee)
    match mount_tmpfs_at(&base_path, &staging_source) {
        Ok(()) => {
            info!(mode = "tmpfs", path = %base_path.display(), "storage initialized (bare fallback)");
        }
        Err(e) => {
            warn!(error = %e, "all storage mounts failed, using bare directory");
        }
    }
    Ok(StorageHandle {
        mode: StorageMode::Tmpfs,
        base_path,
        cleaned_up: false,
        overlay_source,
        apex_mounts: None,
    })
}

fn try_mode_erofs(base_path: &Path, capabilities: &CapabilityFlags) -> Option<StorageHandle> {
    if !capabilities.erofs_supported || !is_erofs_available() {
        return None;
    }
    match try_erofs_storage(base_path) {
        Ok(()) => {
            info!(mode = "erofs", path = %base_path.display(), "storage initialized");
            Some(StorageHandle { mode: StorageMode::Erofs, base_path: base_path.to_path_buf(), cleaned_up: false, overlay_source: String::new(), apex_mounts: None })
        }
        Err(e) => {
            debug!(error = %e, "EROFS init failed");
            let _ = do_umount(base_path);
            None
        }
    }
}

fn try_mode_tmpfs(base_path: &Path, capabilities: &CapabilityFlags, source_name: &str) -> Option<StorageHandle> {
    if !capabilities.tmpfs_xattr {
        return None;
    }
    match try_tmpfs_with_xattr(base_path, source_name) {
        Ok(()) => {
            info!(mode = "tmpfs", path = %base_path.display(), "storage initialized");
            Some(StorageHandle { mode: StorageMode::Tmpfs, base_path: base_path.to_path_buf(), cleaned_up: false, overlay_source: String::new(), apex_mounts: None })
        }
        Err(e) => {
            debug!(error = %e, "tmpfs with xattr failed");
            let _ = do_umount(base_path);
            None
        }
    }
}

fn try_mode_ext4(base_path: &Path, overlay_source: &str) -> Option<StorageHandle> {
    match try_ext4_storage(base_path) {
        Ok(()) => {
            info!(mode = "ext4", path = %base_path.display(), "storage initialized");

            if has_ksud_nuke() || select_nuke_ko(base_path).is_some() {
                nuke_ext4_sysfs(base_path);
                Some(StorageHandle {
                    mode: StorageMode::Ext4,
                    base_path: base_path.to_path_buf(),
                    cleaned_up: false,
                    overlay_source: overlay_source.to_string(),
                    apex_mounts: None,
                })
            } else {
                match try_apex_spoof(base_path) {
                    Ok((versioned, facade)) => {
                        Some(StorageHandle {
                            mode: StorageMode::Ext4,
                            base_path: versioned.clone(),
                            cleaned_up: false,
                            overlay_source: overlay_source.to_string(),
                            apex_mounts: Some((versioned, facade)),
                        })
                    }
                    Err(e) => {
                        debug!(error = %e, "APEX spoof failed, running nuke best-effort");
                        nuke_ext4_sysfs(base_path);
                        Some(StorageHandle {
                            mode: StorageMode::Ext4,
                            base_path: base_path.to_path_buf(),
                            cleaned_up: false,
                            overlay_source: overlay_source.to_string(),
                            apex_mounts: None,
                        })
                    }
                }
            }
        }
        Err(e) => {
            debug!(error = %e, "ext4 loopback failed");
            None
        }
    }
}

/// Explicitly clean up storage. Used by Drop; overlay/magic callers
/// use suppress_cleanup() instead to keep staging alive.
#[allow(dead_code)]
pub fn cleanup_storage(handle: &mut StorageHandle) -> Result<()> {
    cleanup_storage_inner(&handle.base_path, handle.mode, handle.apex_mounts.as_ref())?;
    handle.cleaned_up = true;
    Ok(())
}

fn cleanup_storage_inner(base_path: &Path, _mode: StorageMode, apex_mounts: Option<&(PathBuf, PathBuf)>) -> Result<()> {
    if let Some((versioned, facade)) = apex_mounts {
        let _ = do_umount(facade);
        let _ = fs::remove_dir(facade);
        let _ = do_umount(versioned);
        let _ = fs::remove_dir(versioned);
        // Remove symlink at original base_path if it exists
        let _ = fs::remove_file(base_path);
    }

    let _ = do_umount(base_path);

    if base_path.exists() {
        fs::remove_dir_all(base_path)
            .with_context(|| format!("cannot remove staging dir: {}", base_path.display()))?;
    }

    Ok(())
}

/// ME11: Generate random 12-char alphanumeric path under /mnt/.
/// Falls back to /mnt/vendor/ if /mnt/ is not writable.
fn generate_random_path() -> PathBuf {
    resolve_mount_base(&random_alphanum(RANDOM_PATH_LEN))
}

fn generate_fixed_path() -> PathBuf {
    resolve_mount_base(FIXED_PATH_NAME)
}

fn resolve_mount_base(name: &str) -> PathBuf {
    if is_dir_writable("/mnt") {
        return PathBuf::from("/mnt").join(name);
    }
    if is_dir_writable("/mnt/vendor") {
        return PathBuf::from("/mnt/vendor").join(name);
    }
    PathBuf::from("/dev").join(name)
}

fn random_alphanum(len: usize) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0xDEAD_BEEF);

    // Simple LCG seeded from time -- sufficient for path randomization
    let mut state = seed as u64;
    let chars: Vec<u8> = (0..len)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let idx = ((state >> 33) % 36) as u8;
            if idx < 10 {
                b'0' + idx
            } else {
                b'a' + (idx - 10)
            }
        })
        .collect();

    String::from_utf8(chars).unwrap_or_else(|_| "zeromount_tmp".to_string())
}

fn resolve_staging_source(config_value: &str) -> String {
    if config_value.is_empty() || config_value == "auto" {
        use std::time::{SystemTime, UNIX_EPOCH};
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0xDEAD_BEEF);
        let idx = (seed as usize) % TMPFS_SOURCE_POOL.len();
        TMPFS_SOURCE_POOL[idx].to_string()
    } else {
        config_value.to_string()
    }
}

pub(crate) fn resolve_overlay_source(config_value: &str) -> String {
    if config_value.is_empty() || config_value == "auto" {
        match crate::utils::platform::detect_root_manager() {
            Ok(mgr) => match mgr.name() {
                "KernelSU" | "APatch" => "KSU".to_string(),
                "Magisk" => "magisk".to_string(),
                _ => "overlay".to_string(),
            },
            Err(_) => "overlay".to_string(),
        }
    } else {
        config_value.to_string()
    }
}

fn is_dir_writable(path: &str) -> bool {
    let c_path = match CString::new(path) {
        Ok(p) => p,
        Err(_) => return false,
    };
    unsafe { libc::access(c_path.as_ptr(), libc::W_OK) == 0 }
}

/// Check if the kernel supports EROFS by reading /proc/filesystems.
fn is_erofs_available() -> bool {
    fs::read_to_string("/proc/filesystems")
        .map(|content| content.lines().any(|l| l.contains("erofs")))
        .unwrap_or(false)
}

/// Create EROFS image from base_path content, mount it read-only, then nuke the image.
fn try_erofs_storage(base_path: &Path) -> Result<()> {
    let image_path = base_path.with_extension("erofs.img");

    let status = run_command_with_timeout(
        Command::new("mkfs.erofs")
            .args(["-z", "lz4hc", "-x", "256"])
            .arg(&image_path)
            .arg(base_path),
        CMD_TIMEOUT,
    )?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        bail!("mkfs.erofs failed: {stderr}");
    }

    // Mount EROFS image read-only
    let c_source = CString::new(image_path.as_os_str().as_encoded_bytes())?;
    let c_target = CString::new(base_path.as_os_str().as_encoded_bytes())?;
    let c_fstype = CString::new("erofs")?;

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            libc::MS_RDONLY,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        let errno = std::io::Error::last_os_error();
        let _ = fs::remove_file(&image_path);
        bail!("mount erofs at {}: {}", base_path.display(), errno);
    }

    // ME12: nuke image after mount — kernel keeps inode alive
    let _ = nuke_backing_file(&image_path);
    Ok(())
}

/// Mount tmpfs and verify xattr support for overlay whiteouts.
fn try_tmpfs_with_xattr(base_path: &Path, source_name: &str) -> Result<()> {
    mount_tmpfs_at(base_path, source_name)?;

    // Test xattr support: overlay needs trusted.overlay.whiteout
    let test_path = base_path.join(".xattr_test");
    let _ = fs::write(&test_path, "");

    let c_path = CString::new(test_path.as_os_str().as_encoded_bytes())?;
    let c_name = CString::new("trusted.overlay.whiteout")?;
    let value = b"y";

    let ret = unsafe {
        libc::setxattr(
            c_path.as_ptr(),
            c_name.as_ptr(),
            value.as_ptr() as *const libc::c_void,
            value.len(),
            0,
        )
    };

    let _ = fs::remove_file(&test_path);

    if ret != 0 {
        bail!("tmpfs lacks xattr support for overlay whiteouts");
    }

    Ok(())
}

fn calculate_ext4_image_size_mb() -> u64 {
    let modules_dir = Path::new(MODULES_DIR_PATH);
    if !modules_dir.is_dir() {
        return MIN_EXT4_SIZE_MB;
    }

    let total_bytes: u64 = fs::read_dir(modules_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| dir_size_recursive(&e.path()))
                .sum()
        })
        .unwrap_or(0);

    let total_mb = total_bytes / (1024 * 1024);
    let sized = (total_mb as f64 * 1.5) as u64;
    sized.max(MIN_EXT4_SIZE_MB)
}

fn dir_size_recursive(path: &Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size_recursive(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Create a sparse ext4 image and loop-mount it.
fn try_ext4_storage(base_path: &Path) -> Result<()> {
    let image_path = base_path.with_extension("ext4.img");

    let image_size_mb = calculate_ext4_image_size_mb();
    debug!(size_mb = image_size_mb, "ext4 image size calculated");

    let dd_status = run_command_with_timeout(
        Command::new("dd").args([
            "if=/dev/zero",
            &format!("of={}", image_path.display()),
            "bs=1M",
            "count=0",
            &format!("seek={image_size_mb}"),
        ]),
        CMD_TIMEOUT,
    )?;

    if !dd_status.status.success() {
        let stderr = String::from_utf8_lossy(&dd_status.stderr);
        bail!("dd sparse image failed: {stderr}");
    }

    let mkfs_status = run_command_with_timeout(
        Command::new("mkfs.ext4")
            .args(["-O", "^has_journal"])
            .arg(&image_path),
        CMD_TIMEOUT,
    )?;

    if !mkfs_status.status.success() {
        let _ = fs::remove_file(&image_path);
        let stderr = String::from_utf8_lossy(&mkfs_status.stderr);
        bail!("mkfs.ext4 failed: {stderr}");
    }

    // Loop mount with noatime
    let c_source = CString::new(image_path.as_os_str().as_encoded_bytes())?;
    let c_target = CString::new(base_path.as_os_str().as_encoded_bytes())?;
    let c_fstype = CString::new("ext4")?;
    let c_data = CString::new("loop")?;

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            libc::MS_NOATIME,
            c_data.as_ptr() as *const libc::c_void,
        )
    };

    if ret != 0 {
        let errno = std::io::Error::last_os_error();
        let _ = fs::remove_file(&image_path);
        bail!("mount ext4 at {}: {}", base_path.display(), errno);
    }

    crate::utils::selinux::set_selinux_context(&image_path, "u:object_r:ksu_file:s0");

    Ok(())
}

/// Mount tmpfs at target with the given source name (ME09).
fn mount_tmpfs_at(target: &Path, source_name: &str) -> Result<()> {
    let c_source = CString::new(source_name)?;
    let c_target = CString::new(target.as_os_str().as_encoded_bytes())?;
    let c_fstype = CString::new("tmpfs")?;
    let c_data = CString::new("mode=0755")?;

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            0,
            c_data.as_ptr() as *const libc::c_void,
        )
    };

    if ret != 0 {
        let errno = std::io::Error::last_os_error();
        bail!("mount tmpfs at {}: {}", target.display(), errno);
    }

    Ok(())
}

/// Unmount a path. Returns Ok even if not mounted.
fn do_umount(target: &Path) -> Result<()> {
    let c_target = match CString::new(target.as_os_str().as_encoded_bytes()) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let ret = unsafe { libc::umount2(c_target.as_ptr(), libc::MNT_DETACH) };
    if ret != 0 {
        let errno = std::io::Error::last_os_error();
        // EINVAL = not mounted, which is fine
        if errno.raw_os_error() != Some(libc::EINVAL) {
            debug!(path = %target.display(), error = %errno, "umount failed");
        }
    }

    Ok(())
}

/// ME12: Delete a backing file after mount. The kernel keeps the inode alive
/// via the mount reference, but the file disappears from the directory.
pub fn nuke_backing_file(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("cannot nuke backing file: {}", path.display()))?;
        debug!(path = %path.display(), "nuked backing file");
    }
    Ok(())
}

/// Remove ext4 sysfs entries to hide loop mount evidence.
/// Dual-path: try ksud first (KSU/APatch), fall back to LKM (Magisk).
/// Always non-fatal -- detection evasion is best-effort.
fn nuke_ext4_sysfs(mount_point: &Path) {
    let mount_str = mount_point.to_string_lossy();

    // Path 1: ksud (KSU/APatch 22105+)
    match run_command_with_timeout(
        Command::new("ksud").args(["kernel", "nuke-ext4-sysfs", &mount_str]),
        CMD_TIMEOUT,
    ) {
        Ok(output) if output.status.success() => {
            debug!(path = %mount_str, "ext4 sysfs nuked via ksud");
            return;
        }
        Ok(_) => debug!("ksud nuke-ext4-sysfs failed, trying LKM fallback"),
        Err(_) => debug!("ksud not available, trying LKM fallback"),
    }

    // Path 2: LKM fallback
    let Some(ko_path) = select_nuke_ko(mount_point) else {
        debug!("no suitable nuke LKM found, skipping ext4 sysfs nuke");
        return;
    };

    let Some(symaddr) = read_kallsyms_address("ext4_unregister_sysfs") else {
        debug!("ext4_unregister_sysfs not found in /proc/kallsyms, skipping LKM nuke");
        return;
    };

    match run_command_with_timeout(
        Command::new("insmod")
            .arg(&ko_path)
            .arg(format!("mount_point={mount_str}"))
            .arg(format!("symaddr={symaddr}")),
        CMD_TIMEOUT,
    ) {
        Ok(output) if output.status.code() != Some(0) => {
            // insmod returns non-zero because the module returns -EAGAIN (intentional auto-unload)
            debug!(path = %mount_str, "ext4 sysfs nuked via LKM");
        }
        Ok(_) => debug!(path = %mount_str, "LKM nuke loaded (unexpected success code)"),
        Err(e) => debug!(error = %e, "insmod failed for nuke LKM"),
    }
}

/// Find the best-matching nuke .ko file for the running kernel.
/// Files are at <module_dir>/lkm/nuke-android<ver>-<kernel>.ko
fn select_nuke_ko(_module_base: &Path) -> Option<PathBuf> {
    // Module directory: /data/adb/modules/meta-zeromount/lkm/
    let lkm_dir = Path::new("/data/adb/modules/meta-zeromount").join(LKM_DIR);
    if !lkm_dir.is_dir() {
        return None;
    }

    let uname_r = fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from))?;

    // Extract major.minor from uname -r (e.g., "5.10.198-android12" -> "5.10")
    let kernel_ver: String = uname_r
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".");

    // Suffix match prevents "5.1" from matching "nuke-android12-5.10.ko"
    let suffix = format!("-{kernel_ver}.ko");
    let entries = fs::read_dir(&lkm_dir).ok()?;
    let mut best: Option<PathBuf> = None;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(&suffix) {
            best = Some(entry.path());
            break;
        }
    }

    best
}

/// Read the address of a kernel symbol from /proc/kallsyms.
/// Returns the hex address string (e.g., "0xffffffc010abcdef").
fn read_kallsyms_address(symbol: &str) -> Option<String> {
    let content = fs::read_to_string("/proc/kallsyms").ok()?;
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[2] == symbol {
            return Some(format!("0x{}", parts[0]));
        }
    }
    None
}

fn has_ksud_nuke() -> bool {
    run_command_with_timeout(
        Command::new("ksud").args(["kernel", "nuke-ext4-sysfs", "--help"]),
        CMD_TIMEOUT,
    )
    .map(|o| !String::from_utf8_lossy(&o.stderr).contains("unknown"))
    .unwrap_or(false)
}

fn try_apex_spoof(base_path: &Path) -> Result<(PathBuf, PathBuf)> {
    if !is_dir_writable("/apex") {
        bail!("/apex not writable");
    }

    let versioned = PathBuf::from(format!("/apex/{}@1", APEX_SPOOF_NAME));
    let facade = PathBuf::from(format!("/apex/{}", APEX_SPOOF_NAME));

    fs::create_dir_all(&versioned)?;

    // MS_MOVE ext4 mount to versioned APEX path
    let c_source = CString::new(base_path.as_os_str().as_encoded_bytes())?;
    let c_target = CString::new(versioned.as_os_str().as_encoded_bytes())?;

    let ret = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        let errno = std::io::Error::last_os_error();
        let _ = fs::remove_dir(&versioned);
        bail!("MS_MOVE to {}: {}", versioned.display(), errno);
    }

    fs::create_dir_all(&facade)?;

    // RO bind mount as facade
    let c_versioned = CString::new(versioned.as_os_str().as_encoded_bytes())?;
    let c_facade = CString::new(facade.as_os_str().as_encoded_bytes())?;

    let ret = unsafe {
        libc::mount(
            c_versioned.as_ptr(),
            c_facade.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        warn!(error = %std::io::Error::last_os_error(), "facade bind mount failed (non-fatal)");
    } else {
        // Remount RO
        let _ = unsafe {
            libc::mount(
                std::ptr::null(),
                c_facade.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
                std::ptr::null(),
            )
        };
    }

    // Symlink original path to versioned for compatibility
    if let Err(e) = std::os::unix::fs::symlink(&versioned, base_path) {
        debug!(error = %e, "symlink from base_path to apex (non-fatal)");
    }

    // Nuke backing file
    let image_path = base_path.with_extension("ext4.img");
    let _ = nuke_backing_file(&image_path);

    info!(versioned = %versioned.display(), facade = %facade.display(), "APEX spoof active");
    Ok((versioned, facade))
}
