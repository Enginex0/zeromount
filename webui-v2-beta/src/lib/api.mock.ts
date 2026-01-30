import type {
  VfsRule,
  ExcludedUid,
  ActivityItem,
  MountedModule,
  KsuModule,
  InstalledApp,
  SystemInfo,
  EngineStats,
} from './types';
import { APP_VERSION } from './constants';

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

const mockRules: VfsRule[] = [
  {
    id: '1',
    name: 'Hide Magisk Data',
    source: '/data/adb/magisk',
    target: '/dev/null',
    hits: 156,
    createdAt: new Date(Date.now() - 2 * 60 * 60 * 1000),
    active: true,
  },
  {
    id: '2',
    name: 'Block Detection Paths',
    source: '/system/bin/su',
    target: '[BLOCKED]',
    hits: 2341,
    createdAt: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000),
    active: true,
  },
  {
    id: '3',
    name: 'Redirect Config',
    source: '/data/local/tmp/.config',
    target: '/data/adb/.hidden',
    hits: 47,
    createdAt: new Date(Date.now() - 12 * 60 * 60 * 1000),
    active: true,
  },
];

const mockExcludedUids: ExcludedUid[] = [
  {
    uid: 10234,
    packageName: 'com.example.detector',
    appName: 'Native Detector',
    excludedAt: new Date(Date.now() - 2 * 24 * 60 * 60 * 1000),
  },
  {
    uid: 10456,
    packageName: 'com.securityapp.holmes',
    appName: 'Holmes Security',
    excludedAt: new Date(Date.now() - 5 * 60 * 60 * 1000),
  },
  {
    uid: 10789,
    packageName: 'com.momo.detector',
    appName: 'Momo Detector',
    excludedAt: new Date(Date.now() - 1 * 24 * 60 * 60 * 1000),
  },
];

const mockActivity: ActivityItem[] = [
  {
    id: '1',
    type: 'rule_added',
    message: 'Rule added: /data/local -> /null',
    timestamp: new Date(Date.now() - 2 * 60 * 1000),
  },
  {
    id: '2',
    type: 'uid_excluded',
    message: 'UID 10234 excluded (com.detect.app)',
    timestamp: new Date(Date.now() - 15 * 60 * 1000),
  },
  {
    id: '3',
    type: 'engine_enabled',
    message: 'Engine enabled',
    timestamp: new Date(Date.now() - 60 * 60 * 1000),
  },
];

const mockModules: MountedModule[] = [
  { name: 'PlayIntegrityFix', ruleCount: 12 },
  { name: 'LSPosed', ruleCount: 8 },
  { name: 'FontModule', ruleCount: 45 },
];

const mockKsuModules: KsuModule[] = [
  {
    name: 'PlayIntegrityFix',
    path: '/data/adb/modules/playintegrityfix',
    hasSystem: true,
    hasVendor: false,
    hasProduct: false,
    isLoaded: true,
    fileCount: 12,
  },
  {
    name: 'LSPosed',
    path: '/data/adb/modules/zygisk_lsposed',
    hasSystem: true,
    hasVendor: false,
    hasProduct: false,
    isLoaded: false,
    fileCount: 8,
  },
  {
    name: 'Font Manager',
    path: '/data/adb/modules/fontmanager',
    hasSystem: true,
    hasVendor: true,
    hasProduct: false,
    isLoaded: true,
    fileCount: 45,
  },
  {
    name: 'Busybox NDK',
    path: '/data/adb/modules/busybox-ndk',
    hasSystem: true,
    hasVendor: false,
    hasProduct: false,
    isLoaded: false,
    fileCount: 156,
  },
  {
    name: 'Shamiko',
    path: '/data/adb/modules/shamiko',
    hasSystem: false,
    hasVendor: false,
    hasProduct: false,
    isLoaded: false,
    fileCount: 0,
  },
];

const mockInstalledApps: InstalledApp[] = [
  { uid: 10001, packageName: 'com.android.chrome', appName: 'Chrome', isSystemApp: false, iconPath: 'link/icons/com.android.chrome.png' },
  { uid: 10002, packageName: 'com.whatsapp', appName: 'WhatsApp', isSystemApp: false, iconPath: 'link/icons/com.whatsapp.png' },
  { uid: 10003, packageName: 'com.instagram.android', appName: 'Instagram', isSystemApp: false, iconPath: 'link/icons/com.instagram.android.png' },
  { uid: 10004, packageName: 'com.spotify.music', appName: 'Spotify', isSystemApp: false, iconPath: 'link/icons/com.spotify.music.png' },
  { uid: 10005, packageName: 'com.twitter.android', appName: 'X (Twitter)', isSystemApp: false, iconPath: 'link/icons/com.twitter.android.png' },
  { uid: 10006, packageName: 'com.netflix.mediaclient', appName: 'Netflix', isSystemApp: false, iconPath: 'link/icons/com.netflix.mediaclient.png' },
  { uid: 10007, packageName: 'com.facebook.katana', appName: 'Facebook', isSystemApp: false, iconPath: 'link/icons/com.facebook.katana.png' },
  { uid: 10008, packageName: 'com.discord', appName: 'Discord', isSystemApp: false, iconPath: 'link/icons/com.discord.png' },
  { uid: 10009, packageName: 'com.telegram.messenger', appName: 'Telegram', isSystemApp: false, iconPath: 'link/icons/com.telegram.messenger.png' },
  { uid: 10010, packageName: 'com.google.android.youtube', appName: 'YouTube', isSystemApp: false, iconPath: 'link/icons/com.google.android.youtube.png' },
  { uid: 10011, packageName: 'io.github.vvb2060.mahoshojo', appName: 'Momo Detector', isSystemApp: false, iconPath: 'link/icons/io.github.vvb2060.mahoshojo.png' },
  { uid: 10012, packageName: 'rikka.appops', appName: 'App Ops', isSystemApp: false, iconPath: 'link/icons/rikka.appops.png' },
  { uid: 10013, packageName: 'com.termux', appName: 'Termux', isSystemApp: false, iconPath: 'link/icons/com.termux.png' },
  { uid: 10014, packageName: 'com.topjohnwu.magisk', appName: 'Magisk', isSystemApp: false, iconPath: 'link/icons/com.topjohnwu.magisk.png' },
  { uid: 10015, packageName: 'me.weishu.kernelsu', appName: 'KernelSU', isSystemApp: false, iconPath: 'link/icons/me.weishu.kernelsu.png' },
  { uid: 1000, packageName: 'com.android.settings', appName: 'Settings', isSystemApp: true, iconPath: 'link/icons/com.android.settings.png' },
  { uid: 1001, packageName: 'com.android.phone', appName: 'Phone', isSystemApp: true, iconPath: 'link/icons/com.android.phone.png' },
  { uid: 10100, packageName: 'com.android.systemui', appName: 'System UI', isSystemApp: true, iconPath: 'link/icons/com.android.systemui.png' },
  { uid: 10101, packageName: 'com.google.android.gms', appName: 'Google Play Services', isSystemApp: true, iconPath: 'link/icons/com.google.android.gms.png' },
  { uid: 10102, packageName: 'com.google.android.gsf', appName: 'Google Services Framework', isSystemApp: true, iconPath: 'link/icons/com.google.android.gsf.png' },
  { uid: 10103, packageName: 'com.android.vending', appName: 'Play Store', isSystemApp: true, iconPath: 'link/icons/com.android.vending.png' },
  { uid: 10104, packageName: 'com.google.android.apps.messaging', appName: 'Messages', isSystemApp: true, iconPath: 'link/icons/com.google.android.apps.messaging.png' },
  { uid: 10105, packageName: 'com.android.providers.contacts', appName: 'Contacts Storage', isSystemApp: true, iconPath: 'link/icons/com.android.providers.contacts.png' },
];

export const MockAPI = {
  async getVersion(): Promise<string> {
    await delay(100);
    return `v${APP_VERSION}`;
  },

  async getSystemInfo(): Promise<SystemInfo> {
    await delay(150);
    return {
      driverVersion: `v${APP_VERSION}`,
      kernelVersion: '6.1.75',
      susfsVersion: 'v1.5.2',
      uptime: '4h 23m',
      deviceModel: 'Pixel 6',
      androidVersion: '14',
      selinuxStatus: 'Enforcing',
    };
  },

  async getRules(): Promise<VfsRule[]> {
    await delay(200);
    return [...mockRules];
  },

  async addRule(name: string, source: string, target: string): Promise<VfsRule> {
    await delay(300);
    const newRule: VfsRule = {
      id: Date.now().toString(),
      name,
      source,
      target,
      hits: 0,
      createdAt: new Date(),
      active: true,
    };
    mockRules.push(newRule);
    return newRule;
  },

  async deleteRule(sourcePath: string): Promise<void> {
    await delay(200);
    const index = mockRules.findIndex(r => r.id === sourcePath || r.source === sourcePath);
    if (index > -1) mockRules.splice(index, 1);
  },

  async clearAllRules(): Promise<void> {
    await delay(300);
    mockRules.length = 0;
  },

  async getExcludedUids(): Promise<ExcludedUid[]> {
    await delay(200);
    return [...mockExcludedUids];
  },

  async excludeUid(uid: number, packageName: string, appName: string): Promise<ExcludedUid> {
    await delay(300);
    const excluded: ExcludedUid = {
      uid,
      packageName,
      appName,
      excludedAt: new Date(),
    };
    mockExcludedUids.push(excluded);
    return excluded;
  },

  async includeUid(uid: number): Promise<void> {
    await delay(200);
    const index = mockExcludedUids.findIndex(u => u.uid === uid);
    if (index > -1) mockExcludedUids.splice(index, 1);
  },

  async getActivity(): Promise<ActivityItem[]> {
    await delay(150);
    return [...mockActivity];
  },

  async getStats(): Promise<EngineStats> {
    await delay(100);
    return {
      activeRules: mockRules.length,
      excludedUids: mockExcludedUids.length,
      hitsToday: mockRules.reduce((sum, r) => sum + r.hits, 0),
    };
  },

  async toggleEngine(_enable: boolean): Promise<void> {
    await delay(400);
  },

  async setVerboseLogging(_enabled: boolean): Promise<void> {
    await delay(100);
  },

  async getModules(): Promise<MountedModule[]> {
    await delay(100);
    return [...mockModules];
  },

  async isEngineActive(): Promise<boolean> {
    return true;
  },

  async getInstalledApps(): Promise<InstalledApp[]> {
    await delay(300);
    return [...mockInstalledApps];
  },

  async scanKsuModules(): Promise<KsuModule[]> {
    await delay(200);
    return [...mockKsuModules];
  },

  async loadKsuModule(_moduleName: string, modulePath: string): Promise<number> {
    await delay(500);
    const mod = mockKsuModules.find(m => m.path === modulePath);
    if (mod) mod.isLoaded = true;
    return mod?.fileCount || 0;
  },

  async unloadKsuModule(_moduleName: string, modulePath: string): Promise<number> {
    await delay(400);
    const mod = mockKsuModules.find(m => m.path === modulePath);
    if (mod) mod.isLoaded = false;
    return mod?.fileCount || 0;
  },

  async fetchSystemColor(): Promise<string | null> {
    return null;
  },
};
