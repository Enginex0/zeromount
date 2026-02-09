use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_CONFIG_PATH: &str = "/data/adb/zeromount/config.toml";
const BACKUP_CONFIG_PATH: &str = "/data/adb/zeromount/config.toml.bak";
const BOOTCOUNT_PATH: &str = "/data/adb/zeromount/.bootcount";
const BOOTLOOP_THRESHOLD: u32 = 3;

// -- Top-level config --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroMountConfig {
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub mount: MountConfig,
    #[serde(default)]
    pub susfs: SusfsConfig,
    #[serde(default)]
    pub brene: BreneConfig,
    #[serde(default)]
    pub uname: UnameConfig,
    #[serde(default)]
    pub per_module: HashMap<String, ModuleOverrides>,
}

// -- Logging --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default)]
    pub verbose: bool,
    #[serde(default = "default_log_dir")]
    pub log_dir: PathBuf,
    #[serde(default = "default_max_log_size")]
    pub max_log_size_mb: u32,
    #[serde(default = "default_max_log_files")]
    pub max_log_files: u32,
}

fn default_log_dir() -> PathBuf {
    PathBuf::from("/data/adb/zeromount/logs")
}

fn default_max_log_size() -> u32 {
    2
}

fn default_max_log_files() -> u32 {
    3
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            verbose: false,
            log_dir: default_log_dir(),
            max_log_size_mb: default_max_log_size(),
            max_log_files: default_max_log_files(),
        }
    }
}

// -- Mount --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    #[serde(default)]
    pub storage_mode: StorageMode,
    #[serde(default = "default_true")]
    pub overlay_preferred: bool,
    #[serde(default = "default_true")]
    pub magic_mount_fallback: bool,
    #[serde(default = "default_true")]
    pub random_mount_paths: bool,
    #[serde(default = "default_auto")]
    pub mount_source: String,
    #[serde(default = "default_auto")]
    pub overlay_source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Auto,
    Erofs,
    Tmpfs,
    Ext4,
}

impl Default for StorageMode {
    fn default() -> Self {
        Self::Auto
    }
}

impl Default for MountConfig {
    fn default() -> Self {
        Self {
            storage_mode: StorageMode::Auto,
            overlay_preferred: true,
            magic_mount_fallback: true,
            random_mount_paths: true,
            mount_source: default_auto(),
            overlay_source: default_auto(),
        }
    }
}

// -- SUSFS --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SusfsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub kstat: bool,
    #[serde(default = "default_true")]
    pub path_hide: bool,
    #[serde(default = "default_true")]
    pub maps_hide: bool,
    #[serde(default = "default_true")]
    pub open_redirect: bool,
}

impl Default for SusfsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            kstat: true,
            path_hide: true,
            maps_hide: true,
            open_redirect: true,
        }
    }
}

// -- BRENE --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreneConfig {
    #[serde(default = "default_true")]
    pub auto_hide_apk: bool,
    #[serde(default = "default_true")]
    pub auto_hide_zygisk: bool,
    #[serde(default = "default_true")]
    pub auto_hide_fonts: bool,
    #[serde(default = "default_true")]
    pub auto_hide_rooted_folders: bool,
    #[serde(default = "default_true")]
    pub auto_hide_recovery: bool,
    #[serde(default = "default_true")]
    pub auto_hide_tmp: bool,
    #[serde(default = "default_true")]
    pub auto_hide_sdcard_data: bool,
    #[serde(default)]
    pub avc_log_spoofing: bool,
    #[serde(default)]
    pub susfs_log: bool,
    #[serde(default = "default_true")]
    pub hide_sus_mounts: bool,
    #[serde(default = "default_true")]
    pub emulate_vold_app_data: bool,
    #[serde(default = "default_true")]
    pub force_hide_lsposed: bool,
    #[serde(default)]
    pub custom_sus_paths: Vec<String>,
    #[serde(default)]
    pub custom_sus_maps: Vec<String>,
    #[serde(default)]
    pub custom_sus_path_loops: Vec<String>,
}

impl Default for BreneConfig {
    fn default() -> Self {
        Self {
            auto_hide_apk: true,
            auto_hide_zygisk: true,
            auto_hide_fonts: true,
            auto_hide_rooted_folders: true,
            auto_hide_recovery: true,
            auto_hide_tmp: true,
            auto_hide_sdcard_data: true,
            avc_log_spoofing: false,
            susfs_log: false,
            hide_sus_mounts: true,
            emulate_vold_app_data: true,
            force_hide_lsposed: true,
            custom_sus_paths: Vec::new(),
            custom_sus_maps: Vec::new(),
            custom_sus_path_loops: Vec::new(),
        }
    }
}

// -- Uname spoofing --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnameMode {
    Disabled,
    Static,
    Dynamic,
}

impl Default for UnameMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnameConfig {
    #[serde(default)]
    pub mode: UnameMode,
    #[serde(default)]
    pub release: String,
    #[serde(default)]
    pub version: String,
}

impl Default for UnameConfig {
    fn default() -> Self {
        Self {
            mode: UnameMode::Disabled,
            release: String::new(),
            version: String::new(),
        }
    }
}

// -- Per-module overrides --

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleOverrides {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub force_overlay: Option<bool>,
    #[serde(default)]
    pub force_magic: Option<bool>,
    #[serde(default)]
    pub force_strategy: Option<String>,
    #[serde(default)]
    pub skip_susfs: bool,
    #[serde(default)]
    pub exclude_partitions: Vec<String>,
    #[serde(default)]
    pub disable_overlay: bool,
    #[serde(default)]
    pub force_magic_mount: bool,
}

impl Default for ZeroMountConfig {
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            mount: MountConfig::default(),
            susfs: SusfsConfig::default(),
            brene: BreneConfig::default(),
            uname: UnameConfig::default(),
            per_module: HashMap::new(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_auto() -> String {
    "auto".to_string()
}

// -- 3-layer resolution --

impl ZeroMountConfig {
    /// Layer 1: compiled defaults. Layer 2: config file. Layer 3: CLI overrides.
    #[allow(dead_code)] // Public API for 3-layer config resolution
    pub fn resolve(path: Option<&Path>, overrides: &HashMap<String, String>) -> Result<Self> {
        let mut config = Self::load(path)?;
        for (key, value) in overrides {
            config
                .set(key, value)
                .with_context(|| format!("applying CLI override {key}={value}"))?;
        }
        Ok(config)
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("parsing {}", config_path.display()))?;
        Ok(config)
    }

    pub fn save(&self, path: Option<&Path>) -> Result<()> {
        let config_path = path
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating dir {}", parent.display()))?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)
            .with_context(|| format!("writing {}", config_path.display()))?;
        Ok(())
    }

    // -- Bootloop resilience (ME13) --

    /// Backup config before pipeline. Called by mount handler.
    pub fn backup() -> Result<()> {
        let src = Path::new(DEFAULT_CONFIG_PATH);
        if src.exists() {
            std::fs::copy(src, BACKUP_CONFIG_PATH).context("backing up config.toml")?;
        }
        Ok(())
    }

    /// Restore from backup on bootloop recovery.
    pub fn restore_backup() -> Result<Self> {
        let backup = Path::new(BACKUP_CONFIG_PATH);
        if !backup.exists() {
            tracing::warn!("no backup config found, using defaults");
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(backup).context("reading config backup")?;
        let config: Self = toml::from_str(&content).context("parsing config backup")?;
        Self::reset_bootcount()?;
        tracing::info!("config restored from backup");
        Ok(config)
    }

    /// Read bootloop counter from .bootcount file.
    pub fn read_bootcount() -> u32 {
        std::fs::read_to_string(BOOTCOUNT_PATH)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0)
    }

    /// Increment bootcount. Returns new count.
    pub fn increment_bootcount() -> Result<u32> {
        let count = Self::read_bootcount() + 1;
        if let Some(parent) = Path::new(BOOTCOUNT_PATH).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(BOOTCOUNT_PATH, count.to_string()).context("writing bootcount")?;
        Ok(count)
    }

    /// Reset bootcount to 0 (after successful boot).
    pub fn reset_bootcount() -> Result<()> {
        let _ = std::fs::remove_file(BOOTCOUNT_PATH);
        Ok(())
    }

    /// Check if we're in a bootloop. If count >= threshold, restore backup.
    pub fn check_bootloop() -> Result<bool> {
        let count = Self::read_bootcount();
        if count >= BOOTLOOP_THRESHOLD {
            tracing::error!("bootloop detected ({count} consecutive failures)");
            return Ok(true);
        }
        Ok(false)
    }

    // -- Dot-notation get/set --

    pub fn get(&self, key: &str) -> Option<String> {
        match key {
            // logging.*
            "logging.verbose" => Some(self.logging.verbose.to_string()),
            "logging.log_dir" => Some(self.logging.log_dir.display().to_string()),
            "logging.max_log_size_mb" => Some(self.logging.max_log_size_mb.to_string()),
            "logging.max_log_files" => Some(self.logging.max_log_files.to_string()),

            // mount.*
            "mount.storage_mode" => Some(storage_mode_str(self.mount.storage_mode).to_string()),
            "mount.overlay_preferred" => Some(self.mount.overlay_preferred.to_string()),
            "mount.magic_mount_fallback" => Some(self.mount.magic_mount_fallback.to_string()),
            "mount.random_mount_paths" => Some(self.mount.random_mount_paths.to_string()),
            "mount.mount_source" => Some(self.mount.mount_source.clone()),
            "mount.overlay_source" => Some(self.mount.overlay_source.clone()),

            // susfs.*
            "susfs.enabled" => Some(self.susfs.enabled.to_string()),
            "susfs.kstat" => Some(self.susfs.kstat.to_string()),
            "susfs.path_hide" => Some(self.susfs.path_hide.to_string()),
            "susfs.maps_hide" => Some(self.susfs.maps_hide.to_string()),
            "susfs.open_redirect" => Some(self.susfs.open_redirect.to_string()),

            // brene.*
            "brene.auto_hide_apk" => Some(self.brene.auto_hide_apk.to_string()),
            "brene.auto_hide_zygisk" => Some(self.brene.auto_hide_zygisk.to_string()),
            "brene.auto_hide_fonts" => Some(self.brene.auto_hide_fonts.to_string()),
            "brene.auto_hide_rooted_folders" => {
                Some(self.brene.auto_hide_rooted_folders.to_string())
            }
            "brene.auto_hide_recovery" => Some(self.brene.auto_hide_recovery.to_string()),
            "brene.auto_hide_tmp" => Some(self.brene.auto_hide_tmp.to_string()),
            "brene.auto_hide_sdcard_data" => Some(self.brene.auto_hide_sdcard_data.to_string()),
            "brene.avc_log_spoofing" => Some(self.brene.avc_log_spoofing.to_string()),
            "brene.susfs_log" => Some(self.brene.susfs_log.to_string()),
            "brene.hide_sus_mounts" => Some(self.brene.hide_sus_mounts.to_string()),
            "brene.emulate_vold_app_data" => Some(self.brene.emulate_vold_app_data.to_string()),
            "brene.force_hide_lsposed" => Some(self.brene.force_hide_lsposed.to_string()),
            "brene.custom_sus_paths" => Some(self.brene.custom_sus_paths.join(",")),
            "brene.custom_sus_maps" => Some(self.brene.custom_sus_maps.join(",")),
            "brene.custom_sus_path_loops" => Some(self.brene.custom_sus_path_loops.join(",")),

            // uname.*
            "uname.mode" => Some(uname_mode_str(self.uname.mode).to_string()),
            "uname.release" => Some(self.uname.release.clone()),
            "uname.version" => Some(self.uname.version.clone()),

            // per_module.<id>.<field>
            k if k.starts_with("per_module.") => self.get_module_key(k),

            _ => None,
        }
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            // logging.*
            "logging.verbose" => self.logging.verbose = value.parse()?,
            "logging.log_dir" => self.logging.log_dir = PathBuf::from(value),
            "logging.max_log_size_mb" => self.logging.max_log_size_mb = value.parse()?,
            "logging.max_log_files" => self.logging.max_log_files = value.parse()?,

            // mount.*
            "mount.storage_mode" => self.mount.storage_mode = parse_storage_mode(value)?,
            "mount.overlay_preferred" => self.mount.overlay_preferred = value.parse()?,
            "mount.magic_mount_fallback" => self.mount.magic_mount_fallback = value.parse()?,
            "mount.random_mount_paths" => self.mount.random_mount_paths = value.parse()?,
            "mount.mount_source" => self.mount.mount_source = value.to_string(),
            "mount.overlay_source" => self.mount.overlay_source = value.to_string(),

            // susfs.*
            "susfs.enabled" => self.susfs.enabled = value.parse()?,
            "susfs.kstat" => self.susfs.kstat = value.parse()?,
            "susfs.path_hide" => self.susfs.path_hide = value.parse()?,
            "susfs.maps_hide" => self.susfs.maps_hide = value.parse()?,
            "susfs.open_redirect" => self.susfs.open_redirect = value.parse()?,

            // brene.*
            "brene.auto_hide_apk" => self.brene.auto_hide_apk = value.parse()?,
            "brene.auto_hide_zygisk" => self.brene.auto_hide_zygisk = value.parse()?,
            "brene.auto_hide_fonts" => self.brene.auto_hide_fonts = value.parse()?,
            "brene.auto_hide_rooted_folders" => {
                self.brene.auto_hide_rooted_folders = value.parse()?
            }
            "brene.auto_hide_recovery" => self.brene.auto_hide_recovery = value.parse()?,
            "brene.auto_hide_tmp" => self.brene.auto_hide_tmp = value.parse()?,
            "brene.auto_hide_sdcard_data" => self.brene.auto_hide_sdcard_data = value.parse()?,
            "brene.avc_log_spoofing" => self.brene.avc_log_spoofing = value.parse()?,
            "brene.susfs_log" => self.brene.susfs_log = value.parse()?,
            "brene.hide_sus_mounts" => self.brene.hide_sus_mounts = value.parse()?,
            "brene.emulate_vold_app_data" => self.brene.emulate_vold_app_data = value.parse()?,
            "brene.force_hide_lsposed" => self.brene.force_hide_lsposed = value.parse()?,
            "brene.custom_sus_paths" => self.brene.custom_sus_paths = parse_csv(value),
            "brene.custom_sus_maps" => self.brene.custom_sus_maps = parse_csv(value),
            "brene.custom_sus_path_loops" => self.brene.custom_sus_path_loops = parse_csv(value),

            // uname.*
            "uname.mode" => self.uname.mode = parse_uname_mode(value)?,
            "uname.release" => self.uname.release = value.to_string(),
            "uname.version" => self.uname.version = value.to_string(),

            // per_module.<id>.<field>
            k if k.starts_with("per_module.") => self.set_module_key(k, value)?,

            _ => anyhow::bail!("unknown config key: {key}"),
        }
        Ok(())
    }

    fn get_module_key(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.splitn(3, '.').collect();
        if parts.len() != 3 {
            return None;
        }
        let module_id = parts[1];
        let field = parts[2];
        let rules = self.per_module.get(module_id)?;
        match field {
            "enabled" => rules.enabled.map(|b| b.to_string()),
            "force_overlay" => rules.force_overlay.map(|b| b.to_string()),
            "force_magic" => rules.force_magic.map(|b| b.to_string()),
            "force_strategy" => rules.force_strategy.clone(),
            "skip_susfs" => Some(rules.skip_susfs.to_string()),
            "exclude_partitions" => Some(rules.exclude_partitions.join(",")),
            "disable_overlay" => Some(rules.disable_overlay.to_string()),
            "force_magic_mount" => Some(rules.force_magic_mount.to_string()),
            _ => None,
        }
    }

    fn set_module_key(&mut self, key: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = key.splitn(3, '.').collect();
        if parts.len() != 3 {
            anyhow::bail!("invalid module key: {key} (expected per_module.<id>.<field>)");
        }
        let module_id = parts[1];
        let field = parts[2];
        let rules = self.per_module.entry(module_id.to_string()).or_default();
        match field {
            "enabled" => rules.enabled = parse_optional_bool(value),
            "force_overlay" => rules.force_overlay = parse_optional_bool(value),
            "force_magic" => rules.force_magic = parse_optional_bool(value),
            "force_strategy" => {
                rules.force_strategy = if value.is_empty() || value == "none" {
                    None
                } else {
                    Some(value.to_string())
                };
            }
            "skip_susfs" => rules.skip_susfs = value.parse()?,
            "exclude_partitions" => rules.exclude_partitions = parse_csv(value),
            "disable_overlay" => rules.disable_overlay = value.parse()?,
            "force_magic_mount" => rules.force_magic_mount = value.parse()?,
            _ => anyhow::bail!("unknown module override field: {field}"),
        }
        Ok(())
    }

    /// Get per-module overrides, falling back to empty defaults.
    #[allow(dead_code)] // Public API for per-module config lookup
    pub fn module_overrides(&self, module_id: &str) -> ModuleOverrides {
        self.per_module.get(module_id).cloned().unwrap_or_default()
    }
}

// -- Helpers --

fn storage_mode_str(mode: StorageMode) -> &'static str {
    match mode {
        StorageMode::Auto => "auto",
        StorageMode::Erofs => "erofs",
        StorageMode::Tmpfs => "tmpfs",
        StorageMode::Ext4 => "ext4",
    }
}

fn parse_storage_mode(s: &str) -> Result<StorageMode> {
    match s.to_lowercase().as_str() {
        "auto" => Ok(StorageMode::Auto),
        "erofs" => Ok(StorageMode::Erofs),
        "tmpfs" => Ok(StorageMode::Tmpfs),
        "ext4" => Ok(StorageMode::Ext4),
        _ => anyhow::bail!("invalid storage mode: {s} (expected: auto, erofs, tmpfs, ext4)"),
    }
}

fn uname_mode_str(mode: UnameMode) -> &'static str {
    match mode {
        UnameMode::Disabled => "disabled",
        UnameMode::Static => "static",
        UnameMode::Dynamic => "dynamic",
    }
}

fn parse_uname_mode(s: &str) -> Result<UnameMode> {
    match s.to_lowercase().as_str() {
        "disabled" => Ok(UnameMode::Disabled),
        "static" => Ok(UnameMode::Static),
        "dynamic" => Ok(UnameMode::Dynamic),
        _ => anyhow::bail!("invalid uname mode: {s} (expected: disabled, static, dynamic)"),
    }
}

fn parse_optional_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "none" | "" => None,
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// -- Tests --

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let config = ZeroMountConfig::default();
        assert!(!config.logging.verbose);
        assert_eq!(config.logging.max_log_size_mb, 2);
        assert_eq!(config.logging.max_log_files, 3);
        assert_eq!(config.mount.storage_mode, StorageMode::Auto);
        assert!(config.mount.overlay_preferred);
        assert!(config.mount.magic_mount_fallback);
        assert!(config.mount.random_mount_paths);
        assert_eq!(config.mount.mount_source, "auto");
        assert_eq!(config.mount.overlay_source, "auto");
        assert!(config.susfs.enabled);
        assert!(config.susfs.kstat);
        assert!(config.brene.auto_hide_apk);
        assert!(config.brene.auto_hide_zygisk);
        assert!(config.brene.auto_hide_fonts);
        assert!(config.brene.auto_hide_rooted_folders);
        assert!(config.brene.auto_hide_recovery);
        assert!(config.brene.auto_hide_tmp);
        assert!(config.brene.auto_hide_sdcard_data);
        assert!(!config.brene.avc_log_spoofing);
        assert!(!config.brene.susfs_log);
        assert!(config.brene.hide_sus_mounts);
        assert!(config.brene.emulate_vold_app_data);
        assert!(config.brene.force_hide_lsposed);
        assert_eq!(config.uname.mode, UnameMode::Disabled);
        assert!(config.per_module.is_empty());
    }

    #[test]
    fn get_set_roundtrip() {
        let mut config = ZeroMountConfig::default();

        config.set("logging.verbose", "true").unwrap();
        assert_eq!(config.get("logging.verbose").unwrap(), "true");

        config.set("logging.max_log_files", "5").unwrap();
        assert_eq!(config.get("logging.max_log_files").unwrap(), "5");

        config.set("mount.storage_mode", "erofs").unwrap();
        assert_eq!(config.get("mount.storage_mode").unwrap(), "erofs");

        config.set("mount.overlay_preferred", "false").unwrap();
        assert_eq!(config.get("mount.overlay_preferred").unwrap(), "false");

        config.set("mount.random_mount_paths", "false").unwrap();
        assert_eq!(config.get("mount.random_mount_paths").unwrap(), "false");

        config.set("susfs.kstat", "false").unwrap();
        assert_eq!(config.get("susfs.kstat").unwrap(), "false");

        config.set("brene.auto_hide_recovery", "false").unwrap();
        assert_eq!(config.get("brene.auto_hide_recovery").unwrap(), "false");

        config.set("uname.mode", "static").unwrap();
        assert_eq!(config.get("uname.mode").unwrap(), "static");

        config.set("uname.release", "5.10.0-gki").unwrap();
        assert_eq!(config.get("uname.release").unwrap(), "5.10.0-gki");
    }

    #[test]
    fn brene_csv_fields() {
        let mut config = ZeroMountConfig::default();
        config
            .set("brene.custom_sus_paths", "/data/adb/modules,/data/local/tmp")
            .unwrap();
        assert_eq!(
            config.brene.custom_sus_paths,
            vec!["/data/adb/modules", "/data/local/tmp"]
        );
        assert_eq!(
            config.get("brene.custom_sus_paths").unwrap(),
            "/data/adb/modules,/data/local/tmp"
        );
    }

    #[test]
    fn module_overrides_get_set() {
        let mut config = ZeroMountConfig::default();
        config.set("per_module.zygisk.enabled", "false").unwrap();
        config.set("per_module.zygisk.force_overlay", "true").unwrap();
        config.set("per_module.zygisk.force_magic", "false").unwrap();
        config.set("per_module.zygisk.skip_susfs", "true").unwrap();
        config
            .set("per_module.zygisk.force_strategy", "overlay")
            .unwrap();
        config
            .set("per_module.zygisk.force_magic_mount", "true")
            .unwrap();
        config
            .set("per_module.zygisk.exclude_partitions", "vendor,product")
            .unwrap();

        assert_eq!(config.get("per_module.zygisk.enabled").unwrap(), "false");
        assert_eq!(config.get("per_module.zygisk.force_overlay").unwrap(), "true");
        assert_eq!(config.get("per_module.zygisk.force_magic").unwrap(), "false");
        assert_eq!(config.get("per_module.zygisk.skip_susfs").unwrap(), "true");
        assert_eq!(
            config.get("per_module.zygisk.force_strategy").unwrap(),
            "overlay"
        );
        assert_eq!(
            config.get("per_module.zygisk.force_magic_mount").unwrap(),
            "true"
        );

        let overrides = config.module_overrides("zygisk");
        assert_eq!(overrides.enabled, Some(false));
        assert_eq!(overrides.force_overlay, Some(true));
        assert_eq!(overrides.force_magic, Some(false));
        assert!(overrides.skip_susfs);
        assert!(overrides.force_magic_mount);
        assert_eq!(overrides.force_strategy.as_deref(), Some("overlay"));
        assert_eq!(overrides.exclude_partitions, vec!["vendor", "product"]);
    }

    #[test]
    fn unknown_module_returns_defaults() {
        let config = ZeroMountConfig::default();
        let overrides = config.module_overrides("nonexistent");
        assert!(overrides.enabled.is_none());
        assert!(overrides.force_overlay.is_none());
        assert!(overrides.force_magic.is_none());
        assert!(!overrides.skip_susfs);
        assert!(overrides.force_strategy.is_none());
        assert!(!overrides.force_magic_mount);
    }

    #[test]
    fn toml_roundtrip() {
        let mut config = ZeroMountConfig::default();
        config.set("mount.storage_mode", "tmpfs").unwrap();
        config.set("susfs.maps_hide", "false").unwrap();
        config.set("brene.auto_hide_tmp", "true").unwrap();
        config.set("uname.mode", "dynamic").unwrap();
        config.set("uname.version", "#1 SMP").unwrap();
        config.set("per_module.test.skip_susfs", "true").unwrap();

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: ZeroMountConfig = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.mount.storage_mode, StorageMode::Tmpfs);
        assert!(!deserialized.susfs.maps_hide);
        assert!(deserialized.brene.auto_hide_tmp);
        assert_eq!(deserialized.uname.mode, UnameMode::Dynamic);
        assert_eq!(deserialized.uname.version, "#1 SMP");
        assert!(deserialized.module_overrides("test").skip_susfs);
    }

    #[test]
    fn partial_toml_fills_defaults() {
        let toml_str = r#"
[logging]
verbose = true

[susfs]
kstat = false
"#;
        let config: ZeroMountConfig = toml::from_str(toml_str).unwrap();
        assert!(config.logging.verbose);
        assert_eq!(config.logging.max_log_files, 3);
        assert!(!config.susfs.kstat);
        assert_eq!(config.mount.storage_mode, StorageMode::Auto);
        assert!(config.mount.overlay_preferred);
        assert!(config.mount.random_mount_paths);
        assert_eq!(config.mount.mount_source, "auto");
        assert_eq!(config.mount.overlay_source, "auto");
        assert!(config.susfs.enabled);
        assert!(config.brene.auto_hide_apk);
        assert!(config.brene.auto_hide_rooted_folders);
        assert_eq!(config.uname.mode, UnameMode::Disabled);
    }

    #[test]
    fn resolve_applies_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("logging.verbose".to_string(), "true".to_string());
        overrides.insert("mount.storage_mode".to_string(), "ext4".to_string());

        let config =
            ZeroMountConfig::resolve(Some(Path::new("/nonexistent")), &overrides).unwrap();
        assert!(config.logging.verbose);
        assert_eq!(config.mount.storage_mode, StorageMode::Ext4);
        assert!(config.brene.auto_hide_apk);
    }

    #[test]
    fn invalid_storage_mode_rejected() {
        let mut config = ZeroMountConfig::default();
        assert!(config.set("mount.storage_mode", "xfs").is_err());
    }

    #[test]
    fn invalid_uname_mode_rejected() {
        let mut config = ZeroMountConfig::default();
        assert!(config.set("uname.mode", "chaos").is_err());
    }

    #[test]
    fn unknown_key_rejected() {
        let mut config = ZeroMountConfig::default();
        assert!(config.set("nonexistent", "value").is_err());
    }

    #[test]
    fn mount_source_fields_roundtrip() {
        let mut config = ZeroMountConfig::default();
        assert_eq!(config.get("mount.mount_source").unwrap(), "auto");
        assert_eq!(config.get("mount.overlay_source").unwrap(), "auto");

        config.set("mount.mount_source", "tmpfs").unwrap();
        config.set("mount.overlay_source", "KSU").unwrap();
        assert_eq!(config.get("mount.mount_source").unwrap(), "tmpfs");
        assert_eq!(config.get("mount.overlay_source").unwrap(), "KSU");
    }
}
