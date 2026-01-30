export interface VfsRule {
  id: string;
  name: string;
  source: string;
  target: string;
  hits: number;
  createdAt: Date;
  active: boolean;
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
  type: 'rule_added' | 'rule_removed' | 'uid_excluded' | 'uid_included' | 'engine_enabled' | 'engine_disabled';
  message: string;
  timestamp: Date;
}

export interface EngineStats {
  activeRules: number;
  excludedUids: number;
  hitsToday: number;
}

export interface MountedModule {
  name: string;
  ruleCount: number;
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

export interface Settings {
  theme: 'dark' | 'light' | 'auto' | 'amoled';
  accentColor: string;
  autoAccentColor: boolean;
  animationsEnabled: boolean;
  autoStartOnBoot: boolean;
  verboseLogging: boolean;
  fixedNav: boolean;
}

export type Tab = 'status' | 'modules' | 'config' | 'settings';
