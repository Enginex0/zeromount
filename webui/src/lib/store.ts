import { createSignal, createRoot, createMemo, createEffect } from 'solid-js';
import { createStore } from 'solid-js/store';
import type { Tab, Scenario, VfsRule, ExcludedUid, ActivityItem, EngineStats, SystemInfo, Settings, InstalledApp, KsuModule, CapabilityFlags, ModuleStatus, BreneSettings, SusfsSettings, PerfSettings, UnameSettings, UnameMode, MountSettings, StorageMode, MountStrategy } from './types';
import { api, shouldUseMock } from './api';
import { listPackages, getPackagesInfo, getAppLabelViaAapt } from './ksuApi';
import { darkTheme, lightTheme, amoledTheme, applyTheme, getAccentStyles, accentPresets } from './theme';

function createAppStore() {
  console.log('[ZM-Store] createAppStore() initializing...');
  const [activeTab, setActiveTab] = createSignal<Tab>('status');

  const [engineActive, setEngineActive] = createSignal(false);

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
  let activitySeq = 0;
  const pushActivity = (type: ActivityItem['type'], message: string) => {
    activitySeq++;
    setActivity(prev => [{ id: `rt-${activitySeq}`, type, message, timestamp: new Date() }, ...prev].slice(0, 10));
    api.logActivity(type.toUpperCase(), message).catch(() => {});
  };
  const [installedApps, setInstalledApps] = createSignal<InstalledApp[]>([]);
  const [ksuModules, setKsuModules] = createSignal<KsuModule[]>([]);
  const [scenario, setScenario] = createSignal<Scenario>('none');
  const [capabilities, setCapabilities] = createSignal<CapabilityFlags | null>(null);
  const [moduleStatuses, setModuleStatuses] = createSignal<ModuleStatus[]>([]);
  const [fontModules, setFontModules] = createSignal<string[]>([]);
  const [degraded, setDegraded] = createSignal(false);
  const [degradationReason, setDegradationReason] = createSignal<string | null>(null);
  const [rootManager, setRootManager] = createSignal<string | null>(null);
  const [runtimeStrategy, setRuntimeStrategy] = createSignal<MountStrategy | null>(null);
  const [mountSource, _setMountSource] = createSignal<string | null>(null);
  const [resolvedStorageMode, setResolvedStorageMode] = createSignal<string | null>(null);
  const [lastApiError, setLastApiError] = createSignal<{ operation: string; error: unknown; timestamp: Date } | null>(null);

  const savedBgOpacity = typeof window !== 'undefined'
    ? parseFloat(localStorage.getItem('zeromount-bgOpacity') ?? '0.40')
    : 0.40;
  const [bgOpacity, _setBgOpacity] = createSignal(isNaN(savedBgOpacity) ? 0.40 : savedBgOpacity);
  const setBgOpacity = (value: number) => {
    const snapped = Math.round(value * 10) / 10;
    const clamped = Math.max(0, Math.min(1, snapped));
    _setBgOpacity(clamped);
    localStorage.setItem('zeromount-bgOpacity', String(clamped));
    document.documentElement.style.setProperty('--bg-opacity', String(clamped));
  };

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
    force_hide_lsposed: false,
    spoof_cmdline: false,
    hide_ksu_loops: false,
    prop_spoofing: true,
  };

  const defaultSusfs: SusfsSettings = {
    enabled: true,
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
    mount_source: 'auto',
    overlay_source: 'auto',
  };

  const defaultPerf: PerfSettings = {
    enabled: false,
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
    perf: { ...defaultPerf },
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
    let accentDebounce: ReturnType<typeof setTimeout> | undefined;
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible' && settings.autoAccentColor) {
        clearTimeout(accentDebounce);
        accentDebounce = setTimeout(() => {
          const colors = Object.keys(accentPresets);
          const newRandom = colors[Math.floor(Math.random() * colors.length)];
          setSettings({ accentColor: newRandom });
        }, 300);
      }
    });
  }

  // Toast notifications
  const [toast, setToast] = createSignal<{ message: string; type: 'success' | 'error' | 'info' | 'warning'; duration: number } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  // Actions
  const showToast = (message: string, type: 'success' | 'error' | 'info' | 'warning' = 'info', duration?: number) => {
    if (toastTimer) clearTimeout(toastTimer);
    const ms = duration || (type === 'warning' ? 4500 : 3000);
    setToast({ message, type, duration: ms });
    toastTimer = setTimeout(() => setToast(null), ms);
  };

  const loadInitialData = async () => {
    console.log('[ZM-Store] loadInitialData() starting...');
    setLoading({ status: true, rules: true, activity: true, modules: true });

    try {
      // Phase A (critical path): status + sysInfo + config dump + accent color
      // These populate the StatusTab and settings — render ASAP
      const criticalResults = await Promise.allSettled([
        api.getRuntimeStatus(),
        api.getSystemInfoBatched(),
        api.configDump(),
        settings.autoAccentColor ? api.fetchSystemColor() : Promise.resolve(null),
      ]);

      const status = criticalResults[0].status === 'fulfilled' ? criticalResults[0].value : null;
      const sysInfo = criticalResults[1].status === 'fulfilled' ? criticalResults[1].value : { driverVersion: '', kernelVersion: '', susfsVersion: '', uptime: '', deviceModel: '', androidVersion: '', selinuxStatus: '' };
      const dump = criticalResults[2].status === 'fulfilled' ? criticalResults[2].value : null;
      const systemColor = criticalResults[3].status === 'fulfilled' ? criticalResults[3].value : null;

      // Apply critical data immediately so StatusTab exits skeleton state
      if (status) {
        setScenario(status.scenario as Scenario);
        setCapabilities(status.capabilities);
        setModuleStatuses(status.modules);
        setFontModules(status.font_modules || []);
        setDegraded(status.degraded);
        setDegradationReason(status.degradation_reason);
        setEngineActive(status.engine_active ?? false);
        if (status.driver_version !== null) setSystemInfo('driverVersion', `v${status.driver_version}`);
        if (status.susfs_version) setSystemInfo('susfsVersion', status.susfs_version);
        setRootManager(status.root_manager);
        setRuntimeStrategy(status.active_strategy ?? null);
        _setMountSource(status.mount_source ?? null);
        setResolvedStorageMode(status.resolved_storage_mode ?? null);
        setStats({
          activeRules: status.rule_count,
          excludedUids: status.excluded_uid_count,
          hiddenPaths: status.hidden_path_count,
        });
      }
      setSystemInfo('kernelVersion', sysInfo.kernelVersion);
      setSystemInfo('uptime', sysInfo.uptime);
      setSystemInfo('deviceModel', sysInfo.deviceModel);
      setSystemInfo('androidVersion', sysInfo.androidVersion);
      setSystemInfo('selinuxStatus', sysInfo.selinuxStatus);
      if (!status) {
        setSystemInfo('driverVersion', sysInfo.driverVersion);
        setSystemInfo('susfsVersion', sysInfo.susfsVersion);
      }
      if (systemColor) setSettings({ accentColor: systemColor });
      setLoading('status', false);

      // Config-dependent loads (essentially free when dump exists — just parses the dump object)
      const configLoads = Promise.allSettled([
        loadBreneSettings(dump),
        loadSusfsSettings(dump),
        loadPerfSettings(dump),
        loadMountSettings(dump),
        loadVerboseState(dump),
      ]);

      // Phase B (deferred): rules, UIDs, activity, modules — needed for secondary tabs
      const deferredResults = await Promise.allSettled([
        api.getRules(),
        api.getExcludedUids(),
        api.getActivity(),
        api.scanKsuModules(),
      ]);

      const rulesData = deferredResults[0].status === 'fulfilled' ? deferredResults[0].value : [];
      const uidsData = deferredResults[1].status === 'fulfilled' ? deferredResults[1].value : [];
      const activityData = deferredResults[2].status === 'fulfilled' ? deferredResults[2].value : [];
      const ksuModulesData = deferredResults[3].status === 'fulfilled' ? deferredResults[3].value : [];

      // Apply deferred data
      setRules(rulesData);
      setExcludedUids(uidsData);
      setActivity(prev => {
        const runtimeItems = prev.filter(item => item.id.startsWith('rt-'));
        const rtMessages = new Set(runtimeItems.map(item => item.message));
        const deduped = activityData.filter(item => !rtMessages.has(item.message));
        const merged = [...runtimeItems, ...deduped];
        merged.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
        return merged.slice(0, 10);
      });
      if (!status) {
        setStats({
          activeRules: rulesData.length,
          excludedUids: uidsData.length,
        });
      }
      setKsuModules(ksuModulesData);

      // Wait for config loads to finish (usually already done by now)
      await configLoads;

      // Surface first rejected read so UI can show degraded state
      const allResults = [...criticalResults, ...deferredResults];
      const labels = ['status', 'sysInfo', 'configDump', 'color', 'rules', 'uids', 'activity', 'modules'];
      const firstFail = allResults.findIndex(r => r.status === 'rejected');
      if (firstFail !== -1) {
        setLastApiError({ operation: `loadInitialData:${labels[firstFail]}`, error: (allResults[firstFail] as PromiseRejectedResult).reason, timestamp: new Date() });
      } else {
        setLastApiError(null);
      }

      console.log('[ZM-Store] loadInitialData() complete');
    } catch (err) {
      console.error('[ZM-Store] loadInitialData() error:', err);
      setLastApiError({ operation: 'loadInitialData', error: err, timestamp: new Date() });
      showToast('Failed to load data', 'error');
    } finally {
      setLoading({ status: false, rules: false, activity: false, modules: false });
    }
  };

  const toggleEngine = async () => {
    if (loading.engine) return;

    const newState = !engineActive();
    console.log('[ZM-Store] toggleEngine() called, newState:', newState);
    setEngineActive(newState);
    pushActivity(newState ? 'engine_enabled' : 'engine_disabled', newState ? 'Engine → ON' : 'Engine → OFF');
    setLoading('engine', true);
    try {
      await api.toggleEngine(newState);
      console.log('[ZM-Store] toggleEngine() success, engine now:', newState);
      showToast(newState ? 'Engine activated' : 'Engine deactivated', 'success');
    } catch (err) {
      console.error('[ZM-Store] toggleEngine() error:', err);
      setEngineActive(!newState);
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
      pushActivity('uid_excluded', `${appName} (UID ${uid})`);
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
      pushActivity('uid_included', `UID ${uid} included`);
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
      pushActivity('rule_removed', 'All rules cleared');
      showToast('All rules cleared', 'success');
    } catch (err) {
      console.error('[ZM-Store] clearAllRules() error:', err);
      const msg = String(err).includes('errno') || String(err).includes('No such')
        ? 'ZeroMount VFS unavailable' : 'Failed to clear rules';
      showToast(msg, 'error');
    } finally {
      setLoading('rules', false);
    }
  };

  const updateSettings = (updates: Partial<Settings>) => {
    console.log('[ZM-Store] updateSettings() called:', updates);
    setSettings(updates);
    if (updates.theme) pushActivity('theme_changed', `Theme → ${updates.theme}`);
    if (updates.accentColor) pushActivity('theme_changed', `Accent → ${updates.accentColor}`);
    if (updates.fixedNav !== undefined) pushActivity('setting_changed', `Fixed nav → ${updates.fixedNav ? 'ON' : 'OFF'}`);
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
      pushActivity('setting_changed', `Verbose logging → ${enabled ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setVerboseLogging() error:', e);
      setSettings({ verboseLogging: !enabled });
      showToast('Failed to set verbose logging', 'error');
    }
  };

  const loadBreneSettings = async (dump?: Record<string, any> | null) => {
    const breneKeys: (keyof BreneSettings)[] = [
      'auto_hide_apk', 'auto_hide_zygisk', 'auto_hide_fonts',
      'auto_hide_rooted_folders', 'auto_hide_recovery', 'auto_hide_tmp',
      'auto_hide_sdcard_data', 'avc_log_spoofing', 'susfs_log',
      'hide_sus_mounts', 'emulate_vold_app_data', 'force_hide_lsposed',
      'spoof_cmdline', 'hide_ksu_loops', 'prop_spoofing',
    ];

    if (dump?.brene && dump?.uname) {
      const brene: Partial<BreneSettings> = {};
      for (const key of breneKeys) {
        if (key in dump.brene) {
          const v = dump.brene[key];
          brene[key] = typeof v === 'boolean' ? v : String(v) === 'true';
        }
      }
      setSettings('brene', prev => ({ ...prev, ...brene }));

      const uname: Partial<UnameSettings> = {};
      if (dump.uname.mode != null) uname.mode = dump.uname.mode as UnameMode;
      if (dump.uname.release != null) uname.release = String(dump.uname.release);
      if (dump.uname.version != null) uname.version = String(dump.uname.version);
      setSettings('uname', prev => ({ ...prev, ...uname }));
      return;
    }

    // Fallback: individual configGet calls
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
    let kernelSet = false;
    let configVarSet = false;
    try {
      await api.configSet(`brene.${key}`, String(value));
      // Chain controlled settings to SUSFS kernel + config.sh
      if (key === 'avc_log_spoofing') {
        await api.setSusfsAvcSpoofing(value);
        kernelSet = true;
        await api.writeSusfsConfigVar('avc_log_spoofing', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'susfs_log') {
        await api.setSusfsLog(value);
        kernelSet = true;
        await api.writeSusfsConfigVar('susfs_log', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'hide_sus_mounts') {
        await api.setSusfsHideMounts(value);
        kernelSet = true;
        await api.writeSusfsConfigVar('hide_sus_mnts_for_all_or_non_su_procs', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'emulate_vold_app_data') {
        await api.writeSusfsConfigVar('emulate_vold_app_data', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'force_hide_lsposed') {
        await api.writeSusfsConfigVar('force_hide_lsposed', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'spoof_cmdline') {
        await api.writeSusfsConfigVar('spoof_cmdline', value ? '1' : '0');
        configVarSet = true;
      } else if (key === 'hide_ksu_loops') {
        await api.writeSusfsConfigVar('hide_loops', value ? '1' : '0');
        configVarSet = true;
      }
      pushActivity('brene_toggle', `${key} → ${value ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setBreneToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('brene', key, !value);
      // Best-effort rollback: config, kernel, config.sh
      const old = !value;
      api.configSet(`brene.${key}`, String(old)).catch(re => console.warn('[ZM-Store] rollback configSet failed:', re));
      if (kernelSet) {
        const rollbackKernel = key === 'avc_log_spoofing' ? api.setSusfsAvcSpoofing(old)
          : key === 'susfs_log' ? api.setSusfsLog(old)
          : key === 'hide_sus_mounts' ? api.setSusfsHideMounts(old)
          : null;
        rollbackKernel?.catch(re => console.warn('[ZM-Store] rollback kernel call failed:', re));
      }
      if (configVarSet) {
        const varMap: Record<string, string> = {
          avc_log_spoofing: 'avc_log_spoofing',
          susfs_log: 'susfs_log',
          hide_sus_mounts: 'hide_sus_mnts_for_all_or_non_su_procs',
          emulate_vold_app_data: 'emulate_vold_app_data',
          force_hide_lsposed: 'force_hide_lsposed',
          spoof_cmdline: 'spoof_cmdline',
          hide_ksu_loops: 'hide_loops',
        };
        const varName = varMap[key];
        if (varName) {
          api.writeSusfsConfigVar(varName, old ? '1' : '0').catch(re => console.warn('[ZM-Store] rollback config.sh failed:', re));
        }
      }
    }
  };

  const setSusfsToggle = async (key: keyof SusfsSettings, value: boolean) => {
    setSettings('susfs', key, value);
    try {
      await api.configSet(`susfs.${key}`, String(value));
      pushActivity('susfs_toggle', `${key} → ${value ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setSusfsToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('susfs', key, !value);
    }
  };

  const setPerfToggle = async (key: keyof PerfSettings, value: boolean) => {
    setSettings('perf', key, value);
    try {
      await api.configSet(`perf.${key}`, String(value));
      pushActivity('setting_changed', `perf.${key} → ${value ? 'ON' : 'OFF'}`);
    } catch (e) {
      console.error('[ZM-Store] setPerfToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('perf', key, !value);
    }
  };

  const setUnameMode = async (mode: UnameMode) => {
    const prev = settings.uname.mode;
    setSettings('uname', 'mode', mode);
    try {
      await api.configSet('uname.mode', mode);
      pushActivity('setting_changed', `Uname mode → ${mode}`);
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
      pushActivity('setting_changed', `Uname ${field} → ${value || '(empty)'}`);
    } catch (e) {
      console.error('[ZM-Store] setUnameField() error:', e);
      showToast(`Failed to save uname ${field}`, 'error');
      setSettings('uname', field, prev);
    }
  };

  const loadSusfsSettings = async (dump?: Record<string, any> | null) => {
    if (dump?.susfs) {
      const s = dump.susfs;
      const susfs: Partial<SusfsSettings> = {};
      for (const key of ['enabled', 'path_hide', 'kstat', 'maps_hide', 'open_redirect'] as (keyof SusfsSettings)[]) {
        if (key in s) {
          const v = s[key];
          susfs[key] = typeof v === 'boolean' ? v : String(v) === 'true';
        }
      }
      setSettings('susfs', prev => ({ ...prev, ...susfs }));
      return;
    }

    const keys: (keyof SusfsSettings)[] = ['enabled', 'path_hide', 'kstat', 'maps_hide', 'open_redirect'];
    const results = await Promise.allSettled(keys.map(k => api.configGet(`susfs.${k}`)));
    const susfs: Partial<SusfsSettings> = {};
    keys.forEach((key, i) => {
      const r = results[i];
      if (r.status === 'fulfilled' && r.value !== null) {
        susfs[key] = r.value === 'true';
      }
    });
    setSettings('susfs', prev => ({ ...prev, ...susfs }));
  };

  const loadPerfSettings = async (dump?: Record<string, any> | null) => {
    if (dump?.perf) {
      const p = dump.perf;
      const perf: Partial<PerfSettings> = {};
      if ('enabled' in p) {
        const v = p.enabled;
        perf.enabled = typeof v === 'boolean' ? v : String(v) === 'true';
      }
      setSettings('perf', prev => ({ ...prev, ...perf }));
      return;
    }

    const result = await api.configGet('perf.enabled');
    if (result !== null) {
      setSettings('perf', 'enabled', result === 'true');
    }
  };

  const loadMountSettings = async (dump?: Record<string, any> | null) => {
    console.log('[ZM-Store] loadMountSettings() starting...');

    if (dump?.mount) {
      const m = dump.mount;
      const mount: Partial<MountSettings> = {};
      if (m.storage_mode != null) mount.storage_mode = m.storage_mode as StorageMode;
      if (m.overlay_preferred != null) mount.overlay_preferred = typeof m.overlay_preferred === 'boolean' ? m.overlay_preferred : String(m.overlay_preferred) === 'true';
      if (m.magic_mount_fallback != null) mount.magic_mount_fallback = typeof m.magic_mount_fallback === 'boolean' ? m.magic_mount_fallback : String(m.magic_mount_fallback) === 'true';
      if (m.random_mount_paths != null) mount.random_mount_paths = typeof m.random_mount_paths === 'boolean' ? m.random_mount_paths : String(m.random_mount_paths) === 'true';
      if (m.mount_source != null) mount.mount_source = String(m.mount_source);
      if (m.overlay_source != null) mount.overlay_source = String(m.overlay_source);
      setSettings('mount', prev => ({ ...prev, ...mount }));
      console.log('[ZM-Store] loadMountSettings() loaded from dump:', mount);
      return;
    }

    // Fallback: individual configGet calls — all in a single Promise.allSettled
    const boolKeys: (keyof MountSettings)[] = [
      'overlay_preferred', 'magic_mount_fallback', 'random_mount_paths',
    ];
    const results = await Promise.allSettled([
      api.configGet('mount.storage_mode'),
      ...boolKeys.map(k => api.configGet(`mount.${k}`)),
      api.configGet('mount.mount_source'),
      api.configGet('mount.overlay_source'),
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
    const mountSourceResult = results[boolKeys.length + 1];
    const overlaySourceResult = results[boolKeys.length + 2];
    if (mountSourceResult.status === 'fulfilled' && mountSourceResult.value !== null) {
      mount.mount_source = mountSourceResult.value;
    }
    if (overlaySourceResult.status === 'fulfilled' && overlaySourceResult.value !== null) {
      mount.overlay_source = overlaySourceResult.value;
    }
    setSettings('mount', prev => ({ ...prev, ...mount }));
    console.log('[ZM-Store] loadMountSettings() loaded:', mount);
  };

  const loadVerboseState = async (dump?: Record<string, any> | null) => {
    if (dump?.logging && 'verbose' in dump.logging) {
      const v = dump.logging.verbose;
      setSettings({ verboseLogging: typeof v === 'boolean' ? v : String(v) === 'true' });
      return;
    }
    // Fallback: individual call
    try {
      const verbose = await api.getVerboseLogging();
      setSettings({ verboseLogging: verbose });
    } catch (e) {
      // Non-fatal: default to false
    }
  };

  const setMountStorageMode = async (mode: StorageMode) => {
    console.log('[ZM-Store] setMountStorageMode() called:', mode);
    const prev = settings.mount.storage_mode;
    setSettings('mount', 'storage_mode', mode);
    try {
      await api.configSet('mount.storage_mode', mode);
      pushActivity('setting_changed', `Storage mode → ${mode}`);
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
      pushActivity('setting_changed', `${key} → ${value ? 'ON' : 'OFF'}`);
      console.log('[ZM-Store] setMountToggle() saved:', key, value);
    } catch (e) {
      console.error('[ZM-Store] setMountToggle() error:', e);
      showToast(`Failed to save ${key}`, 'error');
      setSettings('mount', key, prev);
    }
  };

  const setMountSource = async (value: string) => {
    const prev = settings.mount.mount_source;
    setSettings('mount', 'mount_source', value);
    try {
      await api.configSet('mount.mount_source', value);
      pushActivity('setting_changed', `Staging source → ${value}`);
    } catch (e) {
      showToast('Failed to save mount source', 'error');
      setSettings('mount', 'mount_source', prev);
    }
  };

  const setOverlaySource = async (value: string) => {
    const prev = settings.mount.overlay_source;
    setSettings('mount', 'overlay_source', value);
    try {
      await api.configSet('mount.overlay_source', value);
      pushActivity('setting_changed', `Overlay source → ${value}`);
    } catch (e) {
      showToast('Failed to save overlay source', 'error');
      setSettings('mount', 'overlay_source', prev);
    }
  };

  const setMountStrategy = async (strategy: MountStrategy) => {
    console.log('[ZM-Store] setMountStrategy() called:', strategy);
    const prevOverlay = settings.mount.overlay_preferred;
    const prevMagic = settings.mount.magic_mount_fallback;

    const mapping: Record<string, [boolean, boolean]> = {
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
      pushActivity('mount_strategy_changed', `Strategy → ${strategy}`);
      showToast('Mount strategy changed — reboot to apply', 'warning');
      console.log('[ZM-Store] setMountStrategy() saved:', strategy, '→ overlay_preferred:', newOverlay, 'magic_mount_fallback:', newMagic);
    } catch (e) {
      console.error('[ZM-Store] setMountStrategy() error:', e);
      showToast('Failed to save mount strategy', 'error');
      setSettings('mount', 'overlay_preferred', prevOverlay);
      setSettings('mount', 'magic_mount_fallback', prevMagic);
    }
  };

  const activeStrategy = createMemo((): MountStrategy => {
    if (settings.mount.overlay_preferred && settings.mount.magic_mount_fallback) return 'Vfs';
    if (settings.mount.overlay_preferred && !settings.mount.magic_mount_fallback) return 'Overlay';
    return 'MagicMount';
  });

  const effectiveStrategy = createMemo((): MountStrategy => {
    const strategy = activeStrategy();
    const caps = capabilities();
    // Capabilities not loaded yet — show config-based strategy, not MagicMount fallback
    if (!caps) return strategy;
    if (strategy === 'Vfs' && !caps.vfs_driver) {
      return caps.overlay_supported ? 'Overlay' : 'MagicMount';
    }
    if (strategy === 'Overlay' && !caps.overlay_supported) {
      return 'MagicMount';
    }
    return strategy;
  });

  const loadRuntimeStatus = async () => {
    try {
      const status = await api.getRuntimeStatus();
      if (!status) return;

      setScenario(status.scenario as Scenario);
      setCapabilities(status.capabilities);
      setModuleStatuses(status.modules);
      setFontModules(status.font_modules || []);
      setDegraded(status.degraded);
      setDegradationReason(status.degradation_reason);
      setRootManager(status.root_manager);
      setRuntimeStrategy(status.active_strategy ?? null);
      _setMountSource(status.mount_source ?? null);
      setResolvedStorageMode(status.resolved_storage_mode ?? null);

      setEngineActive(status.engine_active ?? false);
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
      setLastApiError({ operation: 'loadRuntimeStatus', error: e, timestamp: new Date() });
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
      isSystemApp: info.isSystem ?? systemSet.has(info.packageName),
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
      const seen = new Set<string>();
      const missingLabels = freshApps.filter(a => {
        if (a.appName !== a.packageName || seen.has(a.packageName)) return false;
        seen.add(a.packageName);
        return true;
      });
      if (missingLabels.length > 0 && missingLabels.length < 10) {
        for (const app of missingLabels) {
          getAppLabelViaAapt(app.packageName).then(label => {
            if (label && label !== app.packageName) {
              setInstalledApps(prev =>
                prev.map(a => a.packageName === app.packageName ? { ...a, appName: label } : a)
              );
            }
          }).catch(() => {});
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
    fontModules,
    degraded,
    degradationReason,
    rootManager,
    runtimeStrategy,
    mountSource,
    resolvedStorageMode,
    bgOpacity,
    setBgOpacity,
    settings,
    currentTheme,
    toast,
    lastApiError,

    loadInitialData,
    loadInstalledApps,
    loadRuntimeStatus,
    loadBreneSettings,
    setBreneToggle,
    setSusfsToggle,
    setPerfToggle,
    setUnameMode,
    setUnameField,
    loadMountSettings,
    setMountStrategy,
    setMountStorageMode,
    setMountToggle,
    setMountSource,
    setOverlaySource,
    activeStrategy,
    effectiveStrategy,
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
