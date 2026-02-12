use std::path::Path;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Once;

use anyhow::{bail, Result};
use tracing::{debug, info, warn};

const KSU_MAGIC1: u32 = 0xDEADBEEF;
const KSU_MAGIC2: u32 = 0xCAFEBABE;

// _IOC(_IOC_WRITE, 'K', 18, 0)
const KSU_IOCTL_ADD_TRY_UMOUNT: u32 = 0x4000_4B12;

// KSU infrastructure mounts that detection tools flag but modules can't prevent
const KSU_INFRA_PATHS: &[&str] = &[
    "/apex/com.android.art/javalib/core-libart.jar",
];

#[repr(C)]
struct KsuTryUmountCmd {
    arg: u64,
    flags: u32,
    mode: u8,
}

static DRIVER_FD: AtomicI32 = AtomicI32::new(-1);
static INIT: Once = Once::new();

fn acquire_driver_fd() -> i32 {
    INIT.call_once(|| {
        let mut fd: i32 = -1;
        let ret = unsafe {
            libc::syscall(
                libc::SYS_reboot,
                KSU_MAGIC1 as libc::c_long,
                KSU_MAGIC2 as libc::c_long,
                0 as libc::c_long,
                &mut fd as *mut i32 as libc::c_long,
            )
        };
        // KSU writes fd via pointer regardless of syscall return value.
        // Bare KSU kernels return ret=-1 from the supercall but fd IS valid.
        // SUSFS-patched kernels return ret=0. Check fd only.
        if fd >= 0 {
            debug!(fd, ret, "KSU driver fd acquired");
            DRIVER_FD.store(fd, Ordering::Release);
        } else {
            warn!(ret, fd, "KSU driver fd acquisition failed");
        }
    });
    DRIVER_FD.load(Ordering::Acquire)
}

fn send_unmountable(path: &str) -> Result<()> {
    let fd = acquire_driver_fd();
    if fd < 0 {
        bail!("KSU driver not available");
    }

    let c_path = std::ffi::CString::new(path)?;
    let cmd = KsuTryUmountCmd {
        arg: c_path.as_ptr() as u64,
        flags: 0x2,
        mode: 1, // add_to_list
    };

    let ret = unsafe {
        libc::ioctl(fd, KSU_IOCTL_ADD_TRY_UMOUNT as libc::Ioctl, &cmd as *const KsuTryUmountCmd)
    };

    if ret < 0 {
        let err = std::io::Error::last_os_error();
        bail!("try_umount ioctl failed for {path}: {err}");
    }

    debug!(path, "registered with try_umount");
    Ok(())
}

pub struct TryUmountStats {
    pub registered: u32,
    pub failed: u32,
}

/// Register mount paths with KSU's try_umount for per-app unmounting.
/// KSU reverses these mounts in the mount namespace of deny-list apps.
pub fn register_unmountable(mount_paths: &[String], root_manager_name: &str) -> TryUmountStats {
    if root_manager_name != "KernelSU" {
        debug!("try_umount skipped: root manager is {root_manager_name}, not KernelSU");
        return TryUmountStats { registered: 0, failed: 0 };
    }

    let mut registered = 0u32;
    let mut failed = 0u32;

    for path in mount_paths {
        match send_unmountable(path) {
            Ok(()) => registered += 1,
            Err(e) => {
                warn!(path = %path, error = %e, "try_umount registration failed");
                failed += 1;
            }
        }
    }

    if registered > 0 || failed > 0 {
        info!(registered, failed, "try_umount registration complete");
    }

    TryUmountStats { registered, failed }
}

/// Register KSU's own infrastructure mounts (e.g. ART jar injection)
/// so detection apps don't see them. Safe because ART loads modified jars
/// at zygote init before fork — try_umount in child namespace is harmless.
pub fn register_ksu_infra_mounts(root_manager_name: &str) -> TryUmountStats {
    if root_manager_name != "KernelSU" {
        return TryUmountStats { registered: 0, failed: 0 };
    }

    let mut registered = 0u32;
    let mut failed = 0u32;

    for &path in KSU_INFRA_PATHS {
        if !Path::new(path).exists() {
            continue;
        }
        // Only register if the path has a mount (check mountinfo)
        if !is_bind_mounted(path) {
            debug!(path, "KSU infra path not mounted, skipping");
            continue;
        }
        match send_unmountable(path) {
            Ok(()) => {
                debug!(path, "KSU infra path registered with try_umount");
                registered += 1;
            }
            Err(e) => {
                debug!(path, error = %e, "KSU infra try_umount failed");
                failed += 1;
            }
        }
    }

    if registered > 0 {
        info!(registered, "KSU infra mounts registered with try_umount");
    }

    TryUmountStats { registered, failed }
}

fn is_bind_mounted(path: &str) -> bool {
    let Ok(content) = std::fs::read_to_string("/proc/self/mountinfo") else {
        return false;
    };
    content.lines().any(|line| line.contains(path))
}
