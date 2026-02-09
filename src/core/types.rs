use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

// -- Scenario --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Full,
    SusfsFrontend,
    KernelOnly,
    SusfsOnly,
    None,
}

impl Default for Scenario {
    fn default() -> Self {
        Self::None
    }
}

// -- Capability Flags --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityFlags {
    pub vfs_driver: bool,
    pub vfs_version: Option<u32>,
    pub vfs_status_ioctl: bool,
    pub susfs_available: bool,
    pub susfs_version: Option<String>,
    pub susfs_kstat: bool,
    pub susfs_path: bool,
    pub susfs_maps: bool,
    pub susfs_open_redirect: bool,
    pub susfs_kstat_redirect: bool,
    pub susfs_open_redirect_all: bool,
    pub overlay_supported: bool,
    pub erofs_supported: bool,
    pub tmpfs_xattr: bool,
}

impl Default for CapabilityFlags {
    fn default() -> Self {
        Self {
            vfs_driver: false,
            vfs_version: None,
            vfs_status_ioctl: false,
            susfs_available: false,
            susfs_version: None,
            susfs_kstat: false,
            susfs_path: false,
            susfs_maps: false,
            susfs_open_redirect: false,
            susfs_kstat_redirect: false,
            susfs_open_redirect_all: false,
            overlay_supported: false,
            erofs_supported: false,
            tmpfs_xattr: false,
        }
    }
}

// -- Mount Planning --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPlan {
    pub scenario: Scenario,
    pub modules: Vec<PlannedModule>,
    pub partition_mounts: Vec<PartitionMount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedModule {
    pub id: String,
    pub source_path: PathBuf,
    pub target_partitions: Vec<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionMount {
    pub partition: String,
    pub mount_point: PathBuf,
    pub contributing_modules: Vec<String>,
}

// -- Mount Results --

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MountStrategy {
    Vfs,
    Overlay,
    MagicMount,
}

impl Default for MountStrategy {
    fn default() -> Self {
        Self::MagicMount
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountResult {
    pub module_id: String,
    pub strategy_used: MountStrategy,
    pub success: bool,
    pub rules_applied: u32,
    pub rules_failed: u32,
    pub error: Option<String>,
    pub mount_paths: Vec<String>,
}

// -- Module Scanning --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScannedModule {
    pub id: String,
    pub path: PathBuf,
    pub files: Vec<ModuleFile>,
    pub has_service_sh: bool,
    pub has_post_fs_data_sh: bool,
    pub prop: ModuleProp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModuleFileType {
    Regular,
    Directory,
    Symlink,
    WhiteoutCharDev,
    WhiteoutXattr,
    WhiteoutAufs,
    OpaqueDir,
    RedirectXattr,
}

impl Default for ModuleFileType {
    fn default() -> Self {
        Self::Regular
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleFile {
    pub relative_path: PathBuf,
    pub file_type: ModuleFileType,
    pub source_module: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModuleProp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_code: u32,
    pub author: String,
    pub description: String,
}

// -- Runtime State --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub scenario: Scenario,
    pub capabilities: CapabilityFlags,
    pub engine_active: Option<bool>,
    pub driver_version: Option<u32>,
    pub rule_count: u32,
    pub excluded_uid_count: u32,
    pub hidden_path_count: u32,
    pub susfs_version: Option<String>,
    pub modules: Vec<ModuleStatus>,
    pub timestamp: u64,
    pub degraded: bool,
    pub degradation_reason: Option<String>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            scenario: Scenario::default(),
            capabilities: CapabilityFlags::default(),
            engine_active: None,
            driver_version: None,
            rule_count: 0,
            excluded_uid_count: 0,
            hidden_path_count: 0,
            susfs_version: None,
            modules: Vec::new(),
            timestamp: 0,
            degraded: false,
            degradation_reason: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleStatus {
    pub id: String,
    pub strategy: MountStrategy,
    pub rules_applied: u32,
    pub rules_failed: u32,
    pub errors: Vec<String>,
    pub mount_paths: Vec<String>,
}

// -- SUSFS Commands --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[allow(dead_code)] // FFI enum matching upstream kernel command codes
pub enum SusfsCommand {
    AddSusPath = 0x55550,
    SetAndroidDataRootPath = 0x55551,
    SetSdcardRootPath = 0x55552,
    AddSusPathLoop = 0x55553,
    AddSusKstat = 0x55570,
    UpdateSusKstat = 0x55571,
    AddSusKstatStatically = 0x55572,
    AddSusKstatRedirect = 0x55573,
    SetUname = 0x55590,
    EnableLog = 0x555a0,
    SetCmdline = 0x555b0,
    AddOpenRedirect = 0x555c0,
    AddOpenRedirectAll = 0x555c1,
    ShowVersion = 0x555e1,
    ShowEnabledFeatures = 0x555e2,
    ShowVariant = 0x555e3,
    EnableAvcLogSpoofing = 0x60010,
    AddSusMap = 0x60020,
}

// -- Detection --

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub scenario: Scenario,
    pub capabilities: CapabilityFlags,
    pub driver_version: Option<u32>,
    pub timestamp: u64,
}

// -- Root Manager Trait --

pub trait RootManager {
    fn name(&self) -> &str;
    fn base_dir(&self) -> &Path;
    fn busybox_path(&self) -> PathBuf;
    fn susfs_binary_paths(&self) -> Vec<PathBuf>;
    fn update_description(&self, text: &str) -> Result<()>;
    fn notify_module_mounted(&self) -> Result<()>;
}
