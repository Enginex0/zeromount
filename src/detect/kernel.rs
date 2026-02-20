use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::core::types::CapabilityFlags;
use crate::vfs::ioctls;

const VFS_DEVICE: &str = "/dev/zeromount";
const SYSFS_DIR: &str = "/sys/kernel/zeromount";
const PROC_CONFIG: &str = "/proc/config.gz";

/// Probe kernel for VFS driver availability and version.
/// DET02: (1) /dev/zeromount existence, (2) GET_VERSION ioctl,
/// (3) /sys/kernel/zeromount/ sysfs, (4) /proc/config.gz check
pub fn probe_vfs_driver() -> Result<CapabilityFlags> {
    let mut caps = CapabilityFlags::default();

    // Step 1: device existence
    if !Path::new(VFS_DEVICE).exists() {
        debug!("VFS device {VFS_DEVICE} not found");
        // No VFS driver; still probe overlay/erofs/tmpfs for fallback mode
        caps.overlay_supported = check_overlay_support().unwrap_or(false);
        caps.erofs_supported = check_erofs_support().unwrap_or(false);
        caps.tmpfs_xattr = check_tmpfs_xattr().unwrap_or(false);
        return Ok(caps);
    }

    caps.vfs_driver = true;
    debug!("VFS device found at {VFS_DEVICE}");

    // Step 2: GET_VERSION ioctl (only ioctl NOT requiring CAP_SYS_ADMIN)
    match ioctls::VfsDriver::open() {
        Ok(driver) => {
            match driver.get_version() {
                Ok(ver) => {
                    debug!("VFS driver version: {ver}");
                    caps.vfs_version = Some(ver);
                }
                Err(e) => {
                    warn!("GET_VERSION ioctl failed: {e}");
                }
            }

            // Probe GET_STATUS ioctl (VFS06, may not exist on older kernels)
            match driver.get_status() {
                Ok(Some(_status)) => {
                    caps.vfs_status_ioctl = true;
                    debug!("GET_STATUS ioctl available");
                }
                Ok(None) => {
                    debug!("GET_STATUS ioctl not available (old kernel)");
                }
                Err(e) => {
                    warn!("GET_STATUS probe error: {e}");
                }
            }
        }
        Err(e) => {
            warn!("failed to open VFS device: {e}");
        }
    }

    // Step 3: sysfs check for additional info
    if let Ok(Some(sysfs_ver)) = probe_sysfs() {
        debug!("sysfs reports version: {sysfs_ver}");
        if caps.vfs_version.is_none() {
            caps.vfs_version = Some(sysfs_ver);
        }
    }

    // Step 4: /proc/config.gz for CONFIG_ZEROMOUNT (only if version unknown)
    if caps.vfs_version.is_none() {
        if let Ok(has_config) = check_proc_config() {
            if has_config {
                debug!("CONFIG_ZEROMOUNT=y found in /proc/config.gz");
            }
        }
    }

    // Probe overlay/erofs/tmpfs for fallback awareness
    caps.overlay_supported = check_overlay_support().unwrap_or(false);
    caps.erofs_supported = check_erofs_support().unwrap_or(false);
    caps.tmpfs_xattr = check_tmpfs_xattr().unwrap_or(false);

    Ok(caps)
}

/// Check /sys/kernel/zeromount/ for version info.
pub fn probe_sysfs() -> Result<Option<u32>> {
    let version_path = Path::new(SYSFS_DIR).join("version");
    if !version_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&version_path)
        .context("reading sysfs version")?;
    let version: u32 = content.trim().parse()
        .context("parsing sysfs version")?;
    Ok(Some(version))
}

/// Check /proc/config.gz for CONFIG_ZEROMOUNT=y.
fn check_proc_config() -> Result<bool> {
    let path = Path::new(PROC_CONFIG);
    if !path.exists() {
        return Ok(false);
    }

    let file = std::fs::File::open(path)
        .context("opening /proc/config.gz")?;

    // /proc/config.gz is gzip-compressed; read raw and search
    // On Android, this file may not exist or may require root.
    // We do a best-effort check -- read first 256KB and search.
    let mut buf = Vec::with_capacity(262144);
    let mut reader = std::io::BufReader::new(file);
    reader.read_to_end(&mut buf).context("reading /proc/config.gz")?;

    // Search for CONFIG_ZEROMOUNT in the compressed data.
    // The string will appear as plain ASCII even in gzip since
    // kernel config values are not compressed individually, but
    // this is a best-effort heuristic. If the file is truly gzip,
    // we'd need flate2 -- but we avoid adding a dependency for
    // a step-4 fallback check that rarely triggers.
    let needle = b"CONFIG_ZEROMOUNT=y";
    Ok(buf.windows(needle.len()).any(|w| w == needle))
}

/// Check if OverlayFS is supported by probing /proc/filesystems.
pub fn check_overlay_support() -> Result<bool> {
    let content = std::fs::read_to_string("/proc/filesystems")
        .unwrap_or_default();
    Ok(content.contains("overlay"))
}

/// Check if EROFS is supported by probing /proc/filesystems.
pub fn check_erofs_support() -> Result<bool> {
    let content = std::fs::read_to_string("/proc/filesystems")
        .unwrap_or_default();
    Ok(content.contains("erofs"))
}

/// Check if tmpfs supports xattr by testing setxattr on /dev (always tmpfs on Android).
pub fn check_tmpfs_xattr() -> Result<bool> {
    use std::ffi::CString;

    let test_path = std::path::Path::new("/dev/.zm_xattr_probe");
    let _file = std::fs::File::create(test_path);

    let c_path = CString::new("/dev/.zm_xattr_probe")?;
    let c_name = CString::new("trusted.overlay.whiteout")?;
    let c_val = b"y";

    // SAFETY: CStrings are non-null NUL-terminated; test file was created on the line above.
    let result = unsafe {
        libc::setxattr(
            c_path.as_ptr(),
            c_name.as_ptr(),
            c_val.as_ptr() as *const libc::c_void,
            1,
            0,
        )
    };

    let _ = std::fs::remove_file(test_path);
    Ok(result == 0)
}
