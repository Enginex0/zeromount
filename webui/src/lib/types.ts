export interface VfsRule {
  id: string;
  name: string;
  source: string;
  target: string;
  createdAt: Date;
}

// Apps excluded from VFS redirection - they see stock files instead of module files
export interface ExcludedUid {
  uid: number;
  packageName: string;
  appName: string;
  excludedAt: Date;
}

export interface ActivityItem {
  id: string;
  type: 'rule_added' | 'rule_removed' | 'uid_excluded' | 'uid_included' | 'engine_enabled' | 'engine_disabled' | 'setting_changed' | 'mount_strategy_changed' | 'susfs_toggle' | 'brene_toggle' | 'theme_changed';
  message: string;
  timestamp: Date;
}

export interface EngineStats {
  activeRules: number;
  excludedUids: number;
  hiddenPaths: number;
}

export interface CapabilityFlags {
  vfs_driver: boolean;
  vfs_version: number | null;
  vfs_status_ioctl: boolean;
  susfs_available: boolean;
  susfs_version: string | null;
  susfs_kstat: boolean;
  susfs_path: boolean;
  susfs_maps: boolean;
  susfs_open_redirect: boolean;
  susfs_kstat_redirect: boolean;
  susfs_open_redirect_all: boolean;
  overlay_supported: boolean;
  erofs_supported: boolean;
  tmpfs_xattr: boolean;
}

export type MountStrategy = 'Vfs' | 'Overlay' | 'MagicMount' | 'Font';

export interface ModuleStatus {
  id: string;
  strategy: MountStrategy;
  rules_applied: number;
  rules_failed: number;
  errors: string[];
  mount_paths: string[];
}

export interface RuntimeStatus {
  scenario: Scenario;
  capabilities: CapabilityFlags;
  engine_active: boolean | null;
  driver_version: number | null;
  rule_count: number;
  excluded_uid_count: number;
  hidden_path_count: number;
  susfs_version: string | null;
  active_strategy: MountStrategy | null;
  modules: ModuleStatus[];
  font_modules: string[];
  timestamp: number;
  degraded: boolean;
  degradation_reason: string | null;
  root_manager: string | null;
}

export interface KsuModule {
  name: string;
  path: string;
  hasSystem: boolean;
  hasVendor: boolean;
  hasProduct: boolean;
  isLoaded: boolean;
  fileCount: number;
}

export interface InstalledApp {
  uid: number;
  packageName: string;
  appName: string;
  isSystemApp: boolean;
  iconPath?: string;
}

export interface SystemInfo {
  driverVersion: string;
  kernelVersion: string;
  susfsVersion: string;
  uptime: string;
  deviceModel: string;
  androidVersion: string;
  selinuxStatus: string;
}

export interface BreneSettings {
  auto_hide_apk: boolean;
  auto_hide_zygisk: boolean;
  auto_hide_fonts: boolean;
  auto_hide_rooted_folders: boolean;
  auto_hide_recovery: boolean;
  auto_hide_tmp: boolean;
  auto_hide_sdcard_data: boolean;
  avc_log_spoofing: boolean;
  susfs_log: boolean;
  hide_sus_mounts: boolean;
  emulate_vold_app_data: boolean;
  force_hide_lsposed: boolean;
  spoof_cmdline: boolean;
  hide_ksu_loops: boolean;
  prop_spoofing: boolean;
}

export interface SusfsSettings {
  enabled: boolean;
  path_hide: boolean;
  kstat: boolean;
  maps_hide: boolean;
  open_redirect: boolean;
}

export type UnameMode = 'disabled' | 'static' | 'dynamic';

export interface UnameSettings {
  mode: UnameMode;
  release: string;
  version: string;
}

export type StorageMode = 'auto' | 'erofs' | 'tmpfs' | 'ext4';

export interface MountSettings {
  storage_mode: StorageMode;
  overlay_preferred: boolean;
  magic_mount_fallback: boolean;
  random_mount_paths: boolean;
  mount_source: string;
  overlay_source: string;
}

export interface Settings {
  theme: 'dark' | 'light' | 'auto' | 'amoled';
  accentColor: string;
  autoAccentColor: boolean;
  verboseLogging: boolean;
  fixedNav: boolean;
  brene: BreneSettings;
  susfs: SusfsSettings;
  uname: UnameSettings;
  mount: MountSettings;
}

export type Scenario = 'full' | 'susfs_frontend' | 'kernel_only' | 'susfs_only' | 'none';

export type Tab = 'status' | 'modules' | 'config' | 'settings';
