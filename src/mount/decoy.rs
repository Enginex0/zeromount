use std::ffi::CString;
use std::fs;
use std::path::{Path, PathBuf};

use tracing::{debug, info, trace, warn};

/// Directories that are typically empty on Android devices -- safe to overmount with tmpfs.
const DECOY_CANDIDATES: &[&str] = &[
    "/oem",
    "/second_stage_resources",
    "/patch_hw",
    "/postinstall",
    "/system_dlkm",
    "/oem_dlkm",
    "/acct",
];

/// Mount a tmpfs on a suitable empty directory for use as a decoy overlay lowerdir.
/// Returns the chosen directory path on success, or None if no candidate works.
pub fn setup_decoy() -> Option<PathBuf> {
    trace!("decoy: scanning {} candidates", DECOY_CANDIDATES.len());

    for candidate in DECOY_CANDIDATES {
        let p = Path::new(candidate);
        if !p.is_dir() {
            trace!("decoy: {candidate} not a directory, skip");
            continue;
        }
        if let Ok(entries) = fs::read_dir(p) {
            let count = entries.count();
            if count != 0 {
                trace!("decoy: {candidate} has {count} entries, skip");
                continue;
            }
        }

        let original_ctx = get_selinux_context_raw(p);
        let ctx_str = original_ctx.as_ref().map(|c| {
            let s = String::from_utf8_lossy(c);
            s.trim_end_matches('\0').to_string()
        });
        trace!(
            "decoy: {candidate} original selinux={}",
            ctx_str.as_deref().unwrap_or("none")
        );

        let cstr = CString::new(*candidate).unwrap();
        let fstype = CString::new("tmpfs").unwrap();
        let source = CString::new("tmpfs").unwrap();

        // rootcontext= sets the root inode's SELinux label at mount time,
        // bypassing the lsetxattr policy check that blocks post-mount relabeling
        let mount_data = ctx_str
            .as_ref()
            .and_then(|ctx| CString::new(format!("rootcontext={ctx}")).ok());
        let data_ptr = mount_data
            .as_ref()
            .map(|d| d.as_ptr() as *const libc::c_void)
            .unwrap_or(std::ptr::null());

        // SAFETY: CStrings are non-null NUL-terminated; data_ptr is valid or null.
        let ret = unsafe {
            libc::mount(source.as_ptr(), cstr.as_ptr(), fstype.as_ptr(), 0, data_ptr)
        };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            trace!("decoy: rootcontext mount failed on {candidate}: {err}, trying plain");
            // SAFETY: CStrings are non-null NUL-terminated; null pointer for mount(2) data is valid.
            let ret = unsafe {
                libc::mount(
                    source.as_ptr(),
                    cstr.as_ptr(),
                    fstype.as_ptr(),
                    0,
                    std::ptr::null(),
                )
            };
            if ret != 0 {
                let err = std::io::Error::last_os_error();
                debug!("decoy: tmpfs mount failed on {candidate}: {err}");
                continue;
            }
            info!("decoy: tmpfs mounted on {candidate} (no rootcontext)");
        } else {
            info!(
                "decoy: tmpfs mounted on {candidate} (rootcontext={})",
                ctx_str.as_deref().unwrap_or("?")
            );
        }
        return Some(p.to_path_buf());
    }

    warn!(
        "decoy: no suitable candidate found from {} options",
        DECOY_CANDIDATES.len()
    );
    None
}

/// Lazy-unmount the decoy tmpfs.
pub fn teardown_decoy(decoy: &Path) {
    trace!("decoy: tearing down tmpfs at {}", decoy.display());
    let cstr = CString::new(decoy.to_string_lossy().as_bytes()).unwrap();
    // SAFETY: CString is non-null NUL-terminated; MNT_DETACH is a valid umount2 flag.
    let ret = unsafe { libc::umount2(cstr.as_ptr(), libc::MNT_DETACH) };
    if ret != 0 {
        warn!(
            "decoy: umount2 MNT_DETACH failed on {}: {}",
            decoy.display(),
            std::io::Error::last_os_error()
        );
    } else {
        debug!("decoy: tmpfs torn down at {}", decoy.display());
    }
}

/// Mirror SELinux contexts from the real filesystem onto decoy subdirectories.
/// Walks each path component, copies the real context, falls back to system_file:s0.
pub fn mirror_decoy_selinux(decoy_base: &Path, target: &Path) {
    let rel = target.strip_prefix("/").unwrap_or(target);
    let mut current = decoy_base.to_path_buf();
    let mut real = PathBuf::from("/");

    trace!(
        "decoy selinux: mirroring {} -> {}/{}",
        target.display(),
        decoy_base.display(),
        rel.display()
    );

    for component in rel.components() {
        real.push(component);
        current.push(component);
        if current.is_dir() {
            if let Some(ctx) = get_selinux_context_raw(&real) {
                let ctx_str = String::from_utf8_lossy(&ctx);
                set_selinux_context_raw(&current, &ctx);
                trace!(
                    "decoy selinux: {} -> {} (from {})",
                    current.display(),
                    ctx_str,
                    real.display()
                );
            } else {
                let fallback = b"u:object_r:system_file:s0";
                set_selinux_context_raw(&current, fallback);
                trace!(
                    "decoy selinux: {} -> system_file:s0 (fallback, {} not found)",
                    current.display(),
                    real.display(),
                );
            }
        }
    }
}

/// Read the SELinux security.selinux xattr from a path. Returns raw bytes or None.
fn get_selinux_context_raw(path: &Path) -> Option<Vec<u8>> {
    let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let attr = b"security.selinux\0";
    let attr_ptr = attr.as_ptr() as *const libc::c_char;
    // SAFETY: CString is non-null NUL-terminated; attr is a static NUL-terminated byte literal.
    unsafe {
        let size = libc::lgetxattr(c_path.as_ptr(), attr_ptr, std::ptr::null_mut(), 0);
        if size <= 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let read = libc::lgetxattr(
            c_path.as_ptr(),
            attr_ptr,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
        );
        if read > 0 {
            buf.truncate(read as usize);
            Some(buf)
        } else {
            None
        }
    }
}

/// Set SELinux security.selinux xattr on a path. Best-effort, logs on failure.
fn set_selinux_context_raw(path: &Path, context: &[u8]) {
    let c_path = match CString::new(path.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let attr = b"security.selinux\0";
    // SAFETY: CString is non-null NUL-terminated; attr is a static NUL-terminated byte literal.
    let ret = unsafe {
        libc::lsetxattr(
            c_path.as_ptr(),
            attr.as_ptr() as *const libc::c_char,
            context.as_ptr() as *const libc::c_void,
            context.len(),
            0,
        )
    };
    if ret != 0 {
        trace!(
            "decoy selinux: lsetxattr failed on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
    }
}
