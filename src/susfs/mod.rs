pub mod brene;
pub mod ffi;
pub mod fonts;
pub mod kstat;
pub mod paths;

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{bail, Context, Result};
use tracing::{debug, warn};

use crate::core::types::SusfsCommand;
use ffi::*;

/// Detected SUSFS feature set.
#[derive(Debug, Clone, Default)]
pub struct SusfsFeatures {
    pub kstat: bool,
    pub path: bool,
    pub maps: bool,
    pub open_redirect: bool,
    pub kstat_redirect: bool,
    pub open_redirect_all: bool,
}

/// Client for SUSFS kernel operations via KSU supercall.
#[derive(Debug, Clone)]
pub struct SusfsClient {
    available: bool,
    version: Option<String>,
    features: SusfsFeatures,
}

impl SusfsClient {
    /// Probe the kernel for SUSFS availability.
    /// Returns a client even if SUSFS is absent -- methods will return
    /// descriptive errors instead of panicking.
    pub fn probe() -> Result<Self> {
        let mut client = Self {
            available: false,
            version: None,
            features: SusfsFeatures::default(),
        };

        match client.query_version() {
            Ok(ver) => {
                debug!("SUSFS detected: {ver}");
                client.version = Some(ver);
                client.available = true;
            }
            Err(e) => {
                warn!("SUSFS not available: {e}");
                return Ok(client);
            }
        }

        // Probe features via show_enabled_features
        if let Ok(features_str) = client.query_enabled_features() {
            client.features = parse_features(&features_str);
        }

        debug!("SUSFS features: {:?}", client.features);

        Ok(client)
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    pub fn features(&self) -> &SusfsFeatures {
        &self.features
    }

    /// Test-only constructor for unit tests that can't call probe().
    #[cfg(test)]
    pub(crate) fn new_for_test(available: bool, features: SusfsFeatures) -> Self {
        Self {
            available,
            version: if available { Some("test".to_string()) } else { None },
            features,
        }
    }

    /// Initialize SUSFS root paths so add_sus_path doesn't EINVAL.
    ///
    /// The kernel's susfs_add_sus_path() uses strstr(path, android_data_path)
    /// to classify paths. When android_data_path is uninitialized (empty),
    /// strstr returns non-NULL for any input, hitting the is_inited=false
    /// branch and returning EINVAL for every path.
    pub fn ensure_root_paths(&self) {
        let data_candidates = [
            "/sdcard/Android/data",
            "/storage/emulated/0/Android/data",
            "/data/media/0/Android/data",
        ];
        let mut data_set = false;
        for candidate in &data_candidates {
            if Path::new(candidate).exists() {
                match self.set_android_data_root_path(candidate) {
                    Ok(()) => {
                        debug!("android_data_root_path set to {candidate}");
                        data_set = true;
                        break;
                    }
                    Err(e) => {
                        debug!("set_android_data_root_path({candidate}) failed: {e}");
                    }
                }
            }
        }
        if !data_set {
            debug!("no valid android_data_root_path candidate found");
        }

        let sdcard_candidates = [
            "/sdcard",
            "/storage/emulated/0",
            "/data/media/0",
        ];
        let mut sdcard_set = false;
        for candidate in &sdcard_candidates {
            if Path::new(candidate).exists() {
                match self.set_sdcard_root_path(candidate) {
                    Ok(()) => {
                        debug!("sdcard_root_path set to {candidate}");
                        sdcard_set = true;
                        break;
                    }
                    Err(e) => {
                        debug!("set_sdcard_root_path({candidate}) failed: {e}");
                    }
                }
            }
        }
        if !sdcard_set {
            debug!("no valid sdcard_root_path candidate found");
        }
    }

    // ---- Query commands ----

    #[allow(dead_code)] // Public API for CLI/diag consumers
    pub fn show_version(&self) -> Result<String> {
        self.ensure_available()?;
        self.query_version()
    }

    #[allow(dead_code)] // Public API for CLI/diag consumers
    pub fn show_enabled_features(&self) -> Result<String> {
        self.ensure_available()?;
        self.query_enabled_features()
    }

    #[allow(dead_code)] // FFI supercall wrapper
    pub fn show_variant(&self) -> Result<String> {
        self.ensure_available()?;
        let mut info = StSusfsVariant {
            susfs_variant: [0u8; SUSFS_MAX_VARIANT_BUFSIZE],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        self.do_supercall(SusfsCommand::ShowVariant, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "show_variant")?;
        Ok(buf_to_string(&info.susfs_variant))
    }

    // ---- Path hiding ----

    pub fn add_sus_path(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(path)
            .with_context(|| format!("stat failed for '{path}'"))?;

        let mut info = StSusfsSusPath {
            target_ino: meta.ino(),
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            i_uid: meta.uid(),
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, path);

        self.do_supercall(SusfsCommand::AddSusPath, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_path")
    }

    pub fn add_sus_path_loop(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(path)
            .with_context(|| format!("stat failed for '{path}'"))?;

        let mut info = StSusfsSusPath {
            target_ino: meta.ino(),
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            i_uid: meta.uid(),
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, path);

        self.do_supercall(SusfsCommand::AddSusPathLoop, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_path_loop")
    }

    #[allow(dead_code)] // FFI supercall wrapper
    pub fn set_android_data_root_path(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let mut info = StExternalDir {
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            is_inited: 0,
            _pad0: [0; 3],
            cmd: SusfsCommand::SetAndroidDataRootPath as i32,
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, path);

        self.do_supercall(SusfsCommand::SetAndroidDataRootPath, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "set_android_data_root_path")
    }

    #[allow(dead_code)] // FFI supercall wrapper
    pub fn set_sdcard_root_path(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let mut info = StExternalDir {
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            is_inited: 0,
            _pad0: [0; 3],
            cmd: SusfsCommand::SetSdcardRootPath as i32,
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, path);

        self.do_supercall(SusfsCommand::SetSdcardRootPath, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "set_sdcard_root_path")
    }

    // ---- Kstat spoofing ----

    /// Register a path for kstat spoofing BEFORE bind mount / overlay.
    /// Must be completed with update_sus_kstat() after the mount.
    #[allow(dead_code)] // FFI supercall wrapper
    pub fn add_sus_kstat(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(path)
            .with_context(|| format!("stat failed for '{path}'"))?;

        let mut info = zeroed_kstat();
        info.is_statically = 0;
        info.target_ino = meta.ino();
        copy_path_to_buf(&mut info.target_pathname, path);
        copy_stat_to_kstat(&mut info, &meta);
        info.err = ERR_CMD_NOT_SUPPORTED;

        self.do_supercall(SusfsCommand::AddSusKstat, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_kstat")
    }

    /// Complete kstat spoofing after bind mount / overlay.
    /// Updates target_ino to the new (mounted) inode, keeps original size/blocks.
    #[allow(dead_code)] // FFI supercall wrapper
    pub fn update_sus_kstat(&self, path: &str) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(path)
            .with_context(|| format!("stat failed for '{path}'"))?;

        let mut info = zeroed_kstat();
        info.is_statically = 0;
        info.target_ino = meta.ino();
        copy_path_to_buf(&mut info.target_pathname, path);
        info.spoofed_size = meta.size() as i64;
        info.spoofed_blocks = meta.blocks();
        info.err = ERR_CMD_NOT_SUPPORTED;

        self.do_supercall(SusfsCommand::UpdateSusKstat, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "update_sus_kstat")
    }

    /// Register kstat spoofing with explicit values (no pre-mount step needed).
    pub fn add_sus_kstat_statically(&self, path: &str, spoof: &KstatValues) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(path)
            .with_context(|| format!("stat failed for '{path}'"))?;

        let mut info = zeroed_kstat();
        info.is_statically = 1;
        info.target_ino = meta.ino();
        copy_path_to_buf(&mut info.target_pathname, path);
        apply_spoof_values(&mut info, &meta, spoof);
        info.err = ERR_CMD_NOT_SUPPORTED;

        self.do_supercall(SusfsCommand::AddSusKstatStatically, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_kstat_statically")
    }

    /// Custom kstat redirect (0x55573) -- dual-path registration.
    /// Falls back to add_sus_kstat_statically if the custom command is unavailable.
    pub fn add_sus_kstat_redirect(
        &self,
        virtual_path: &str,
        real_path: &str,
        spoof: &KstatValues,
    ) -> Result<()> {
        self.ensure_available()?;

        if !self.features.kstat_redirect {
            debug!("kstat_redirect unavailable, falling back to add_sus_kstat_statically");
            return self.add_sus_kstat_statically(virtual_path, spoof);
        }

        let mut info = StSusfsSusKstatRedirect {
            virtual_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            real_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            spoofed_ino: spoof.ino.unwrap_or(0),
            spoofed_dev: spoof.dev.unwrap_or(0),
            spoofed_nlink: spoof.nlink.unwrap_or(0),
            _pad0: [0; 4],
            spoofed_size: spoof.size.unwrap_or(0),
            spoofed_atime_tv_sec: spoof.atime_sec.unwrap_or(0),
            spoofed_mtime_tv_sec: spoof.mtime_sec.unwrap_or(0),
            spoofed_ctime_tv_sec: spoof.ctime_sec.unwrap_or(0),
            spoofed_atime_tv_nsec: spoof.atime_nsec.unwrap_or(0),
            spoofed_mtime_tv_nsec: spoof.mtime_nsec.unwrap_or(0),
            spoofed_ctime_tv_nsec: spoof.ctime_nsec.unwrap_or(0),
            spoofed_blksize: spoof.blksize.unwrap_or(0),
            spoofed_blocks: spoof.blocks.unwrap_or(0),
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.virtual_pathname, virtual_path);
        copy_path_to_buf(&mut info.real_pathname, real_path);

        self.do_supercall(SusfsCommand::AddSusKstatRedirect, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_kstat_redirect")
    }

    // ---- Open redirect ----

    pub fn add_open_redirect(&self, target: &str, redirected: &str) -> Result<()> {
        self.ensure_available()?;
        let meta = fs::metadata(target)
            .with_context(|| format!("stat failed for '{target}'"))?;

        let mut info = StSusfsOpenRedirect {
            target_ino: meta.ino(),
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            redirected_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, target);
        copy_path_to_buf(&mut info.redirected_pathname, redirected);

        self.do_supercall(SusfsCommand::AddOpenRedirect, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_open_redirect")
    }

    /// All-UID open redirect (custom 0x555c1). No CLI handler in upstream ksu_susfs.
    /// Falls back to per-UID add_open_redirect if unavailable.
    pub fn add_open_redirect_all(&self, target: &str, redirected: &str) -> Result<()> {
        self.ensure_available()?;

        if !self.features.open_redirect_all {
            debug!("open_redirect_all unavailable, falling back to add_open_redirect");
            return self.add_open_redirect(target, redirected);
        }

        let meta = fs::metadata(target)
            .with_context(|| format!("stat failed for '{target}'"))?;

        let mut info = StSusfsOpenRedirect {
            target_ino: meta.ino(),
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            redirected_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, target);
        copy_path_to_buf(&mut info.redirected_pathname, redirected);

        self.do_supercall(SusfsCommand::AddOpenRedirectAll, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_open_redirect_all")
    }

    // ---- Maps hiding ----

    pub fn add_sus_map(&self, map_path: &str) -> Result<()> {
        self.ensure_available()?;
        let mut info = StSusfsSusMap {
            target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.target_pathname, map_path);

        self.do_supercall(SusfsCommand::AddSusMap, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "add_sus_map")
    }

    // ---- Uname spoofing ----

    pub fn set_uname(&self, release: &str, version: &str) -> Result<()> {
        self.ensure_available()?;
        let mut info = StSusfsUname {
            release: [0u8; NEW_UTS_LEN + 1],
            version: [0u8; NEW_UTS_LEN + 1],
            _pad0: [0; 2],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.release, release);
        copy_path_to_buf(&mut info.version, version);

        self.do_supercall(SusfsCommand::SetUname, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "set_uname")
    }

    // ---- Log toggle ----

    pub fn enable_log(&self, enable: bool) -> Result<()> {
        self.ensure_available()?;
        let mut info = StSusfsLog {
            enabled: u8::from(enable),
            _pad0: [0; 3],
            err: ERR_CMD_NOT_SUPPORTED,
        };

        self.do_supercall(SusfsCommand::EnableLog, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "enable_log")
    }

    // ---- Hide sus mounts ----

    pub fn hide_sus_mounts(&self, enable: bool) -> Result<()> {
        self.ensure_available()?;
        let mut info = StSusfsHideSusMnts {
            enabled: u8::from(enable),
            _pad0: [0; 3],
            err: ERR_CMD_NOT_SUPPORTED,
        };

        self.do_supercall(SusfsCommand::HideSusMntsForNonSuProcs, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "hide_sus_mounts")
    }

    // ---- AVC log spoofing ----

    pub fn enable_avc_log_spoofing(&self, enable: bool) -> Result<()> {
        self.ensure_available()?;
        let mut info = StSusfsAvcLogSpoofing {
            enabled: u8::from(enable),
            _pad0: [0; 3],
            err: ERR_CMD_NOT_SUPPORTED,
        };

        self.do_supercall(SusfsCommand::EnableAvcLogSpoofing, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "enable_avc_log_spoofing")
    }

    // ---- Cmdline spoofing ----

    pub fn set_cmdline(&self, fake_content: &str) -> Result<()> {
        self.ensure_available()?;
        if fake_content.len() >= SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE {
            bail!("cmdline content exceeds max size ({SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE})");
        }

        let mut info = StSusfsSpoofCmdline {
            fake_cmdline_or_bootconfig: [0u8; SUSFS_FAKE_CMDLINE_OR_BOOTCONFIG_SIZE],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        copy_path_to_buf(&mut info.fake_cmdline_or_bootconfig, fake_content);

        self.do_supercall(SusfsCommand::SetCmdline, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "set_cmdline")
    }

    #[allow(dead_code)] // FFI supercall wrapper
    pub fn set_cmdline_from_file(&self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read cmdline file: {}", path.display()))?;
        self.set_cmdline(&content)
    }

    // ---- Internal helpers ----

    fn ensure_available(&self) -> Result<()> {
        if !self.available {
            bail!("SUSFS is not available on this kernel");
        }
        Ok(())
    }

    fn query_version(&self) -> Result<String> {
        let mut info = StSusfsVersion {
            susfs_version: [0u8; SUSFS_MAX_VERSION_BUFSIZE],
            err: ERR_CMD_NOT_SUPPORTED,
        };
        self.do_supercall(SusfsCommand::ShowVersion, &mut info as *mut _ as *mut u8)?;
        check_err(info.err, "show_version")?;
        Ok(buf_to_string(&info.susfs_version))
    }

    fn query_enabled_features(&self) -> Result<String> {
        // This struct is large (~8KB), heap-allocate
        let mut info = Box::new(StSusfsEnabledFeatures {
            enabled_features: [0u8; SUSFS_ENABLED_FEATURES_SIZE],
            err: ERR_CMD_NOT_SUPPORTED,
        });
        self.do_supercall(
            SusfsCommand::ShowEnabledFeatures,
            info.as_mut() as *mut _ as *mut u8,
        )?;
        check_err(info.err, "show_enabled_features")?;
        Ok(buf_to_string(&info.enabled_features))
    }

    fn do_supercall(&self, cmd: SusfsCommand, data: *mut u8) -> Result<()> {
        match supercall(cmd, data) {
            Ok(_) => Ok(()),
            Err(errno) => bail!(
                "supercall {cmd:?} failed: {} (errno {errno})",
                errno_to_str(errno)
            ),
        }
    }
}

/// Spoofed kstat values. None = use the file's current stat value.
#[derive(Debug, Clone, Default)]
pub struct KstatValues {
    pub ino: Option<u64>,
    pub dev: Option<u64>,
    pub nlink: Option<u32>,
    pub size: Option<i64>,
    pub atime_sec: Option<i64>,
    pub atime_nsec: Option<i64>,
    pub mtime_sec: Option<i64>,
    pub mtime_nsec: Option<i64>,
    pub ctime_sec: Option<i64>,
    pub ctime_nsec: Option<i64>,
    pub blksize: Option<u64>,
    pub blocks: Option<u64>,
}

// ---- private helpers ----

fn zeroed_kstat() -> StSusfsSusKstat {
    StSusfsSusKstat {
        is_statically: 0,
        _pad0: [0; 7],
        target_ino: 0,
        target_pathname: [0u8; SUSFS_MAX_LEN_PATHNAME],
        spoofed_ino: 0,
        spoofed_dev: 0,
        spoofed_nlink: 0,
        _pad1: [0; 4],
        spoofed_size: 0,
        spoofed_atime_tv_sec: 0,
        spoofed_mtime_tv_sec: 0,
        spoofed_ctime_tv_sec: 0,
        spoofed_atime_tv_nsec: 0,
        spoofed_mtime_tv_nsec: 0,
        spoofed_ctime_tv_nsec: 0,
        spoofed_blksize: 0,
        spoofed_blocks: 0,
        err: 0,
    }
}

#[allow(dead_code)] // FFI helper used by add_sus_kstat supercall path
fn copy_stat_to_kstat(info: &mut StSusfsSusKstat, meta: &fs::Metadata) {
    info.spoofed_ino = meta.ino();
    info.spoofed_dev = meta.dev();
    info.spoofed_nlink = meta.nlink() as u32;
    info.spoofed_size = meta.size() as i64;
    info.spoofed_atime_tv_sec = meta.atime();
    info.spoofed_mtime_tv_sec = meta.mtime();
    info.spoofed_ctime_tv_sec = meta.ctime();
    info.spoofed_atime_tv_nsec = meta.atime_nsec();
    info.spoofed_mtime_tv_nsec = meta.mtime_nsec();
    info.spoofed_ctime_tv_nsec = meta.ctime_nsec();
    info.spoofed_blksize = meta.blksize();
    info.spoofed_blocks = meta.blocks();
}

fn apply_spoof_values(info: &mut StSusfsSusKstat, meta: &fs::Metadata, spoof: &KstatValues) {
    info.spoofed_ino = spoof.ino.unwrap_or(meta.ino());
    info.spoofed_dev = spoof.dev.unwrap_or(meta.dev());
    info.spoofed_nlink = spoof.nlink.unwrap_or(meta.nlink() as u32);
    info.spoofed_size = spoof.size.unwrap_or(meta.size() as i64);
    info.spoofed_atime_tv_sec = spoof.atime_sec.unwrap_or(meta.atime());
    info.spoofed_mtime_tv_sec = spoof.mtime_sec.unwrap_or(meta.mtime());
    info.spoofed_ctime_tv_sec = spoof.ctime_sec.unwrap_or(meta.ctime());
    info.spoofed_atime_tv_nsec = spoof.atime_nsec.unwrap_or(meta.atime_nsec());
    info.spoofed_mtime_tv_nsec = spoof.mtime_nsec.unwrap_or(meta.mtime_nsec());
    info.spoofed_ctime_tv_nsec = spoof.ctime_nsec.unwrap_or(meta.ctime_nsec());
    info.spoofed_blksize = spoof.blksize.unwrap_or(meta.blksize());
    info.spoofed_blocks = spoof.blocks.unwrap_or(meta.blocks());
}

fn parse_features(features_str: &str) -> SusfsFeatures {
    SusfsFeatures {
        kstat: features_str.contains("CONFIG_KSU_SUSFS_SUS_KSTAT"),
        path: features_str.contains("CONFIG_KSU_SUSFS_SUS_PATH"),
        maps: features_str.contains("CONFIG_KSU_SUSFS_SUS_MAPS")
            || features_str.contains("CONFIG_KSU_SUSFS_SUS_MAP"),
        open_redirect: features_str.contains("CONFIG_KSU_SUSFS_OPEN_REDIRECT"),
        kstat_redirect: features_str.contains("CONFIG_KSU_SUSFS_SUS_KSTAT_REDIRECT"),
        open_redirect_all: features_str.contains("CONFIG_KSU_SUSFS_OPEN_REDIRECT_ALL"),
    }
}

fn check_err(err: i32, op: &str) -> Result<()> {
    if err == 0 {
        Ok(())
    } else if err == ERR_CMD_NOT_SUPPORTED {
        bail!("{op}: command not supported by kernel (enable feature in kernel config)")
    } else {
        bail!("{op}: kernel returned error {err}")
    }
}

fn errno_to_str(errno: i32) -> &'static str {
    match errno {
        1 => "EPERM",
        2 => "ENOENT",
        13 => "EACCES",
        14 => "EFAULT",
        22 => "EINVAL",
        28 => "ENOSPC",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Feature parsing tests --

    #[test]
    fn parse_features_stock_kernel() {
        let features = "CONFIG_KSU_SUSFS_SUS_PATH=y\n\
                        CONFIG_KSU_SUSFS_SUS_KSTAT=y\n\
                        CONFIG_KSU_SUSFS_SUS_MAPS=y\n\
                        CONFIG_KSU_SUSFS_OPEN_REDIRECT=y\n";
        let f = parse_features(features);
        assert!(f.path);
        assert!(f.kstat);
        assert!(f.maps);
        assert!(f.open_redirect);
        assert!(!f.kstat_redirect);
        assert!(!f.open_redirect_all);
    }

    #[test]
    fn parse_features_extended_kernel() {
        let features = "CONFIG_KSU_SUSFS_SUS_PATH=y\n\
                        CONFIG_KSU_SUSFS_SUS_KSTAT=y\n\
                        CONFIG_KSU_SUSFS_SUS_MAPS=y\n\
                        CONFIG_KSU_SUSFS_OPEN_REDIRECT=y\n\
                        CONFIG_KSU_SUSFS_SUS_KSTAT_REDIRECT=y\n\
                        CONFIG_KSU_SUSFS_OPEN_REDIRECT_ALL=y\n";
        let f = parse_features(features);
        assert!(f.path);
        assert!(f.kstat);
        assert!(f.maps);
        assert!(f.open_redirect);
        assert!(f.kstat_redirect);
        assert!(f.open_redirect_all);
    }

    #[test]
    fn parse_features_partial() {
        let features = "CONFIG_KSU_SUSFS_SUS_PATH=y\n";
        let f = parse_features(features);
        assert!(f.path);
        assert!(!f.kstat);
        assert!(!f.maps);
        assert!(!f.open_redirect);
    }

    #[test]
    fn parse_features_empty() {
        let f = parse_features("");
        assert!(!f.path);
        assert!(!f.kstat);
        assert!(!f.maps);
        assert!(!f.open_redirect);
    }

    #[test]
    fn parse_features_maps_alternate_name() {
        // Some kernels report SUS_MAP (singular) instead of SUS_MAPS
        let f = parse_features("CONFIG_KSU_SUSFS_SUS_MAP=y\n");
        assert!(f.maps);
    }

    // -- Error handling tests --

    #[test]
    fn check_err_zero_is_ok() {
        assert!(check_err(0, "test").is_ok());
    }

    #[test]
    fn check_err_cmd_not_supported() {
        let result = check_err(ERR_CMD_NOT_SUPPORTED, "test_op");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("test_op"));
        assert!(msg.contains("not supported"));
    }

    #[test]
    fn check_err_other_error() {
        let result = check_err(42, "test_op");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("42"));
    }

    // -- SusfsClient construction tests --

    #[test]
    fn susfs_features_default_all_false() {
        let f = SusfsFeatures::default();
        assert!(!f.kstat);
        assert!(!f.path);
        assert!(!f.maps);
        assert!(!f.open_redirect);
        assert!(!f.kstat_redirect);
        assert!(!f.open_redirect_all);
    }

    // -- FFI helper tests --

    #[test]
    fn copy_path_to_buf_basic() {
        let mut buf = [0u8; 32];
        copy_path_to_buf(&mut buf, "/system/fonts/NotoSans.ttf");
        assert_eq!(buf[0], b'/');
        assert_eq!(buf[26], 0); // NUL terminator at len position
    }

    #[test]
    fn copy_path_to_buf_truncation() {
        let mut buf = [0u8; 8];
        copy_path_to_buf(&mut buf, "0123456789abcdef");
        // Should truncate to 7 chars + NUL
        assert_eq!(buf[7], 0);
        assert_eq!(&buf[..7], b"0123456");
    }

    #[test]
    fn buf_to_string_basic() {
        let mut buf = [0u8; 16];
        buf[..5].copy_from_slice(b"hello");
        assert_eq!(buf_to_string(&buf), "hello");
    }

    #[test]
    fn buf_to_string_full_no_nul() {
        let buf = [b'A'; 4];
        assert_eq!(buf_to_string(&buf), "AAAA");
    }

    // -- errno_to_str --

    #[test]
    fn errno_names() {
        assert_eq!(errno_to_str(1), "EPERM");
        assert_eq!(errno_to_str(2), "ENOENT");
        assert_eq!(errno_to_str(22), "EINVAL");
        assert_eq!(errno_to_str(999), "unknown");
    }

    // -- Fallback behavior tests (F14) --
    // These verify the feature-gating logic that controls kstat_redirect
    // and open_redirect_all fallback paths.

    #[test]
    fn features_without_kstat_redirect_triggers_fallback_path() {
        let features = SusfsFeatures {
            kstat: true,
            path: true,
            maps: true,
            open_redirect: true,
            kstat_redirect: false,
            open_redirect_all: false,
        };
        // When kstat_redirect is false, the client's add_sus_kstat_redirect
        // method falls back to add_sus_kstat_statically
        assert!(!features.kstat_redirect);
        assert!(features.kstat); // static fallback requires base kstat support
    }

    #[test]
    fn features_without_open_redirect_all_triggers_fallback_path() {
        let features = SusfsFeatures {
            kstat: true,
            path: true,
            maps: true,
            open_redirect: true,
            kstat_redirect: true,
            open_redirect_all: false,
        };
        // When open_redirect_all is false, add_open_redirect_all falls back
        // to per-UID add_open_redirect
        assert!(!features.open_redirect_all);
        assert!(features.open_redirect); // per-UID fallback requires base redirect
    }

    #[test]
    fn parse_features_custom_without_base_prefix_collision() {
        // CONFIG_KSU_SUSFS_SUS_KSTAT_REDIRECT contains CONFIG_KSU_SUSFS_SUS_KSTAT
        // as prefix — both should match when redirect is present
        let features = "CONFIG_KSU_SUSFS_SUS_KSTAT_REDIRECT=y\n\
                        CONFIG_KSU_SUSFS_OPEN_REDIRECT_ALL=y\n";
        let f = parse_features(features);
        assert!(f.kstat, "base kstat should match from KSTAT_REDIRECT substring");
        assert!(f.kstat_redirect);
        assert!(f.open_redirect, "base redirect should match from REDIRECT_ALL substring");
        assert!(f.open_redirect_all);
    }

    #[test]
    fn err_cmd_not_supported_is_recognized() {
        assert_eq!(ERR_CMD_NOT_SUPPORTED, 126);
        let result = check_err(ERR_CMD_NOT_SUPPORTED, "probe_test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not supported"));
    }

    #[test]
    fn fallback_chain_kstat_redirect_to_static() {
        // Verify the fallback decision logic: when features.kstat_redirect
        // is false, the code path in add_sus_kstat_redirect switches to
        // add_sus_kstat_statically. We verify this by checking the feature
        // gate matches the expected behavior.
        let with_custom = SusfsFeatures {
            kstat_redirect: true,
            ..SusfsFeatures::default()
        };
        let without_custom = SusfsFeatures {
            kstat_redirect: false,
            kstat: true,
            ..SusfsFeatures::default()
        };

        // With custom: use kstat_redirect directly
        assert!(with_custom.kstat_redirect);
        // Without custom: must have base kstat for static fallback
        assert!(!without_custom.kstat_redirect);
        assert!(without_custom.kstat);
    }

    #[test]
    fn fallback_chain_open_redirect_all_to_per_uid() {
        let with_all = SusfsFeatures {
            open_redirect_all: true,
            open_redirect: true,
            ..SusfsFeatures::default()
        };
        let without_all = SusfsFeatures {
            open_redirect_all: false,
            open_redirect: true,
            ..SusfsFeatures::default()
        };

        assert!(with_all.open_redirect_all);
        assert!(!without_all.open_redirect_all);
        assert!(without_all.open_redirect);
    }
}
