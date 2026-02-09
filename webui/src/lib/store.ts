import { createSignal, createRoot, createMemo, createEffect } from 'solid-js';
import { createStore } from 'solid-js/store';
import type { Tab, Scenario, VfsRule, ExcludedUid, ActivityItem, EngineStats, SystemInfo, Settings, InstalledApp, KsuModule, CapabilityFlags, ModuleStatus, BreneSettings, SusfsSettings, UnameSettings, UnameMode, MountSettings, StorageMode, MountStrategy } from './types';
import { api, shouldUseMock } from './api';
import { listPackages, getPackagesInfo, getAppLabelViaAapt } from './ksuApi';
import { darkTheme, lightTheme, amoledTheme, applyTheme, applyAccent, getAccentStyles, accentPresets } from './theme';

function createAppStore() {
  console.log('[ZM-Store] createAppStore() initializing...');
  const [activeTab, setActiveTab] = createSignal<Tab>('status');

  const [engineActive, setEngineActive] = createSignal(true);

  // Granular loading states for precise UI feedback
  const [loading, setLoading] = createStore({
    status: false,     // Engine status, stats, systemInfo
    modules: false,    // KSU modules scan
    apps: false,       // Installed apps list
    rules: false,      // VFS rules CRUD
    activity: false,   // Activity log
    engine: false,     // Engine toggle
  });

  const [stats, setStats] = createStore<EngineStats>({
    activeRules: 0,
    excludedUids: 0,
    hiddenPaths: 0,
  });

  const [systemInfo, setSystemInfo] = createStore<SystemInfo>({
    driverVersion: '',
    kernelVersion: '',
    susfsVersion: '',
    uptime: '',
    deviceModel: '',
    androidVersion: '',
    selinuxStatus: '',
  });

  const [rules, setRules] = createSignal<VfsRule[]>([]);
  const [excludedUids, setExcludedUids] = createSignal<ExcludedUid[]>([]);
  const [activity, setActivity] = createSignal<ActivityItem[]>([]);
  const [installedApps, setInstalledApps] = createSignal<InstalledApp[]>([]);
  const [ksuModules, setKsuModules] = createSignal<KsuModule[]>([]);
  const [scenario, setScenario] = createSignal<Scenario>('none');
  const [capabilities, setCapabilities] = createSignal<CapabilityFlags | null>(null);
  const [moduleStatuses, setModuleStatuses] = createSignal<ModuleStatus[]>([]);
  const [degraded, setDegraded] = createSignal(false);
  const [degradationReason, setDegradationReason] = createSignal<string | null>(null);

  const savedTheme = typeof window !== 'undefined'
    ? (localStorage.getItem('zeromount-theme') as 'dark' | 'light' | 'auto' | 'amoled' | null)
    : null;

  const savedFixedNav = typeof window !== 'undefined'
    ? localStorage.getItem('zeromount-fixedNav') === 'true'
    : true;

  const savedAutoAccentRaw = typeof window !== 'undefined'
    ? localStorage.getItem('zeromount-autoAccent')
    : null;
  const savedAutoAccent = savedAutoAccentRaw === null ? true : savedAutoAccentRaw === 'true';

  const savedAccent = typeof window !== 'undefined'
    ? localStorage.getItem('zeromount-accent')
    : null;

  const accentColors = Object.keys(accentPresets);
  const randomAccent = accentColors[Math.floor(Math.random() * accentColors.length)];
  const initialAccent = savedAutoAccent ? randomAccent : (savedAccent && accentPresets[savedAccent] ? savedAccent : '#FF8E53');

  const defaultBrene: BreneSettings = {
    auto_hide_apk: true,
    auto_hide_zygisk: true,
    auto_hide_fonts: true,
    auto_hide_rooted_folders: true,
    auto_hide_recovery: true,
    auto_hide_tmp: true,
    auto_hide_sdcard_data: true,
    avc_log_spoofing: true,
    susfs_log: false,
    hide_sus_mounts: true,
    emulate_vold_app_data: true,
    force_hide_lsposed: true,
    prop_spoofing: true,
  };

  const defaultSusfs: SusfsSettings = {
    path_hide: true,
    kstat: true,
    maps_hide: true,
    open_redirect: true,
  };

  const defaultUname: UnameSettings = {
    mode: 'disabled',
    release: '',
    version: '',
  };

  const defaultMount: MountSettings = {
    storage_mode: 'auto',
    overlay_preferred: true,
    magic_mount_fallback: true,
    random_mount_paths: true,
  };

  const [settings, setSettings] = createStore<Settings>({
    theme: (savedTheme || 'amoled') as 'dark' | 'light' | 'auto' | 'amoled',
    accentColor: initialAccent,
    autoAccentColor: savedAutoAccent,
    verboseLogging: false,
    fixedNav: savedFixedNav,
    brene: { ...defaultBrene },
    susfs: { ...defaultSusfs },
    uname: { ...defaultUname },
    mount: { ...defaultMount },
  });

  const [systemPrefersDark, setSystemPrefersDark] = createSignal(
    typeof window !== 'undefined'
      ? window.matchMedia('(prefers-color-scheme: dark)').matches
      : true
  );

  if (typeof window !== 'undefined') {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    mediaQuery.addEventListener('change', (e) => setSystemPrefersDark(e.matches));
  }

  const currentTheme = createMemo(() => {
    const pref = settings.theme;
    const baseTheme = pref === 'light' ? lightTheme
      : pref === 'amoled' ? amoledTheme
      : pref === 'auto' ? (systemPrefersDark() ? darkTheme : lightTheme)
      : darkTheme;

    const accentStyles = getAccentStyles(settings.accentColor);
    return {
      ...baseTheme,
      gradientPrimary: accentStyles.gradient,
      textAccent: accentStyles.textAccent,
      textOnAccent: accentStyles.textOnAccent,
      accentRgb: accentStyles.rgb,
      shadowGlow: `0 0 20px rgba(${accentStyles.rgb}, 0.3)`,
    };
  });

  // Apply theme and accent color together
  createEffect(() => {
    applyTheme(currentTheme(), settings.accentColor);
  });

  // Watch for accent color changes and apply them
  createEffect(() => {
    applyAccent(settings.accentColor);
  });

  // Save theme preference to localStorage when it changes
  createEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('zeromount-theme', settings.theme);
    }
  });

  // Save accent color to localStorage when it changes
  createEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('zeromount-accent', settings.accentColor);
    }
  });

  createEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('zeromount-fixedNav', String(settings.fixedNav));
    }
  });

  createEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('zeromount-autoAccent', String(settings.autoAccentColor));
    }
  });

  // Randomize accent when page becomes visible (for cached WebViews)
  if (typeof window !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible' && settings.autoAccentColor) {
        const colors = Object.keys(accentPresets);
        const newRandom = colors[Math.floor(Math.random() * colors.length)];
        setSettings({ accentColor: newRandom });
      }
    });
  }

  // Toast notifications
  const [toast, setToast] = createSignal<{ message: string; type: 'success' | 'error' | 'info' } | null>(null);

  // Actions
  const showToast = (message: string, type: 'success' | 'error' | 'info' = 'info') => {
    setToast({ message, type });
    setTimeout(() => setToast(null), 3000);
  };

  const loadInitialData = async () => {
    console.log('[ZM-Store] loadInitialData() starting...');
    setLoading({ status: true, rules: true, activity: true });

    try {
      // Primary: single zeromount status --json call for bulk data
      const results = await Promise.allSettled([
        api.getRuntimeStatus(),
        api.getRules(),
        api.getExcludedUids(),
        api.getActivity(),
        api.getSystemInfo(),
        api.scanKsuModules(),
        loadBreneSettings(),
        settings.autoAccentColor ? api.fetchSystemColor() : Promise.resolve(null),
        loadMountSettings(),
      ]);

      const status = results[0].status === 'fulfilled' ? results[0].value : null;
      const rulesData = results[1].status === 'fulfilled' ? results[1].value : [];
      const uidsData = results[2].status === 'fulfilled' ? results[2].value : [];
      const activityData = results[3].status === 'fulfilled' ? results[3].value : [];
      const sysInfo = results[4].status === 'fulfilled' ? results[4].value : { driverVersion: '', kernelVersion: '', susfsVersion: '', uptime: '', deviceModel: '', androidVersion: '', selinuxStatus: '' };
      const ksuModulesData = results[5].status === 'fulfilled' ? results[5].value : [];
      const systemColor = results[7].status === 'fulfilled' ? results[7].value : null;

      // Apply runtime status (authoritative source for scenario, capabilities, engine state)
      if (status) {
        setScenario(status.scenario as Scenario);
        setCapabilities(status.capabilities);
        setModuleStatuses(status.modules);
        setDegraded(status.degraded);
        setDegradationReason(status.degradation_reason);
        if (status.engine_active !== null) setEngineActive(status.engine_active);
        if (status.driver_version !== null) setSystemInfo('driverVersion', `v${status.driver_version}`);
        if (status.susfs_version) setSystemInfo('susfsVersion', status.susfs_version);
        setStats({
          activeRules: status.rule_count,
          excludedUids: status.excluded_uid_count,
          hiddenPaths: status.hidden_path_count,
        });
      }

      // Apply supplementary data (rules list, UIDs, device info, modules)
      setRules(rulesData);
      setExcludedUids(uidsData);
      setActivity(activityData);
      setSystemInfo('kernelVersion', sysInfo.kernelVersion);
      setSystemInfo('uptime', sysInfo.uptime);
      setSystemInfo('deviceModel', sysInfo.deviceModel);
      setSystemInfo('androidVersion', sysInfo.androidVersion);
      setSystemInfo('selinuxStatus', sysInfo.selinuxStatus);
      // Backfill from sysInfo if status JSON was unavailable
      if (!status) {
        setSystemInfo('driverVersion', sysInfo.driverVersion);
        setSystemInfo('susfsVersion', sysInfo.susfsVersion);
        setStats({
          activeRules: rulesData.length,
          excludedUids: uidsData.length,
        });
      }
      setKsuModules(ksuModulesData);
      if (systemColor) setSettings({ accentColor: systemColor });
      console.log('[ZM-Store] loadInitialData() complete');
    } catch (err) {
      console.error('[ZM-Store] loadInitialData() error:', err);
      showToast('Failed to load data', 'error');
    } finally {
      setLoading({ status: false, rules: false, activity: false });
    }
  };

  const toggleEngine = async () => {
    if (loading.engine) return;

    const newState = !engineActive();
    console.log('[ZM-Store] toggleEngine() called, newState:', newState);
    setLoading('engine', true);
    try {
      await api.toggleEngine(newState);
      setEngineActive(newState);
      console.log('[ZM-Store] toggleEngine() success, engine now:', newState);
      showToast(newState ? 'Engine activated' : 'Engine deactivated', 'success');
    } catch (err) {
      console.error('[ZM-Store] toggleEngine() error:', err);
      showToast('Failed to toggle engine', 'error');
    } finally {
      setLoading('engine', false);
    }
  };

  const excludeUid = async (uid: number, packageName: string, appName: string) => {
    console.log('[ZM-Store] excludeUid() called:', { uid, packageName, appName });
    if (uid <= 0) {
      console.warn('[ZM-Store] Cannot exclude invalid UID:', uid);
      showToast('Cannot exclude app with unknown UID', 'error');
      return null;
    }
    if (pendingUidOperations.has(uid)) {
      console.log('[ZM-Store] Operation pending for UID:', uid);
      return null;
    }
    pendingUidOperations.add(uid);
    setLoading('apps', true);
    try {
      const excluded = await api.excludeUid(uid, packageName, appName);
      setExcludedUids(prev => [...prev, excluded]);
      setStats('excludedUids', s => s + 1);
      console.log('[ZM-Store] excludeUid() success');
      showToast(`Excluded ${appName}`, 'success');
      return excluded;
    } catch (err) {
      console.error('[ZM-Store] excludeUid() error:', err);
      showToast('Failed to exclude UID', 'error');
      throw err;
    } finally {
      pendingUidOperations.delete(uid);
      setLoading('apps', false);
    }
  };

  const includeUid = async (uid: number) => {
    console.log('[ZM-Store] includeUid() called:', uid);
    if (uid <= 0) {
      console.warn('[ZM-Store] Cannot include invalid UID:', uid);
      showToast('Cannot include app with unknown UID', 'error');
      return;
    }
    if (pendingUidOperations.has(uid)) {
      console.log('[ZM-Store] Operation pending for UID:', uid);
      return;
    }
    pendingUidOperations.add(uid);
    setLoading('apps', true);
    try {
      await api.includeUid(uid);
      setExcludedUids(prev => prev.filter(u => u.uid !== uid));
      setStats('excludedUids', s => s - 1);
      console.log('[ZM-Store] includeUid() success');
      showToast('UID included', 'success');
    } catch (err) {
      console.error('[ZM-Store] includeUid() error:', err);
      showToast('Failed to include UID', 'error');
    } finally {
      pendingUidOperations.delete(uid);
      setLoading('apps', false);
    }
  };

  const clearAllRules = async () => {
    console.log('[ZM-Store] clearAllRules() called');
    setLoading('rules', true);
    try {
      await api.clearAllRules();
      setRules([]);
      setStats('activeRules', 0);
      console.log('[ZM-Store] clearAllRules() success');
      showToast('All rules cleared', 'success');
    } catch (err) {
      console.error('[ZM-Store] clearAllRules() error:', err);
      showToast('Failed to clear rules', 'error');
    } finally {
      setLoading('rules', false);
    }
  };

  const updateSettings = (updates: Partial<Settings>) => {
    console.log('[ZM-Store] updateSettings() called:', updates);
    setSettings(updates);
    if (updates.theme) api.logActivity('THEME_CHANGED', `Theme → ${updates.theme}`);
    if (updates.accentColor) api.logActivity('THEME_CHANGED', `Accent → ${updates.accentColor}`);
    if (updates.fixedNav !== undefined) api.logActivity('SETTING_CHANGED', `Fixed nav → ${updates.fixedNav ? 'ON' : 'OFF'}`);
  };

  const fetchSystemColor = async () => {
    try {
      const systemColor = await api.fetchSystemColor();
      if (systemColor) {
        setSettings({ accentColor: systemColor });
      }
    } catch (e) {
      console.error('[ZM-Store] fetchSystemColor() error:', e);
    }
  };

  const setVerboseLogging = async (enabled: boolean) => {
    setSettings({ verboseLogging: enabled });
    try {
      await api.setVerboseLogging(enabled);
      await api.logActivity('SETTING_CHANGED', `Verbose logging → ${enabled ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setVerboseLogging() error:', e);
      showToast('Failed to set verbose logging', 'error');
    }
  };

  const loadBreneSettings = async () => {
    const breneKeys: (keyof BreneSettings)[] = [
      'auto_hide_apk', 'auto_hide_zygisk', 'auto_hide_fonts',
      'auto_hide_rooted_folders', 'auto_hide_recovery', 'auto_hide_tmp',
      'auto_hide_sdcard_data', 'avc_log_spoofing', 'susfs_log',
      'hide_sus_mounts', 'emulate_vold_app_data', 'force_hide_lsposed',
    ];
    const results = await Promise.allSettled([
      ...breneKeys.map(k => api.configGet(`brene.${k}`)),
      api.configGet('uname.mode'),
      api.configGet('uname.release'),
      api.configGet('uname.version'),
    ]);
    const brene: Partial<BreneSettings> = {};
    breneKeys.forEach((key, i) => {
      const r = results[i];
      if (r.status === 'fulfilled' && r.value !== null) {
        brene[key] = r.value === 'true';
      }
    });
    setSettings('brene', prev => ({ ...prev, ...brene }));

    const unameMode = results[breneKeys.length];
    const unameRelease = results[breneKeys.length + 1];
    const unameVersion = results[breneKeys.length + 2];
    const uname: Partial<UnameSettings> = {};
    if (unameMode.status === 'fulfilled' && unameMode.value !== null) {
      uname.mode = unameMode.value as UnameMode;
    }
    if (unameRelease.status === 'fulfilled' && unameRelease.value !== null) {
      uname.release = unameRelease.value;
    }
    if (unameVersion.status === 'fulfilled' && unameVersion.value !== null) {
      uname.version = unameVersion.value;
    }
    setSettings('uname', prev => ({ ...prev, ...uname }));
  };

  const setBreneToggle = async (key: keyof BreneSettings, value: boolean) => {
    setSettings('brene', key, value);
    try {
      await api.configSet(`brene.${key}`, String(value));
      // Chain controlled settings to SUSFS kernel + config.sh
      if (key === 'avc_log_spoofing') {
        await api.setSusfsAvcSpoofing(value);
        await api.writeSusfsConfigVar('avc_log_spoofing', value ? '1' : '0');
      } else if (key === 'susfs_log') {
        await api.setSusfsLog(value);
        await api.writeSusfsConfigVar('susfs_log', value ? '1' : '0');
      } else if (key === 'hide_sus_mounts') {
        await api.setSusfsHideMounts(value);
        await api.writeSusfsConfigVar('hide_sus_mnts_for_all_or_non_su_procs', value ? '1' : '0');
      } else if (key === 'emulate_vold_app_data') {
        await api.writeSusfsConfigVar('emulate_vold_app_data', value ? '1' : '0');
      } else if (key === 'force_hide_lsposed') {
        await api.writeSusfsConfigVar('force_hide_lsposed', value ? '1' : '0');
      }
      await api.logActivity('BRENE_TOGGLE', `${key} → ${value ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setBreneToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('brene', key, !value);
    }
  };

  const setSusfsToggle = async (key: keyof SusfsSettings, value: boolean) => {
    setSettings('susfs', key, value);
    try {
      await api.configSet(`susfs.${key}`, String(value));
      await api.logActivity('SUSFS_TOGGLE', `${key} → ${value ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setSusfsToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('susfs', key, !value);
    }
  };

  const setUnameMode = async (mode: UnameMode) => {
    const prev = settings.uname.mode;
    setSettings('uname', 'mode', mode);
    try {
      await api.configSet('uname.mode', mode);
      await api.logActivity('SETTING_CHANGED', `Uname mode → ${mode}`);
    } catch (e) {
      console.error('[ZM-Store] setUnameMode() error:', e);
      showToast('Failed to save uname mode', 'error');
      setSettings('uname', 'mode', prev);
    }
  };

  const setUnameField = async (field: 'release' | 'version', value: string) => {
    const prev = settings.uname[field];
    setSettings('uname', field, value);
    try {
      await api.configSet(`uname.${field}`, value);
      await api.logActivity('SETTING_CHANGED', `Uname ${field} → ${value || '(empty)'}`);
    } catch (e) {
      console.error('[ZM-Store] setUnameField() error:', e);
      showToast(`Failed to save uname ${field}`, 'error');
      setSettings('uname', field, prev);
    }
  };

  const loadMountSettings = async () => {
    console.log('[ZM-Store] loadMountSettings() starting...');
    const boolKeys: (keyof MountSettings)[] = [
      'overlay_preferred', 'magic_mount_fallback', 'random_mount_paths',
    ];
    const results = await Promise.allSettled([
      api.configGet('mount.storage_mode'),
      ...boolKeys.map(k => api.configGet(`mount.${k}`)),
    ]);
    const mount: Partial<MountSettings> = {};
    if (results[0].status === 'fulfilled' && results[0].value !== null) {
      mount.storage_mode = results[0].value as StorageMode;
    }
    boolKeys.forEach((key, i) => {
      const r = results[i + 1];
      if (r.status === 'fulfilled' && r.value !== null) {
        (mount as any)[key] = r.value === 'true';
      }
    });
    setSettings('mount', prev => ({ ...prev, ...mount }));
    console.log('[ZM-Store] loadMountSettings() loaded:', mount);
  };

  const setMountStorageMode = async (mode: StorageMode) => {
    console.log('[ZM-Store] setMountStorageMode() called:', mode);
    const prev = settings.mount.storage_mode;
    setSettings('mount', 'storage_mode', mode);
    try {
      await api.configSet('mount.storage_mode', mode);
      await api.logActivity('SETTING_CHANGED', `Storage mode → ${mode}`);
      console.log('[ZM-Store] setMountStorageMode() saved:', mode);
    } catch (e) {
      console.error('[ZM-Store] setMountStorageMode() error:', e);
      showToast('Failed to save storage mode', 'error');
      setSettings('mount', 'storage_mode', prev);
    }
  };

  const setMountToggle = async (key: 'overlay_preferred' | 'magic_mount_fallback' | 'random_mount_paths', value: boolean) => {
    console.log('[ZM-Store] setMountToggle() called:', key, value);
    const prev = settings.mount[key];
    setSettings('mount', key, value);
    try {
      await api.configSet(`mount.${key}`, String(value));
      await api.logActivity('SETTING_CHANGED', `${key} → ${value ? 'ON' : 'OFF'}`);
      console.log('[ZM-Store] setMountToggle() saved:', key, value);
    } catch (e) {
      console.error('[ZM-Store] setMountToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('mount', key, prev);
    }
  };

  const setMountStrategy = async (strategy: MountStrategy) => {
    console.log('[ZM-Store] setMountStrategy() called:', strategy);
    const prevOverlay = settings.mount.overlay_preferred;
    const prevMagic = settings.mount.magic_mount_fallback;

    const mapping: Record<MountStrategy, [boolean, boolean]> = {
      'Vfs': [true, true],
      'Overlay': [true, false],
      'MagicMount': [false, true],
    };
    const [newOverlay, newMagic] = mapping[strategy];

    setSettings('mount', 'overlay_preferred', newOverlay);
    setSettings('mount', 'magic_mount_fallback', newMagic);

    try {
      await Promise.all([
        api.configSet('mount.overlay_preferred', String(newOverlay)),
        api.configSet('mount.magic_mount_fallback', String(newMagic)),
      ]);
      await api.logActivity('MOUNT_STRATEGY_CHANGED', `Strategy → ${strategy}`);
      showToast('Mount strategy changed — reboot to apply', 'info');
      console.log('[ZM-Store] setMountStrategy() saved:', strategy, '→ overlay_preferred:', newOverlay, 'magic_mount_fallback:', newMagic);
    } catch (e) {
      console.error('[ZM-Store] setMountStrategy() error:', e);
      showToast('Failed to save mount strategy', 'error');
      setSettings('mount', 'overlay_preferred', prevOverlay);
      setSettings('mount', 'magic_mount_fallback', prevMagic);
    }
  };

  const activeStrategy = (): MountStrategy => {
    if (settings.mount.overlay_preferred && settings.mount.magic_mount_fallback) return 'Vfs';
    if (settings.mount.overlay_preferred && !settings.mount.magic_mount_fallback) return 'Overlay';
    return 'MagicMount';
  };

  const loadRuntimeStatus = async () => {
    try {
      const status = await api.getRuntimeStatus();
      if (!status) return;

      setScenario(status.scenario as Scenario);
      setCapabilities(status.capabilities);
      setModuleStatuses(status.modules);
      setDegraded(status.degraded);
      setDegradationReason(status.degradation_reason);

      if (status.engine_active !== null) {
        setEngineActive(status.engine_active);
      }
      if (status.driver_version !== null) {
        setSystemInfo('driverVersion', `v${status.driver_version}`);
      }
      if (status.susfs_version) {
        setSystemInfo('susfsVersion', status.susfs_version);
      }
      setStats({
        activeRules: status.rule_count,
        excludedUids: status.excluded_uid_count,
        hiddenPaths: status.hidden_path_count,
      });
    } catch (e) {
      console.error('[ZM-Store] loadRuntimeStatus() error:', e);
    }
  };

  let lastKnownPackages: Set<string> | null = null;
  let lastTriggerTimestamp: number | null = null;
  let triggerPollInterval: number | undefined;
  let pollingStarted = false;
  let appFetchInProgress = false;
  const pendingUidOperations = new Set<number>();

  const fetchAppsViaKsuApi = async (): Promise<InstalledApp[]> => {
    const [userPackages, systemPackages] = await Promise.all([
      listPackages('user'),
      listPackages('system'),
    ]);

    const allPackages = [...new Set([...userPackages, ...systemPackages])];
    if (allPackages.length === 0) return [];

    const packageInfos = await getPackagesInfo(allPackages);
    const systemSet = new Set(systemPackages);

    return packageInfos.map(info => ({
      packageName: info.packageName,
      appName: info.appLabel || info.packageName,
      uid: info.uid ?? -1,
      isSystemApp: info.isSystemApp ?? systemSet.has(info.packageName),
    }));
  };

  const refreshApps = async () => {
    if (appFetchInProgress) return;
    appFetchInProgress = true;
    try {
      const freshApps = await fetchAppsViaKsuApi();
      setInstalledApps(freshApps);
      lastKnownPackages = new Set(freshApps.map(a => a.packageName));

      // Background: fetch labels for apps where KSU cache returned null
      const missingLabels = freshApps.filter(a => a.appName === a.packageName);
      if (missingLabels.length > 0 && missingLabels.length < 10) {
        for (const app of missingLabels) {
          getAppLabelViaAapt(app.packageName).then(label => {
            if (label && label !== app.packageName) {
              setInstalledApps(prev =>
                prev.map(a => a.packageName === app.packageName ? { ...a, appName: label } : a)
              );
            }
          });
        }
      }
    } finally {
      appFetchInProgress = false;
    }
  };

  const startTriggerPolling = () => {
    if (pollingStarted) return;
    pollingStarted = true;

    triggerPollInterval = window.setInterval(async () => {
      if (appFetchInProgress) return;

      try {
        const newTrigger = await api.getRefreshTrigger();

        // Trigger file doesn't exist - fall back to package count comparison
        if (newTrigger === null) {
          if (lastKnownPackages !== null) {
            const [userPkgs, systemPkgs] = await Promise.all([
              listPackages('user'),
              listPackages('system'),
            ]);
            const currentCount = new Set([...userPkgs, ...systemPkgs]).size;
            const lastCount = lastKnownPackages.size;
            if (currentCount !== lastCount) {
              console.log('[ZM-Store] Package count changed, refreshing');
              await refreshApps();
            }
          }
          return;
        }

        // First poll with trigger file present - record baseline
        if (lastTriggerTimestamp === null) {
          lastTriggerTimestamp = newTrigger;
          return;
        }

        // Trigger changed - daemon signaled a refresh
        if (newTrigger !== lastTriggerTimestamp) {
          console.log('[ZM-Store] Trigger file changed, refreshing app list');
          lastTriggerTimestamp = newTrigger;
          await refreshApps();
        }
      } catch (e) {
        console.error('[ZM-Store] Polling error:', e);
      }
    }, 2000);
  };

  const stopTriggerPolling = () => {
    if (triggerPollInterval) {
      clearInterval(triggerPollInterval);
      triggerPollInterval = undefined;
    }
    pollingStarted = false;
  };

  const loadInstalledApps = async () => {
    if (appFetchInProgress) {
      console.log('[ZM-Store] App fetch already in progress, skipping');
      return;
    }
    appFetchInProgress = true;
    console.log('[ZM-Store] loadInstalledApps() called');
    setLoading('apps', true);
    try {
      if (shouldUseMock()) {
        const mockApps = await api.getInstalledApps();
        setInstalledApps(mockApps);
        console.log('[ZM-Store] loadInstalledApps() loaded', mockApps.length, 'mock apps');
        return;
      }

      const apps = await fetchAppsViaKsuApi();
      setInstalledApps(apps);
      console.log('[ZM-Store] loadInstalledApps() loaded', apps.length, 'apps via KSU API');

      // Initialize lastKnownPackages for change detection polling
      lastKnownPackages = new Set(apps.map(a => a.packageName));
      startTriggerPolling();
    } catch (err) {
      console.error('[ZM-Store] loadInstalledApps() error:', err);
      showToast('Failed to load apps', 'error');
    } finally {
      appFetchInProgress = false;
      setLoading('apps', false);
    }
  };

  const scanKsuModules = async () => {
    console.log('[ZM-Store] scanKsuModules() called');
    setLoading('modules', true);
    try {
      const mods = await api.scanKsuModules();
      setKsuModules(mods);
      console.log('[ZM-Store] scanKsuModules() loaded', mods.length, 'modules');
    } catch (err) {
      console.error('[ZM-Store] scanKsuModules() error:', err);
      showToast('Failed to scan modules', 'error');
    } finally {
      setLoading('modules', false);
    }
  };

  const loadKsuModule = async (moduleName: string, modulePath: string) => {
    console.log('[ZM-Store] loadKsuModule() called:', { moduleName, modulePath });
    try {
      const count = await api.loadKsuModule(moduleName, modulePath);
      setKsuModules(prev => prev.map(m =>
        m.path === modulePath ? { ...m, isLoaded: true } : m
      ));
      setStats('activeRules', s => s + count);
      console.log('[ZM-Store] loadKsuModule() success, added', count, 'rules');
      showToast(`Loaded ${moduleName} (${count} rules)`, 'success');
      return count;
    } catch (err) {
      console.error('[ZM-Store] loadKsuModule() error:', err);
      showToast(`Failed to load ${moduleName}`, 'error');
      throw err;
    }
  };

  const unloadKsuModule = async (moduleName: string, modulePath: string) => {
    console.log('[ZM-Store] unloadKsuModule() called:', { moduleName, modulePath });
    try {
      const count = await api.unloadKsuModule(moduleName, modulePath);
      setKsuModules(prev => prev.map(m =>
        m.path === modulePath ? { ...m, isLoaded: false } : m
      ));
      setStats('activeRules', s => Math.max(0, s - count));
      console.log('[ZM-Store] unloadKsuModule() success, removed', count, 'rules');
      showToast(`Unloaded ${moduleName} (${count} rules)`, 'success');
      return count;
    } catch (err) {
      console.error('[ZM-Store] unloadKsuModule() error:', err);
      showToast(`Failed to unload ${moduleName}`, 'error');
      throw err;
    }
  };

  return {
    activeTab,
    setActiveTab,
    engineActive,
    loading,
    stats,
    systemInfo,
    rules,
    excludedUids,
    activity,
    installedApps,
    ksuModules,
    scenario,
    capabilities,
    moduleStatuses,
    degraded,
    degradationReason,
    settings,
    currentTheme,
    toast,

    loadInitialData,
    loadInstalledApps,
    loadRuntimeStatus,
    loadBreneSettings,
    setBreneToggle,
    setSusfsToggle,
    setUnameMode,
    setUnameField,
    loadMountSettings,
    setMountStrategy,
    setMountStorageMode,
    setMountToggle,
    activeStrategy,
    scanKsuModules,
    loadKsuModule,
    unloadKsuModule,
    toggleEngine,
    excludeUid,
    includeUid,
    clearAllRules,
    updateSettings,
    fetchSystemColor,
    setVerboseLogging,
    showToast,
    stopPolling: stopTriggerPolling,
  };
}

console.log('[ZM-Store] Creating store root...');
export const store = createRoot(createAppStore);
console.log('[ZM-Store] Store created successfully');
