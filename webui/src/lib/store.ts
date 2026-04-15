import { createSignal, createRoot, createMemo, createEffect, batch } from 'solid-js';
import { createStore } from 'solid-js/store';
import type { Tab, Scenario, VfsRule, ExcludedUid, ActivityItem, EngineStats, SystemInfo, Settings, InstalledApp, KsuModule, CapabilityFlags, ModuleStatus, BreneSettings, SusfsSettings, PerfSettings, EmojiSettings, AdbSettings, GuardSettings, GuardStatus, UnameSettings, UnameMode, MountSettings, StorageMode, MountStrategy, WebUiInitResponse, SusfsOwnership, BridgeValues } from './types';
import { api, shouldUseMock, invalidateCache, readFromCache } from './api';
import { PATHS, MODULE_ID_RE } from './constants';
import { listPackages, getPackagesInfo, getAppLabelViaAapt, runShell, type ExecResult } from './ksuApi';
import { darkTheme, lightTheme, amoledTheme, applyTheme, getAccentStyles, accentPresets, accentNames } from './theme';
import { readCache, writeCache, type HydratableState } from './cache';
import { t, loadLocale, detectLocale, LANGUAGES } from './i18n';

function createAppStore() {
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
    hiddenMaps: 0,
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
  const [emojiConflict, setEmojiConflict] = createSignal<string | null>(null);
  const [degraded, setDegraded] = createSignal(false);
  const [degradationReason, setDegradationReason] = createSignal<string | null>(null);
  const [rootManager, setRootManager] = createSignal<string | null>(null);
  const [runtimeStrategy, setRuntimeStrategy] = createSignal<MountStrategy | null>(null);
  const [mountSource, _setMountSource] = createSignal<string | null>(null);
  const [resolvedStorageMode, setResolvedStorageMode] = createSignal<string | null>(null);
  const [lastApiError, setLastApiError] = createSignal<{ operation: string; error: unknown; timestamp: Date } | null>(null);
  const [externalSusfsModule, setExternalSusfsModule] = createSignal<'susfs4ksu' | 'brene' | null>(null);
  const [bridgeValues, setBridgeValues] = createSignal<BridgeValues | null>(null);
  const [verboseDumpPath, setVerboseDumpPath] = createSignal<string | null>(null);
  const [guardStatus, setGuardStatus] = createSignal<GuardStatus>({
    enabled: true, recoveryLockout: false, bootcount: 0, disabled: false, lastRecovery: null,
    allowedModules: [], pfdMarkers: 0, svcMarkers: 0,
  });

  const savedBgOpacity = typeof window !== 'undefined'
    ? parseFloat(localStorage.getItem('zeromount-bgOpacity') ?? '0.35')
    : 0.35;
  const [bgOpacity, _setBgOpacity] = createSignal(isNaN(savedBgOpacity) ? 0.35 : savedBgOpacity);
  const setBgOpacity = (value: number) => {
    const clamped = Math.max(0, Math.min(1, value));
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

  const savedLanguage = typeof window !== 'undefined'
    ? localStorage.getItem('zeromount-language')
    : null;
  if (savedLanguage) {
    loadLocale(savedLanguage);
  } else if (typeof window !== 'undefined') {
    const detected = detectLocale(LANGUAGES.map(l => l.code));
    if (detected !== 'en') loadLocale(detected);
  }

  const accentColors = Object.keys(accentPresets);
  const randomAccent = accentColors[Math.floor(Math.random() * accentColors.length)];
  const firstOpen = savedAutoAccentRaw === null;
  const initialAccent = firstOpen ? '#FF6B6B' : savedAutoAccent ? randomAccent : (savedAccent && accentPresets[savedAccent] ? savedAccent : '#FF8E53');

  const defaultBrene: BreneSettings = {
    auto_hide_apk: true,
    auto_hide_zygisk: true,
    auto_hide_fonts: true,
    auto_hide_rooted_folders: true,
    auto_hide_recovery: true,
    auto_hide_tmp: true,
    avc_log_spoofing: true,
    susfs_log: false,
    hide_sus_mounts: true,
    hide_sus_mounts_off_after_boot: false,
    emulate_vold_app_data: true,
    vold_use_path_loop: true,
    force_hide_lsposed: true,
    spoof_cmdline: false,
    hide_ksu_loops: true,
    kernel_umount: true,
    try_umount: false,
    skip_legit_mounts: true,
    prop_spoofing: true,
    auto_hide_injections: true,
    hide_cusrom: 0,
  };

  const defaultSusfs: SusfsSettings = {
    enabled: true,
    path_hide: true,
    kstat: true,
    maps_hide: true,
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
    exclude_hosts_modules: true,
    module_blacklist: '',
    ext4_image_size_mb: 0,
    restart_framework: false,
  };

  const defaultPerf: PerfSettings = {
    enabled: false,
  };

  const defaultAdb: AdbSettings = {
    usb_debugging: false,
    developer_options: false,
    adb_root: false,
  };

  const defaultGuard: GuardSettings = {
    enabled: true,
    boot_timeout_secs: 100,
    zygote_watch_secs: 30,
    zygote_poll_secs: 4,
    zygote_max_restarts: 4,
    systemui_watch_secs: 30,
    systemui_poll_secs: 4,
    systemui_max_restarts: 3,
    systemui_absent_timeout_secs: 25,
    systemui_monitor_enabled: true,
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
    emoji: { enabled: false },
    adb: { ...defaultAdb },
    guard: { ...defaultGuard },
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

  createEffect(() => {
    applyTheme(currentTheme(), settings.accentColor);
  });

  createEffect(() => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('zeromount-theme', settings.theme);
    }
  });

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

  const susfsOwnership = createMemo((): SusfsOwnership => {
    const caps = capabilities();
    const external = externalSusfsModule();
    const enabled = settings.susfs.enabled;

    if (!caps?.susfs_available) return 'disabled';
    if (enabled) return 'embedded_active';
    if (external) return 'deferred_external';
    return 'disabled';
  });


  const [toast, setToast] = createSignal<{ message: string; type: 'success' | 'error' | 'info' | 'warning'; duration: number } | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  const showToast = (message: string, type: 'success' | 'error' | 'info' | 'warning' = 'info', duration?: number) => {
    if (toastTimer) clearTimeout(toastTimer);
    const ms = duration || (type === 'warning' ? 4500 : 3000);
    setToast({ message, type, duration: ms });
    toastTimer = setTimeout(() => setToast(null), ms);
  };

  const hydrateFromCache = () => {
    const cached = readCache();
    if (!cached) return false;

    batch(() => {
      setScenario(cached.scenario);
      setEngineActive(cached.engineActive);
      setCapabilities(cached.capabilities);
      setStats(cached.stats);
      setSystemInfo(cached.systemInfo);
      setModuleStatuses(cached.moduleStatuses);
      setFontModules(cached.fontModules);
      setDegraded(cached.degraded);
      setDegradationReason(cached.degradationReason);
      setRootManager(cached.rootManager);
      setRuntimeStrategy(cached.runtimeStrategy);
      _setMountSource(cached.mountSource);
      setResolvedStorageMode(cached.resolvedStorageMode);
      setRules(cached.rules);
      setExcludedUids(cached.excludedUids);
      setActivity(cached.activity);
      setKsuModules(cached.ksuModules);
      setSettings('brene', prev => ({ ...prev, ...cached.brene }));
      setSettings('susfs', prev => ({ ...prev, ...cached.susfs }));
      setSettings('uname', prev => ({ ...prev, ...cached.uname }));
      setSettings('mount', prev => ({ ...prev, ...cached.mount }));
      if (cached.adb && typeof cached.adb.adb_root === 'boolean') {
        setSettings('adb', 'adb_root', cached.adb.adb_root);
      }
      setSettings({ verboseLogging: cached.verboseLogging });
      if (cached.externalSusfsModule !== undefined) setExternalSusfsModule(cached.externalSusfsModule);
      if (cached.bridgeValues !== undefined) setBridgeValues(cached.bridgeValues);
    });
    return true;
  };

  const buildCacheState = (): HydratableState => ({
    scenario: scenario(),
    engineActive: engineActive(),
    capabilities: capabilities(),
    stats: { ...stats },
    systemInfo: { ...systemInfo },
    moduleStatuses: moduleStatuses(),
    fontModules: fontModules(),
    degraded: degraded(),
    degradationReason: degradationReason(),
    rootManager: rootManager(),
    runtimeStrategy: runtimeStrategy(),
    mountSource: mountSource(),
    resolvedStorageMode: resolvedStorageMode(),
    rules: rules(),
    excludedUids: excludedUids(),
    activity: activity(),
    ksuModules: ksuModules(),
    brene: { ...settings.brene },
    susfs: { ...settings.susfs },
    uname: { ...settings.uname },
    mount: { ...settings.mount },
    adb: { ...settings.adb },
    verboseLogging: settings.verboseLogging,
    externalSusfsModule: externalSusfsModule(),
    bridgeValues: bridgeValues(),
  });

  const applyBatchedResponse = (data: WebUiInitResponse) => {
    batch(() => {
    const s = data.status;
    setScenario(s.scenario as Scenario);
    setCapabilities(s.capabilities);
    setModuleStatuses(s.modules);
    setFontModules(s.font_modules || []);
    setDegraded(s.degraded);
    setDegradationReason(s.degradation_reason);
    setEngineActive(s.engine_active ?? false);
    if (s.driver_version !== null) setSystemInfo('driverVersion', `v${s.driver_version}`);
    setSystemInfo('susfsVersion', s.susfs_version || '');
    setRootManager(s.root_manager);
    setRuntimeStrategy(s.active_strategy ?? null);
    _setMountSource(s.mount_source ?? null);
    setResolvedStorageMode(s.resolved_storage_mode ?? null);
    setStats({
      activeRules: s.rule_count,
      excludedUids: s.excluded_uid_count,
      hiddenPaths: s.hidden_path_count,
      hiddenMaps: s.hidden_maps_count ?? 0,
    });

    const si = data.system_info;
    setSystemInfo('kernelVersion', si.kernelVersion);
    setSystemInfo('uptime', si.uptime);
    setSystemInfo('deviceModel', si.deviceModel);
    setSystemInfo('androidVersion', si.androidVersion);
    setSystemInfo('selinuxStatus', si.selinuxStatus);

    const cfg = data.config;
    if (cfg.brene) {
      const brene: Partial<BreneSettings> = {};
      for (const key of Object.keys(cfg.brene) as (keyof BreneSettings)[]) {
        if (key in cfg.brene) {
          const v = cfg.brene[key];
          (brene as any)[key] = typeof v === 'boolean' ? v : String(v) === 'true';
        }
      }
      setSettings('brene', prev => ({ ...prev, ...brene }));
    }
    if (cfg.susfs) {
      const susfs: Partial<SusfsSettings> = {};
      for (const key of Object.keys(cfg.susfs) as (keyof SusfsSettings)[]) {
        if (key in cfg.susfs) {
          const v = (cfg.susfs as any)[key];
          susfs[key] = typeof v === 'boolean' ? v : String(v) === 'true';
        }
      }
      setSettings('susfs', prev => ({ ...prev, ...susfs }));
    }
    if (cfg.uname) {
      setSettings('uname', prev => ({
        ...prev,
        mode: (cfg.uname.mode ?? prev.mode) as UnameMode,
        release: cfg.uname.release ?? prev.release,
        version: cfg.uname.version ?? prev.version,
      }));
    }
    if (cfg.mount) {
      setSettings('mount', prev => ({
        ...prev,
        storage_mode: (cfg.mount.storage_mode ?? prev.storage_mode) as StorageMode,
        overlay_preferred: cfg.mount.overlay_preferred ?? prev.overlay_preferred,
        magic_mount_fallback: cfg.mount.magic_mount_fallback ?? prev.magic_mount_fallback,
        random_mount_paths: cfg.mount.random_mount_paths ?? prev.random_mount_paths,
        mount_source: cfg.mount.mount_source ?? prev.mount_source,
        overlay_source: cfg.mount.overlay_source ?? prev.overlay_source,
        exclude_hosts_modules: cfg.mount.exclude_hosts_modules ?? prev.exclude_hosts_modules,
        module_blacklist: cfg.mount.module_blacklist ?? prev.module_blacklist,
        ext4_image_size_mb: cfg.mount.ext4_image_size_mb ?? prev.ext4_image_size_mb,
        restart_framework: cfg.mount.restart_framework ?? prev.restart_framework,
      }));
    }
    if (cfg.perf) {
      setSettings('perf', prev => ({
        ...prev,
        enabled: typeof cfg.perf.enabled === 'boolean' ? cfg.perf.enabled : prev.enabled,
      }));
    }
    if (cfg.emoji) {
      setSettings('emoji', prev => ({
        ...prev,
        enabled: typeof cfg.emoji.enabled === 'boolean' ? cfg.emoji.enabled : prev.enabled,
      }));
    }
    if (cfg.adb && typeof cfg.adb.adb_root === 'boolean') {
      setSettings('adb', 'adb_root', cfg.adb.adb_root);
    }
    setEmojiConflict(data.emoji_conflict || null);

    const extMod = s.capabilities?.external_susfs_module;
    setExternalSusfsModule(extMod && extMod !== 'none' ? extMod : null);
    setBridgeValues(data.bridge_values ?? null);

    if (data.guard) {
      setGuardStatus(data.guard);
    }
    if (cfg.guard) {
      setSettings('guard', prev => ({ ...prev, ...cfg.guard }));
    }

    if (cfg.logging) {
      setSettings({ verboseLogging: typeof cfg.logging.verbose === 'boolean' ? cfg.logging.verbose : settings.verboseLogging });
    }

    const serverLang = cfg.ui?.language;
    if (serverLang && !savedLanguage) loadLocale(serverLang);

    setRules(data.rules.map((r, i) => ({
      id: r.id || String(i + 1),
      name: r.name,
      source: r.source,
      target: r.target,
      createdAt: new Date(),
    })));

    setExcludedUids(data.excluded_uids.map(u => ({
      uid: u.uid,
      packageName: u.packageName,
      appName: u.appName,
      excludedAt: u.excludedAt ? new Date(u.excludedAt) : new Date(),
    })));

    const validTypes = ['rule_added', 'rule_removed', 'uid_excluded', 'uid_included', 'engine_enabled', 'engine_disabled', 'setting_changed', 'mount_strategy_changed', 'susfs_toggle', 'brene_toggle', 'theme_changed'];
    setActivity(prev => {
      const runtimeItems = prev.filter(item => item.id.startsWith('rt-'));
      const rtMessages = new Set(runtimeItems.map(item => item.message));
      const freshItems: ActivityItem[] = data.activity
        .filter(a => !rtMessages.has(a.message))
        .map(a => ({
          id: a.id,
          type: (validTypes.includes(a.type) ? a.type : 'engine_enabled') as ActivityItem['type'],
          message: a.message,
          timestamp: new Date(a.timestamp),
        }));
      const merged = [...runtimeItems, ...freshItems];
      merged.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
      return merged.slice(0, 10);
    });

    setKsuModules(data.modules);
    });
  };

  const loadInitialDataLegacy = async () => {

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

    batch(() => {
      if (status) {
        setScenario(status.scenario as Scenario);
        setCapabilities(status.capabilities);
        setModuleStatuses(status.modules);
        setFontModules(status.font_modules || []);
        setDegraded(status.degraded);
        setDegradationReason(status.degradation_reason);
        setEngineActive(status.engine_active ?? false);
        if (status.driver_version !== null) setSystemInfo('driverVersion', `v${status.driver_version}`);
        setSystemInfo('susfsVersion', status.susfs_version || '');
        setRootManager(status.root_manager);
        setRuntimeStrategy(status.active_strategy ?? null);
        _setMountSource(status.mount_source ?? null);
        setResolvedStorageMode(status.resolved_storage_mode ?? null);
        setStats({
          activeRules: status.rule_count,
          excludedUids: status.excluded_uid_count,
          hiddenPaths: status.hidden_path_count,
          hiddenMaps: status.hidden_maps_count ?? 0,
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
    });

    const configLoads = Promise.allSettled([
      loadBreneSettings(dump),
      loadSusfsSettings(dump),
      loadPerfSettings(dump),
      loadAdbSettings(dump),
      loadMountSettings(dump),
      loadVerboseState(dump),
    ]);

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

    batch(() => {
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
    });

    await configLoads;
  };

  const loadInitialData = async () => {
    setLoading({ status: true, rules: true, activity: true, modules: true });

    hydrateFromCache();

    try {
      // Layer 1+2: check inlined/cached data first (zero ksu.exec)
      const cached = await readFromCache();
      if (cached) {
        applyBatchedResponse(cached);
        // Check boot-time inlined accent color
        const inlinedAccent = (window as any).__ZM_ACCENT__;
        if (settings.autoAccentColor && typeof inlinedAccent === 'string' && inlinedAccent) {
          setSettings({ accentColor: inlinedAccent });
        }
        // Android owns developer_options + usb_debugging — always read live
        await loadAdbSettings(cached);
      } else {
        // Fallback: live ksu.exec (first install before reboot, or cache invalidated)
        const [batchedData, systemColor] = await Promise.all([
          api.webuiInit(),
          settings.autoAccentColor ? api.fetchSystemColor() : Promise.resolve(null),
        ]);

        if (systemColor) setSettings({ accentColor: systemColor });

        if (batchedData) {
          applyBatchedResponse(batchedData);
          await loadAdbSettings(batchedData);
        } else {
          await loadInitialDataLegacy();
        }
      }

      writeCache(buildCacheState());
      setLastApiError(null);
    } catch (err) {
      setLastApiError({ operation: 'loadInitialData', error: err, timestamp: new Date() });
      showToast(t('toast.failedLoadData'), 'error');
    } finally {
      setLoading({ status: false, rules: false, activity: false, modules: false });
    }
  };

  const toggleEngine = async () => {
    if (loading.engine) return;

    const newState = !engineActive();
    setEngineActive(newState);
    pushActivity(newState ? 'engine_enabled' : 'engine_disabled', newState ? t('activity.engineOn') : t('activity.engineOff'));
    setLoading('engine', true);
    try {
      await api.toggleEngine(newState);
      showToast(newState ? t('toast.engineActivated') : t('toast.engineDeactivated'), 'success');
    } catch (err) {
      setEngineActive(!newState);
      showToast(t('toast.failedToggleEngine'), 'error');
    } finally {
      setLoading('engine', false);
    }
  };

  const excludeUid = async (uid: number, packageName: string, appName: string) => {
    if (uid <= 0) {
      showToast(t('toast.cannotExcludeUnknownUid'), 'error');
      return null;
    }
    if (pendingUidOperations.has(uid)) {
      return null;
    }
    pendingUidOperations.add(uid);
    setLoading('apps', true);
    try {
      const excluded = await api.excludeUid(uid, packageName, appName);
      setExcludedUids(prev => [...prev, excluded]);
      setStats('excludedUids', s => s + 1);
      pushActivity('uid_excluded', t('activity.uidExcluded', { name: appName, uid }));
      showToast(t('toast.excludedApp', { appName }), 'success');
      return excluded;
    } catch (err) {
      showToast(t('toast.failedExcludeUid'), 'error');
      throw err;
    } finally {
      pendingUidOperations.delete(uid);
      setLoading('apps', false);
    }
  };

  const includeUid = async (uid: number) => {
    if (uid <= 0) {
      showToast(t('toast.cannotIncludeUnknownUid'), 'error');
      return;
    }
    if (pendingUidOperations.has(uid)) {
      return;
    }
    pendingUidOperations.add(uid);
    setLoading('apps', true);
    try {
      await api.includeUid(uid);
      setExcludedUids(prev => prev.filter(u => u.uid !== uid));
      setStats('excludedUids', s => s - 1);
      pushActivity('uid_included', t('activity.uidIncluded', { uid }));
      showToast(t('toast.uidIncluded'), 'success');
    } catch (err) {
      showToast(t('toast.failedIncludeUid'), 'error');
    } finally {
      pendingUidOperations.delete(uid);
      setLoading('apps', false);
    }
  };

  const clearAllRules = async () => {
    setLoading('rules', true);
    try {
      await api.clearAllRules();
      setRules([]);
      setStats('activeRules', 0);
      pushActivity('rule_removed', t('activity.allRulesCleared'));
      showToast(t('toast.allRulesCleared'), 'success');
    } catch (err) {
      const msg = String(err).includes('errno') || String(err).includes('No such')
        ? t('toast.vfsUnavailable') : t('toast.failedClearRules');
      showToast(msg, 'error');
    } finally {
      setLoading('rules', false);
    }
  };

  const updateSettings = (updates: Partial<Settings>) => {
    setSettings(updates);
    if (updates.theme) pushActivity('theme_changed', t('activity.themeChanged', { value: updates.theme }));
    if (updates.accentColor) pushActivity('theme_changed', t('activity.accentChanged', { value: accentNames[updates.accentColor] || updates.accentColor }));
    if (updates.fixedNav !== undefined) pushActivity('setting_changed', t('activity.fixedNavChanged', { value: updates.fixedNav ? t('activity.on') : t('activity.off') }));
  };

  const fetchSystemColor = async () => {
    try {
      const systemColor = await api.fetchSystemColor();
      if (systemColor) {
        setSettings({ accentColor: systemColor });
      }
    } catch (e) {
    }
  };

  const setVerboseLogging = async (enabled: boolean) => {
    setSettings({ verboseLogging: enabled });
    try {
      await api.setVerboseLogging(enabled);
      pushActivity('setting_changed', t('activity.verboseChanged', { value: enabled ? t('activity.on') : t('activity.off') }));
      if (enabled) {
        showToast(t('toast.rebootingVerbose'), 'warning', 3000);
        const timerId = setTimeout(async () => {
          try { await api.reboot(); }
          catch { showToast(t('toast.rebootFailed'), 'error'); }
        }, 3000);
        (window as any).__zmRebootTimer = timerId;
      } else {
        if ((window as any).__zmRebootTimer) {
          clearTimeout((window as any).__zmRebootTimer);
          (window as any).__zmRebootTimer = null;
        }
        showToast(t('toast.verboseDisabled'), 'info');
        setVerboseDumpPath(null);
      }
    } catch (e) {
      setSettings({ verboseLogging: !enabled });
      showToast(t('toast.failedSetVerbose'), 'error');
    }
  };

  const loadBreneSettings = async (dump?: Record<string, any> | null) => {
    const breneKeys: (keyof BreneSettings)[] = [
      'auto_hide_apk', 'auto_hide_zygisk', 'auto_hide_fonts',
      'auto_hide_rooted_folders', 'auto_hide_recovery', 'auto_hide_tmp',
      'avc_log_spoofing', 'susfs_log',
      'hide_sus_mounts', 'emulate_vold_app_data', 'force_hide_lsposed',
      'spoof_cmdline', 'hide_ksu_loops', 'kernel_umount', 'try_umount', 'prop_spoofing', 'auto_hide_injections',
    ];

    if (dump?.brene && dump?.uname) {
      const brene: Partial<BreneSettings> = {};
      for (const key of breneKeys) {
        if (key in dump.brene) {
          const v = dump.brene[key];
          (brene as any)[key] = typeof v === 'boolean' ? v : String(v) === 'true';
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
        (brene as any)[key] = r.value === 'true';
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
    try {
      await api.configSet(`brene.${key}`, String(value));

      // Live supercalls for kernel-immediate toggles
      if (key === 'avc_log_spoofing') {
        await api.setSusfsAvcSpoofing(value);
        kernelSet = true;
      } else if (key === 'susfs_log') {
        await api.setSusfsLog(value);
        kernelSet = true;
      } else if (key === 'hide_sus_mounts') {
        await api.setSusfsHideMounts(value);
        kernelSet = true;
      } else if (key === 'kernel_umount') {
        await api.setKernelUmount(value);
        kernelSet = true;
      }

      // Bridge write syncs config.toml -> external module config.sh
      await api.bridgeWrite(`brene.${key}`, String(value));

      pushActivity('brene_toggle', t('activity.settingChanged', { key, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('brene', key, !value);
      const old = !value;
      api.configSet(`brene.${key}`, String(old)).catch(() => {});
      if (kernelSet) {
        const rollbackKernel = key === 'avc_log_spoofing' ? api.setSusfsAvcSpoofing(old)
          : key === 'susfs_log' ? api.setSusfsLog(old)
          : key === 'hide_sus_mounts' ? api.setSusfsHideMounts(old)
          : key === 'kernel_umount' ? api.setKernelUmount(old)
          : null;
        rollbackKernel?.catch(() => {});
      }
      api.bridgeWrite(`brene.${key}`, String(old)).catch(() => {});
    }
  };

  const setBreneNumeric = async (key: keyof BreneSettings, value: number) => {
    setSettings('brene', key, value as never);
    try {
      await api.configSet(`brene.${key}`, String(value));
      await api.bridgeWrite(`brene.${key}`, String(value));
      pushActivity('brene_toggle', t('activity.settingChanged', { key, value: String(value) }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
    }
  };

  const setSusfsToggle = async (key: keyof SusfsSettings, value: boolean) => {
    setSettings('susfs', key, value);
    try {
      await api.configSet(`susfs.${key}`, String(value));
      pushActivity('susfs_toggle', t('activity.settingChanged', { key, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('susfs', key, !value);
    }
  };

  const setPerfToggle = async (key: keyof PerfSettings, value: boolean) => {
    setSettings('perf', key, value);
    try {
      await api.configSet(`perf.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key: `perf.${key}`, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('perf', key, !value);
    }
  };

  const setEmojiToggle = async (key: keyof EmojiSettings, value: boolean) => {
    setSettings('emoji', key, value);
    try {
      await api.configSet(`emoji.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key: `emoji.${key}`, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('emoji', key, !value);
    }
  };

  // Only adb_root persists to config; developer_options + usb_debugging are
  // written to Android directly by the toggle handler (Android owns the truth).
  const setAdbToggle = async (key: keyof AdbSettings, value: boolean) => {
    setSettings('adb', key, value);
    if (key !== 'adb_root') {
      pushActivity('setting_changed', t('activity.settingChanged', { key: `adb.${key}`, value: value ? t('activity.on') : t('activity.off') }));
      return;
    }
    try {
      await api.configSet(`adb.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key: `adb.${key}`, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('adb', key, !value);
    }
  };

  const setGuardToggle = async (key: 'enabled' | 'systemui_monitor_enabled', value: boolean) => {
    setSettings('guard', key, value);
    if (key === 'enabled') setGuardStatus(prev => ({ ...prev, enabled: value }));
    try {
      await api.configSet(`guard.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key: `guard.${key}`, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key: `guard.${key}` }), 'error');
      setSettings('guard', key, !value);
      if (key === 'enabled') setGuardStatus(prev => ({ ...prev, enabled: !value }));
    }
  };

  const guardAllowModule = async (name: string) => {
    if (!MODULE_ID_RE.test(name) || name.length > 256) {
      showToast(t('toast.failedWhitelist', { name }), 'error');
      return;
    }
    try {
      await runShell(`${PATHS.BINARY} guard allow ${name}`);
      setGuardStatus(prev => ({
        ...prev,
        allowedModules: [...prev.allowedModules.filter(m => m !== name), name],
      }));
      showToast(t('toast.addedToWhitelist', { name }), 'success');
    } catch (e) {
      showToast(t('toast.failedWhitelist', { name }), 'error');
    }
  };

  const guardDisallowModule = async (name: string) => {
    if (name === 'meta-zeromount') {
      showToast(t('toast.cannotRemoveSelf'), 'error');
      return;
    }
    if (!MODULE_ID_RE.test(name) || name.length > 256) {
      showToast(t('toast.failedRemove', { name }), 'error');
      return;
    }
    try {
      await runShell(`${PATHS.BINARY} guard disallow ${name}`);
      setGuardStatus(prev => ({
        ...prev,
        allowedModules: prev.allowedModules.filter(m => m !== name),
      }));
      showToast(t('toast.removedFromWhitelist', { name }), 'success');
    } catch (e) {
      showToast(t('toast.failedRemove', { name }), 'error');
    }
  };

  const guardClearLockout = async () => {
    try {
      await runShell(`${PATHS.BINARY} guard clear-lockout`);
      setGuardStatus(prev => ({ ...prev, recoveryLockout: false, pfdMarkers: 0, svcMarkers: 0 }));
      showToast(t('toast.lockoutCleared'), 'success');
    } catch {
      showToast(t('toast.failedClearLockout'), 'error');
    }
  };

  const setUnameMode = async (mode: UnameMode) => {
    const prev = settings.uname.mode;
    setSettings('uname', 'mode', mode);
    try {
      await api.configSet('uname.mode', mode);
      pushActivity('setting_changed', t('activity.settingChanged', { key: 'uname.mode', value: mode }));
    } catch (e) {
      showToast(t('toast.failedSaveUnameMode'), 'error');
      setSettings('uname', 'mode', prev);
    }
  };

  const setUnameField = async (field: 'release' | 'version', value: string) => {
    const prev = settings.uname[field];
    setSettings('uname', field, value);
    try {
      await api.configSet(`uname.${field}`, value);
      pushActivity('setting_changed', t('activity.settingChanged', { key: `uname.${field}`, value: value || t('activity.empty') }));
    } catch (e) {
      showToast(t('toast.failedSaveUname', { field }), 'error');
      setSettings('uname', field, prev);
    }
  };

  const loadSusfsSettings = async (dump?: Record<string, any> | null) => {
    if (dump?.susfs) {
      const s = dump.susfs;
      const susfs: Partial<SusfsSettings> = {};
      for (const key of ['enabled', 'path_hide', 'kstat', 'maps_hide'] as (keyof SusfsSettings)[]) {
        if (key in s) {
          const v = s[key];
          susfs[key] = typeof v === 'boolean' ? v : String(v) === 'true';
        }
      }
      setSettings('susfs', prev => ({ ...prev, ...susfs }));
      return;
    }

    const keys: (keyof SusfsSettings)[] = ['enabled', 'path_hide', 'kstat', 'maps_hide'];
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

  // adb_root: ZeroMount behavior, config is source of truth (consumed by post-fs-data.sh).
  // developer_options + usb_debugging: Android owns the truth, always read live.
  const loadAdbSettings = async (dump?: Record<string, any> | null) => {
    if (dump?.adb && 'adb_root' in dump.adb) {
      const v = dump.adb.adb_root;
      setSettings('adb', 'adb_root', typeof v === 'boolean' ? v : String(v) === 'true');
    } else {
      const r = await api.configGet('adb.adb_root');
      if (r !== null) setSettings('adb', 'adb_root', r === 'true');
    }

    const [devResult, usbResult] = await Promise.allSettled([
      runShell('/system/bin/settings get global development_settings_enabled'),
      runShell('/system/bin/settings get global adb_enabled'),
    ]);
    const parseFlag = (label: string, r: PromiseSettledResult<ExecResult>): boolean => {
      if (r.status !== 'fulfilled') {
        console.warn(`[adb] ${label}: exec rejected`, r.reason);
        return false;
      }
      const { errno, stdout, stderr } = r.value;
      const trimmed = (stdout ?? '').trim();
      if (errno !== 0) {
        console.warn(`[adb] ${label}: errno=${errno} stderr=${stderr} stdout="${trimmed}"`);
        return false;
      }
      if (trimmed === '1' || trimmed === 'true') return true;
      if (trimmed === '0' || trimmed === 'null' || trimmed === '') return false;
      console.warn(`[adb] ${label}: unexpected stdout="${trimmed}"`);
      return trimmed.startsWith('1');
    };
    setSettings('adb', prev => ({
      ...prev,
      developer_options: parseFlag('developer_options', devResult),
      usb_debugging: parseFlag('usb_debugging', usbResult),
    }));
  };

  const loadMountSettings = async (dump?: Record<string, any> | null) => {

    if (dump?.mount) {
      const m = dump.mount;
      const mount: Partial<MountSettings> = {};
      if (m.storage_mode != null) mount.storage_mode = m.storage_mode as StorageMode;
      if (m.overlay_preferred != null) mount.overlay_preferred = typeof m.overlay_preferred === 'boolean' ? m.overlay_preferred : String(m.overlay_preferred) === 'true';
      if (m.magic_mount_fallback != null) mount.magic_mount_fallback = typeof m.magic_mount_fallback === 'boolean' ? m.magic_mount_fallback : String(m.magic_mount_fallback) === 'true';
      if (m.random_mount_paths != null) mount.random_mount_paths = typeof m.random_mount_paths === 'boolean' ? m.random_mount_paths : String(m.random_mount_paths) === 'true';
      if (m.mount_source != null) mount.mount_source = String(m.mount_source);
      if (m.overlay_source != null) mount.overlay_source = String(m.overlay_source);
      if (m.exclude_hosts_modules != null) mount.exclude_hosts_modules = typeof m.exclude_hosts_modules === 'boolean' ? m.exclude_hosts_modules : String(m.exclude_hosts_modules) === 'true';
      if (m.module_blacklist != null) mount.module_blacklist = String(m.module_blacklist);
      if (m.ext4_image_size_mb != null) mount.ext4_image_size_mb = typeof m.ext4_image_size_mb === 'number' ? m.ext4_image_size_mb : parseInt(String(m.ext4_image_size_mb), 10) || 0;
      if (m.restart_framework != null) mount.restart_framework = typeof m.restart_framework === 'boolean' ? m.restart_framework : String(m.restart_framework) === 'true';
      setSettings('mount', prev => ({ ...prev, ...mount }));
      return;
    }

    // Fallback: individual configGet calls — all in a single Promise.allSettled
    const boolKeys: (keyof MountSettings)[] = [
      'overlay_preferred', 'magic_mount_fallback', 'random_mount_paths',
      'exclude_hosts_modules', 'restart_framework',
    ];
    const results = await Promise.allSettled([
      api.configGet('mount.storage_mode'),
      ...boolKeys.map(k => api.configGet(`mount.${k}`)),
      api.configGet('mount.mount_source'),
      api.configGet('mount.overlay_source'),
      api.configGet('mount.module_blacklist'),
      api.configGet('mount.ext4_image_size_mb'),
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
    const blacklistResult = results[boolKeys.length + 3];
    const ext4SizeResult = results[boolKeys.length + 4];
    if (mountSourceResult.status === 'fulfilled' && mountSourceResult.value !== null) {
      mount.mount_source = mountSourceResult.value;
    }
    if (overlaySourceResult.status === 'fulfilled' && overlaySourceResult.value !== null) {
      mount.overlay_source = overlaySourceResult.value;
    }
    if (blacklistResult.status === 'fulfilled' && blacklistResult.value !== null) {
      mount.module_blacklist = blacklistResult.value;
    }
    if (ext4SizeResult.status === 'fulfilled' && ext4SizeResult.value !== null) {
      mount.ext4_image_size_mb = parseInt(ext4SizeResult.value, 10) || 0;
    }
    setSettings('mount', prev => ({ ...prev, ...mount }));
  };

  const loadVerboseState = async (dump?: Record<string, any> | null) => {
    if (dump?.logging && 'verbose' in dump.logging) {
      const v = dump.logging.verbose;
      setSettings({ verboseLogging: typeof v === 'boolean' ? v : String(v) === 'true' });
    } else {
      try {
        const verbose = await api.getVerboseLogging();
        setSettings({ verboseLogging: verbose });
      } catch (e) {
        // Non-fatal: default to false
      }
    }
    try {
      const dumpPath = await api.getVerboseDumpPath();
      setVerboseDumpPath(dumpPath);
    } catch (e) {
    }
  };

  const refreshBridgeValues = async () => {
    const ext = externalSusfsModule();
    if (!ext) return;
    const basePath = ext === 'brene'
      ? '/data/adb/brene/config.sh'
      : '/data/adb/susfs4ksu/config.sh';
    try {
      const values = await api.readAllBridgeValues(basePath);
      setBridgeValues({ module: ext, values });
    } catch (e) {
      console.error('[ZM-Store] refreshBridgeValues() error:', e);
    }
  };

  if (typeof window !== 'undefined') {
    document.addEventListener('visibilitychange', () => {
      if (document.visibilityState === 'visible' && externalSusfsModule()) {
        refreshBridgeValues();
      }
    });
  }

  const setMountStorageMode = async (mode: StorageMode) => {
    const prev = settings.mount.storage_mode;
    setSettings('mount', 'storage_mode', mode);
    try {
      await api.configSet('mount.storage_mode', mode);
      pushActivity('setting_changed', t('activity.settingChanged', { key: 'storage_mode', value: mode }));
    } catch (e) {
      showToast(t('toast.failedSaveStorageMode'), 'error');
      setSettings('mount', 'storage_mode', prev);
    }
  };

  const setMountToggle = async (key: 'overlay_preferred' | 'magic_mount_fallback' | 'random_mount_paths' | 'exclude_hosts_modules' | 'restart_framework', value: boolean) => {
    const prev = settings.mount[key];
    setSettings('mount', key, value);
    try {
      await api.configSet(`mount.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key, value: value ? t('activity.on') : t('activity.off') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      setSettings('mount', key, prev);
    }
  };

  const setMountSource = async (value: string) => {
    const prev = settings.mount.mount_source;
    setSettings('mount', 'mount_source', value);
    try {
      await api.configSet('mount.mount_source', value);
      pushActivity('setting_changed', t('activity.settingChanged', { key: 'mount_source', value }));
    } catch (e) {
      showToast(t('toast.failedSaveMountSource'), 'error');
      setSettings('mount', 'mount_source', prev);
    }
  };

  const setOverlaySource = async (value: string) => {
    const prev = settings.mount.overlay_source;
    setSettings('mount', 'overlay_source', value);
    try {
      await api.configSet('mount.overlay_source', value);
      pushActivity('setting_changed', t('activity.settingChanged', { key: 'overlay_source', value }));
    } catch (e) {
      showToast(t('toast.failedSaveOverlaySource'), 'error');
      setSettings('mount', 'overlay_source', prev);
    }
  };

  const setMountField = async (key: 'module_blacklist' | 'ext4_image_size_mb', value: string | number) => {
    const prev = settings.mount[key];
    (setSettings as any)('mount', key, value);
    try {
      await api.configSet(`mount.${key}`, String(value));
      pushActivity('setting_changed', t('activity.settingChanged', { key, value: value || t('activity.empty') }));
    } catch (e) {
      showToast(t('toast.failedSaveKey', { key }), 'error');
      (setSettings as any)('mount', key, prev);
    }
  };

  const setMountStrategy = async (strategy: MountStrategy) => {
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
      pushActivity('mount_strategy_changed', t('activity.strategyChanged', { value: strategy }));
      showToast(t('toast.mountStrategyChanged'), 'warning');
    } catch (e) {
      showToast(t('toast.failedSaveMountStrategy'), 'error');
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

      batch(() => {
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
        setSystemInfo('susfsVersion', status.susfs_version || '');
        setStats({
          activeRules: status.rule_count,
          excludedUids: status.excluded_uid_count,
          hiddenPaths: status.hidden_path_count,
          hiddenMaps: status.hidden_maps_count ?? 0,
        });
      });
    } catch (e) {
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
          lastTriggerTimestamp = newTrigger;
          await refreshApps();
        }
      } catch (e) {
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
      return;
    }
    appFetchInProgress = true;
    setLoading('apps', true);
    try {
      if (shouldUseMock()) {
        const mockApps = await api.getInstalledApps();
        setInstalledApps(mockApps);
        return;
      }

      const apps = await fetchAppsViaKsuApi();
      setInstalledApps(apps);

      // Initialize lastKnownPackages for change detection polling
      lastKnownPackages = new Set(apps.map(a => a.packageName));
      startTriggerPolling();
    } catch (err) {
      showToast(t('toast.failedLoadApps'), 'error');
    } finally {
      appFetchInProgress = false;
      setLoading('apps', false);
    }
  };

  const scanKsuModules = async () => {
    setLoading('modules', true);
    try {
      const mods = await api.scanKsuModules();
      setKsuModules(mods);
    } catch (err) {
      showToast(t('toast.failedScanModules'), 'error');
    } finally {
      setLoading('modules', false);
    }
  };

  const loadKsuModule = async (moduleName: string, modulePath: string) => {
    try {
      const count = await api.loadKsuModule(moduleName, modulePath);
      setKsuModules(prev => prev.map(m =>
        m.path === modulePath ? { ...m, isLoaded: true } : m
      ));
      setStats('activeRules', s => s + count);
      showToast(t('toast.moduleLoaded', { moduleName, count }), 'success');
      return count;
    } catch (err) {
      showToast(t('toast.failedLoadModule', { moduleName }), 'error');
      throw err;
    }
  };

  const unloadKsuModule = async (moduleName: string, modulePath: string) => {
    try {
      const count = await api.unloadKsuModule(moduleName, modulePath);
      setKsuModules(prev => prev.map(m =>
        m.path === modulePath ? { ...m, isLoaded: false } : m
      ));
      const status = await api.getRuntimeStatus();
      if (status) {
        setModuleStatuses(status.modules);
        setFontModules(status.font_modules || []);
      }
      setStats('activeRules', s => Math.max(0, s - count));
      showToast(t('toast.moduleUnloaded', { moduleName, count }), 'success');
      return count;
    } catch (err) {
      showToast(t('toast.failedUnloadModule', { moduleName }), 'error');
      throw err;
    }
  };

  const setLanguage = async (code: string) => {
    await loadLocale(code);
    localStorage.setItem('zeromount-language', code);
    try {
      await api.configSet('ui.language', code);
    } catch {
      showToast(t('toast.failedSaveKey', { key: 'ui.language' }), 'error');
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
    susfsMode: () => capabilities()?.susfs_mode || 'absent',
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
    externalSusfsModule,
    susfsOwnership,
    bridgeValues,
    refreshBridgeValues,

    loadInitialData,
    loadInstalledApps,
    loadRuntimeStatus,
    loadBreneSettings,
    setBreneToggle,
    setBreneNumeric,
    setSusfsToggle,
    setPerfToggle,
    setEmojiToggle,
    setAdbToggle,
    setGuardToggle,
    guardStatus,
    guardAllowModule,
    guardDisallowModule,
    guardClearLockout,
    emojiConflict,
    setUnameMode,
    setUnameField,
    loadMountSettings,
    setMountStrategy,
    setMountStorageMode,
    setMountToggle,
    setMountField,
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
    verboseDumpPath,
    showToast,
    setLanguage,
    stopPolling: stopTriggerPolling,
  };
}

export const store = createRoot(createAppStore);
