use crate::core::types::SusfsCommand;

// Supercall magic values (from susfs_def.h + ksu_susfs/jni/main.c)
pub const KSU_INSTALL_MAGIC1: u64 = 0xDEADBEEF;
pub const SUSFS_MAGIC: u64 = 0xFAFAFAFA;

// Size constants matching susfs_def.h
pub const SUSFS_MAX_LEN_PATHNAME: usize = 256;
#[allow(dead_code)] // FFI constant matching susfs_def.h
pub const SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE: usize = 8192;
pub const SUSFS_ENABLED_FEATURES_SIZE: usize = 8192;
pub const SUSFS_MAX_VERSION_BUFSIZE: usize = 16;
#[allow(dead_code)] // FFI constant matching susfs_def.h
pub const SUSFS_MAX_VARIANT_BUFSIZE: usize = 16;
pub const NEW_UTS_LEN: usize = 64;

pub const ERR_CMD_NOT_SUPPORTED: i32 = 126;

// ------------------------------------------------------------------
// FFI structs -- #[repr(C)], field order matches susfs_def.h exactly.
// Target: aarch64-linux-android (LP64).
//   C `unsigned long`  = u64, `long` = i64
//   C `unsigned int`   = u32, `int`  = i32
//   C `long long`      = i64, `unsigned long long` = u64
//   C `bool`           = u8 (1 byte in C, but kernel bool is int-sized
//                         on most archs; the struct packing and the
//                         subsequent `int err` field handle alignment)
// ------------------------------------------------------------------

/// add_sus_path / add_sus_path_loop
/// C: struct st_susfs_sus_path (main.c:84-89)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsSusPath {
    pub target_ino: u64,
    pub target_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub i_uid: u32,
    pub err: i32,
}

/// set_android_data_root_path / set_sdcard_root_path
/// C: struct st_external_dir (main.c:91-96)
#[repr(C)]
#[derive(Clone)]
#[allow(dead_code)] // FFI struct matching kernel susfs_def.h
pub struct StExternalDir {
    pub target_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub is_inited: u8, // C bool
    pub _pad0: [u8; 3],
    pub cmd: i32,
    pub err: i32,
}

/// add_sus_kstat / update_sus_kstat / add_sus_kstat_statically
/// C: struct st_susfs_sus_kstat (main.c:103-120)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsSusKstat {
    pub is_statically: u8, // C bool
    pub _pad0: [u8; 7],   // align to 8 for next unsigned long
    pub target_ino: u64,
    pub target_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub spoofed_ino: u64,
    pub spoofed_dev: u64,
    pub spoofed_nlink: u32,
    pub _pad1: [u8; 4], // align to 8 for long long
    pub spoofed_size: i64,
    pub spoofed_atime_tv_sec: i64,
    pub spoofed_mtime_tv_sec: i64,
    pub spoofed_ctime_tv_sec: i64,
    pub spoofed_atime_tv_nsec: i64,
    pub spoofed_mtime_tv_nsec: i64,
    pub spoofed_ctime_tv_nsec: i64,
    pub spoofed_blksize: u64,
    pub spoofed_blocks: u64,
    pub err: i32,
}

/// add_sus_kstat_redirect (custom 0x55573)
/// C: struct st_susfs_sus_kstat_redirect (main.c:122-138)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsSusKstatRedirect {
    pub virtual_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub real_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub spoofed_ino: u64,
    pub spoofed_dev: u64,
    pub spoofed_nlink: u32,
    pub _pad0: [u8; 4],
    pub spoofed_size: i64,
    pub spoofed_atime_tv_sec: i64,
    pub spoofed_mtime_tv_sec: i64,
    pub spoofed_ctime_tv_sec: i64,
    pub spoofed_atime_tv_nsec: i64,
    pub spoofed_mtime_tv_nsec: i64,
    pub spoofed_ctime_tv_nsec: i64,
    pub spoofed_blksize: u64,
    pub spoofed_blocks: u64,
    pub err: i32,
}

/// set_uname
/// C: struct st_susfs_uname (main.c:140-144)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsUname {
    pub release: [u8; NEW_UTS_LEN + 1], // 65 bytes
    pub version: [u8; NEW_UTS_LEN + 1], // 65 bytes
    pub _pad0: [u8; 2],                 // align to 4 for int
    pub err: i32,
}

/// enable_log
/// C: struct st_susfs_log (main.c:146-149)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsLog {
    pub enabled: u8, // C bool
    pub _pad0: [u8; 3],
    pub err: i32,
}

/// set_cmdline_or_bootconfig
/// C: struct st_susfs_spoof_cmdline_or_bootconfig (main.c:151-154)
#[repr(C)]
#[derive(Clone)]
#[allow(dead_code)] // FFI struct matching kernel susfs_def.h
pub struct StSusfsSpoofCmdline {
    pub fake_cmdline_or_bootconfig: [u8; SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE],
    pub err: i32,
}

/// add_open_redirect / add_open_redirect_all
/// C: struct st_susfs_open_redirect (main.c:156-161)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsOpenRedirect {
    pub target_ino: u64,
    pub target_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub redirected_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub err: i32,
}

/// add_sus_map
/// C: struct st_susfs_sus_map (main.c:163-166)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsSusMap {
    pub target_pathname: [u8; SUSFS_MAX_LEN_PATHNAME],
    pub err: i32,
}

/// enable_avc_log_spoofing
/// C: struct st_susfs_avc_log_spoofing (main.c:168-171)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsAvcLogSpoofing {
    pub enabled: u8,
    pub _pad0: [u8; 3],
    pub err: i32,
}

/// show enabled_features
/// C: struct st_susfs_enabled_features (main.c:173-176)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsEnabledFeatures {
    pub enabled_features: [u8; SUSFS_ENABLED_FEATURES_SIZE],
    pub err: i32,
}

/// show variant
/// C: struct st_susfs_variant (main.c:178-181)
#[repr(C)]
#[derive(Clone)]
#[allow(dead_code)] // FFI struct matching kernel susfs_def.h
pub struct StSusfsVariant {
    pub susfs_variant: [u8; SUSFS_MAX_VARIANT_BUFSIZE],
    pub err: i32,
}

/// show version
/// C: struct st_susfs_version (main.c:183-186)
#[repr(C)]
#[derive(Clone)]
pub struct StSusfsVersion {
    pub susfs_version: [u8; SUSFS_MAX_VERSION_BUFSIZE],
    pub err: i32,
}

/// hide_sus_mnts_for_non_su_procs (included for completeness, not used by ZeroMount per S05)
#[repr(C)]
#[derive(Clone)]
#[allow(dead_code)] // FFI struct matching kernel susfs_def.h
pub struct StSusfsHideSusMnts {
    pub enabled: u8,
    pub _pad0: [u8; 3],
    pub err: i32,
}

// ---- supercall ----

/// Issue a SUSFS supercall via SYS_reboot with KSU magic numbers.
/// Returns Ok(ret) on syscall success, Err(errno) on syscall failure.
pub fn supercall(cmd: SusfsCommand, data: *mut u8) -> Result<i32, i32> {
    supercall_raw(cmd as u32, data)
}

// Accepts a raw command code for probing arbitrary/nonexistent commands
pub fn supercall_raw(cmd: u32, data: *mut u8) -> Result<i32, i32> {
    // SAFETY: syscall args are valid constants and data is a caller-provided mutable pointer.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_reboot as libc::c_long,
            KSU_INSTALL_MAGIC1 as libc::c_long,
            SUSFS_MAGIC as libc::c_long,
            cmd as libc::c_long,
            data as libc::c_long,
        )
    };
    if ret < 0 {
        Err(std::io::Error::last_os_error().raw_os_error().unwrap_or(-1))
    } else {
        Ok(ret as i32)
    }
}

// ---- helpers ----

/// Copy a Rust string into a fixed-size C byte buffer, NUL-terminated.
pub fn copy_path_to_buf(buf: &mut [u8], s: &str) {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len() - 1);
    buf[..len].copy_from_slice(&bytes[..len]);
    buf[len] = 0;
}

/// Read a NUL-terminated C string from a byte buffer.
pub fn buf_to_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    // Struct layout tests -- ensure #[repr(C)] structs match the C header.
    // These verify field offsets and total sizes on the build host.
    // On aarch64 (LP64), unsigned long = 8 bytes, long = 8 bytes.
    // On x86_64 (LP64), the sizes are the same, so tests pass on dev machines.

    #[test]
    fn sus_path_layout() {
        // C: unsigned long (8) + char[256] + unsigned int (4) + int (4) = 272
        assert_eq!(mem::size_of::<StSusfsSusPath>(), 272);
        assert_eq!(mem::offset_of!(StSusfsSusPath, target_ino), 0);
        assert_eq!(mem::offset_of!(StSusfsSusPath, target_pathname), 8);
        assert_eq!(mem::offset_of!(StSusfsSusPath, i_uid), 264);
        assert_eq!(mem::offset_of!(StSusfsSusPath, err), 268);
    }

    #[test]
    fn sus_map_layout() {
        // C: char[256] + int (4) = 260
        assert_eq!(mem::size_of::<StSusfsSusMap>(), 260);
    }

    #[test]
    fn open_redirect_layout() {
        // C: unsigned long (8) + char[256] + char[256] + int (4) = 524 content
        // + 4 bytes trailing padding for 8-byte alignment (due to u64 field) = 528
        let size = mem::size_of::<StSusfsOpenRedirect>();
        assert_eq!(size, 528);
    }

    #[test]
    fn version_layout() {
        // C: char[16] + int (4) = 20
        assert_eq!(mem::size_of::<StSusfsVersion>(), 20);
    }

    #[test]
    fn variant_layout() {
        // C: char[16] + int (4) = 20
        assert_eq!(mem::size_of::<StSusfsVariant>(), 20);
    }

    #[test]
    fn log_layout() {
        // C: bool (1) + padding (3) + int (4) = 8
        assert_eq!(mem::size_of::<StSusfsLog>(), 8);
    }

    #[test]
    fn avc_log_spoofing_layout() {
        // Same as log
        assert_eq!(mem::size_of::<StSusfsAvcLogSpoofing>(), 8);
    }

    #[test]
    fn uname_layout() {
        // C: char[65] + char[65] + padding(2) + int(4) = 136
        assert_eq!(mem::size_of::<StSusfsUname>(), 136);
    }

    #[test]
    fn enabled_features_layout() {
        // C: char[8192] + int (4) = 8196
        assert_eq!(mem::size_of::<StSusfsEnabledFeatures>(), 8196);
    }

    #[test]
    fn spoof_cmdline_layout() {
        // C: char[8192] + int (4) = 8196
        assert_eq!(mem::size_of::<StSusfsSpoofCmdline>(), 8196);
    }

    #[test]
    fn kstat_redirect_layout() {
        // C: char[256] + char[256] + unsigned long (8) + unsigned long (8)
        //    + unsigned int (4) + pad(4) + long long (8) + long*6 (48)
        //    + unsigned long (8) + unsigned long long (8) + int (4) + pad(4) = 616
        assert_eq!(mem::size_of::<StSusfsSusKstatRedirect>(), 616);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, virtual_pathname), 0);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, real_pathname), 256);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_ino), 512);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_dev), 520);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_nlink), 528);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, _pad0), 532);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_size), 536);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_atime_tv_sec), 544);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_mtime_tv_sec), 552);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_ctime_tv_sec), 560);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_atime_tv_nsec), 568);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_mtime_tv_nsec), 576);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_ctime_tv_nsec), 584);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_blksize), 592);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, spoofed_blocks), 600);
        assert_eq!(mem::offset_of!(StSusfsSusKstatRedirect, err), 608);
    }

    #[test]
    fn magic_constants() {
        assert_eq!(KSU_INSTALL_MAGIC1, 0xDEADBEEF);
        assert_eq!(SUSFS_MAGIC, 0xFAFAFAFA);
        assert_eq!(ERR_CMD_NOT_SUPPORTED, 126);
    }

    #[test]
    fn size_constants() {
        assert_eq!(SUSFS_MAX_LEN_PATHNAME, 256);
        assert_eq!(SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE, 8192);
        assert_eq!(SUSFS_ENABLED_FEATURES_SIZE, 8192);
        assert_eq!(SUSFS_MAX_VERSION_BUFSIZE, 16);
        assert_eq!(SUSFS_MAX_VARIANT_BUFSIZE, 16);
        assert_eq!(NEW_UTS_LEN, 64);
    }

}
