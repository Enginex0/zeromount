import type {
  Scenario, VfsRule, ExcludedUid, ActivityItem, EngineStats, SystemInfo,
  KsuModule, CapabilityFlags, ModuleStatus, MountStrategy,
  BreneSettings, SusfsSettings, UnameSettings, MountSettings, AdbSettings,
  BridgeValues,
} from './types';

const CACHE_KEY = 'zm-state-cache';
const CACHE_VERSION = 4;

export interface HydratableState {
  scenario: Scenario;
  engineActive: boolean;
  capabilities: CapabilityFlags | null;
  stats: EngineStats;
  systemInfo: SystemInfo;
  moduleStatuses: ModuleStatus[];
  fontModules: string[];
  degraded: boolean;
  degradationReason: string | null;
  rootManager: string | null;
  zygiskHookActive: boolean | null;
  runtimeStrategy: MountStrategy | null;
  mountSource: string | null;
  resolvedStorageMode: string | null;
  rules: VfsRule[];
  excludedUids: ExcludedUid[];
  activity: ActivityItem[];
  ksuModules: KsuModule[];
  brene: BreneSettings;
  susfs: SusfsSettings;
  uname: UnameSettings;
  mount: MountSettings;
  adb: AdbSettings;
  verboseLogging: boolean;
  externalSusfsModule: 'susfs4ksu' | 'brene' | null;
  bridgeValues: BridgeValues | null;
}

interface SerializedState extends Omit<HydratableState, 'rules' | 'excludedUids' | 'activity'> {
  _v: number;
  _ts: number;
  rules: Array<Omit<VfsRule, 'createdAt'> & { createdAt: string }>;
  excludedUids: Array<Omit<ExcludedUid, 'excludedAt'> & { excludedAt: string }>;
  activity: Array<Omit<ActivityItem, 'timestamp'> & { timestamp: string }>;
}

export function writeCache(state: HydratableState): void {
  try {
    const serialized: SerializedState = {
      _v: CACHE_VERSION,
      _ts: Date.now(),
      scenario: state.scenario,
      engineActive: state.engineActive,
      capabilities: state.capabilities,
      stats: state.stats,
      systemInfo: state.systemInfo,
      moduleStatuses: state.moduleStatuses,
      fontModules: state.fontModules,
      degraded: state.degraded,
      degradationReason: state.degradationReason,
      rootManager: state.rootManager,
      zygiskHookActive: state.zygiskHookActive,
      runtimeStrategy: state.runtimeStrategy,
      mountSource: state.mountSource,
      resolvedStorageMode: state.resolvedStorageMode,
      rules: state.rules.map(r => ({ ...r, createdAt: r.createdAt.toISOString() })),
      excludedUids: state.excludedUids.map(u => ({ ...u, excludedAt: u.excludedAt.toISOString() })),
      activity: state.activity.map(a => ({ ...a, timestamp: a.timestamp.toISOString() })),
      ksuModules: state.ksuModules,
      brene: state.brene,
      susfs: state.susfs,
      uname: state.uname,
      mount: state.mount,
      adb: state.adb,
      verboseLogging: state.verboseLogging,
      externalSusfsModule: state.externalSusfsModule,
      bridgeValues: state.bridgeValues,
    };
    localStorage.setItem(CACHE_KEY, JSON.stringify(serialized));
  } catch (e) {
  }
}

export function readCache(): HydratableState | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY);
    if (!raw) return null;

    const cached: SerializedState = JSON.parse(raw);
    if (cached._v !== CACHE_VERSION) {
      localStorage.removeItem(CACHE_KEY);
      return null;
    }

    return {
      scenario: cached.scenario,
      engineActive: cached.engineActive,
      capabilities: cached.capabilities,
      stats: cached.stats,
      systemInfo: cached.systemInfo,
      moduleStatuses: cached.moduleStatuses,
      fontModules: cached.fontModules,
      degraded: cached.degraded,
      degradationReason: cached.degradationReason,
      rootManager: cached.rootManager,
      zygiskHookActive: (cached as any).zygiskHookActive ?? null,
      runtimeStrategy: cached.runtimeStrategy,
      mountSource: cached.mountSource,
      resolvedStorageMode: cached.resolvedStorageMode,
      rules: cached.rules.map(r => ({ ...r, createdAt: new Date(r.createdAt) })),
      excludedUids: cached.excludedUids.map(u => ({ ...u, excludedAt: new Date(u.excludedAt) })),
      activity: cached.activity.map(a => ({ ...a, timestamp: new Date(a.timestamp) })),
      ksuModules: cached.ksuModules,
      brene: cached.brene,
      susfs: cached.susfs,
      uname: cached.uname,
      mount: cached.mount,
      adb: cached.adb,
      verboseLogging: cached.verboseLogging,
      externalSusfsModule: cached.externalSusfsModule ?? null,
      bridgeValues: cached.bridgeValues ?? null,
    };
  } catch (e) {
    localStorage.removeItem(CACHE_KEY);
    return null;
  }
}

export function clearCache(): void {
  localStorage.removeItem(CACHE_KEY);
}
