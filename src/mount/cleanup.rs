use std::ffi::CString;
use std::fs;

use anyhow::{Context, Result};
use tracing::{debug, info, trace};

/// Parse /proc/self/mountinfo for stale overlays (rw per-mount + ro super options)
/// and lazy-unmount them. Returns the number of cleaned mounts.
pub fn cleanup_stale_overlays() -> Result<usize> {
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").context("failed to read /proc/self/mountinfo")?;

    let mut cleaned = 0;
    let mut overlay_count = 0;

    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let mount_point = fields[4];
        let per_mount_opts = fields[5];

        // Find the separator "-" between mount options and filesystem type
        let sep_idx = fields.iter().position(|&f| f == "-");
        let Some(sep) = sep_idx else { continue };
        if sep + 3 > fields.len() {
            continue;
        }

        let fs_type = fields[sep + 1];
        let super_opts = fields[sep + 3];

        if fs_type != "overlay" {
            continue;
        }
        overlay_count += 1;

        let per_mount_rw = per_mount_opts.split(',').any(|o| o == "rw");
        let super_ro = super_opts.split(',').any(|o| o == "ro");

        if per_mount_rw && super_ro {
            debug!(mount_point, "stale overlay detected (rw per-mount + ro super)");
            let Some(target) = CString::new(mount_point).ok() else { continue; };
            // SAFETY: CString is non-null NUL-terminated; MNT_DETACH is a valid umount2 flag.
            let ret = unsafe { libc::umount2(target.as_ptr(), libc::MNT_DETACH) };
            if ret == 0 {
                cleaned += 1;
                info!(mount_point, "stale overlay removed");
            } else {
                trace!(
                    mount_point,
                    error = %std::io::Error::last_os_error(),
                    "stale overlay umount failed"
                );
            }
        }
    }

    debug!(
        overlays = overlay_count,
        cleaned,
        "stale overlay scan complete"
    );
    Ok(cleaned)
}
