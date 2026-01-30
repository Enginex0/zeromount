import { createSignal, createRoot, createMemo, createEffect } from 'solid-js';
import { createStore } from 'solid-js/store';
import type { Tab, VfsRule, ExcludedUid, ActivityItem, EngineStats, SystemInfo, Settings, MountedModule, InstalledApp, KsuModule } from './types';
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
    hitsToday: 0,
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
  const [modules, setModules] = createSignal<MountedModule[]>([]);
  const [installedApps, setInstalledApps] = createSignal<InstalledApp[]>([]);
  const [ksuModules, setKsuModules] = createSignal<KsuModule[]>([]);

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

  const [settings, setSettings] = createStore<Settings>({
    theme: (savedTheme || 'amoled') as 'dark' | 'light' | 'auto' | 'amoled',
    accentColor: initialAccent,
    autoAccentColor: savedAutoAccent,
    animationsEnabled: true,
    autoStartOnBoot: true,
    verboseLogging: false,
    fixedNav: savedFixedNav,
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
      // All calls in parallel - removed redundant getStats() which re-called getRules/getExcludedUids
      const results = await Promise.allSettled([
        api.getRules(),
        api.getExcludedUids(),
        api.getActivity(),
        api.getSystemInfo(),
        api.getModules(),
        api.isEngineActive(),
        api.scanKsuModules(),
        settings.autoAccentColor ? api.fetchSystemColor() : Promise.resolve(null),
      ]);

      const rulesData = results[0].status === 'fulfilled' ? results[0].value : [];
      const uidsData = results[1].status === 'fulfilled' ? results[1].value : [];
      const activityData = results[2].status === 'fulfilled' ? results[2].value : [];
      const sysInfo = results[3].status === 'fulfilled' ? results[3].value : { driverVersion: '', kernelVersion: '', susfsVersion: '', uptime: '', deviceModel: '', androidVersion: '', selinuxStatus: '' };
      const modulesData = results[4].status === 'fulfilled' ? results[4].value : [];
      const isActive = results[5].status === 'fulfilled' ? results[5].value : false;
      const ksuModulesData = results[6].status === 'fulfilled' ? results[6].value : [];
      const systemColor = results[7].status === 'fulfilled' ? results[7].value : null;

      // Derive stats locally instead of redundant API call
      const statsData = {
        activeRules: rulesData.length,
        excludedUids: uidsData.length,
        hitsToday: rulesData.reduce((sum, r) => sum + r.hits, 0),
      };

      const failedCount = results.filter(r => r.status === 'rejected').length;
      if (failedCount > 0) {
        console.warn('[ZM-Store] loadInitialData() partial failure:', failedCount, 'APIs failed');
      }

      console.log('[ZM-Store] loadInitialData() API results:', {
        rules: rulesData.length,
        uids: uidsData.length,
        activity: activityData.length,
        stats: statsData,
        sysInfo,
        modules: modulesData.length,
        isActive
      });

      setRules(rulesData);
      setExcludedUids(uidsData);
      setActivity(activityData);
      setStats(statsData);
      setSystemInfo(sysInfo);
      setModules(modulesData);
      setEngineActive(isActive);
      setKsuModules(ksuModulesData);
      if (systemColor) setSettings({ accentColor: systemColor });
      console.log('[ZM-Store] loadInitialData() state updated successfully');
    } catch (err) {
      console.error('[ZM-Store] loadInitialData() error:', err);
      showToast('Failed to load data', 'error');
    } finally {
      setLoading({ status: false, rules: false, activity: false });
      console.log('[ZM-Store] loadInitialData() complete');
    }
  };

  const refreshModules = async () => {
    console.log('[ZM-Store] refreshModules() called');
    try {
      const modulesData = await api.getModules();
      setModules(modulesData);
      console.log('[ZM-Store] refreshModules() updated with', modulesData.length, 'modules');
    } catch (e) {
      console.log('[ZM-Store] refreshModules() silent fail:', e);
      // Silent fail for background refresh
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

  const addRule = async (name: string, source: string, target: string) => {
    console.log('[ZM-Store] addRule() called:', { name, source, target });
    setLoading('rules', true);
    try {
      const newRule = await api.addRule(name, source, target);
      setRules(prev => [...prev, newRule]);
      setStats('activeRules', s => s + 1);
      console.log('[ZM-Store] addRule() success, rule id:', newRule.id);
      showToast('Rule created successfully', 'success');
      return newRule;
    } catch (err) {
      console.error('[ZM-Store] addRule() error:', err);
      showToast('Failed to create rule', 'error');
      throw err;
    } finally {
      setLoading('rules', false);
    }
  };

  const deleteRule = async (sourcePath: string) => {
    console.log('[ZM-Store] deleteRule() called:', sourcePath);
    setLoading('rules', true);
    try {
      await api.deleteRule(sourcePath);
      setRules(prev => prev.filter(r => r.source !== sourcePath));
      setStats('activeRules', s => s - 1);
      console.log('[ZM-Store] deleteRule() success');
      showToast('Rule deleted', 'success');
    } catch (err) {
      console.error('[ZM-Store] deleteRule() error:', err);
      showToast('Failed to delete rule', 'error');
    } finally {
      setLoading('rules', false);
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
    modules,
    installedApps,
    ksuModules,
    settings,
    currentTheme,
    toast,

    loadInitialData,
    refreshModules,
    loadInstalledApps,
    scanKsuModules,
    loadKsuModule,
    unloadKsuModule,
    toggleEngine,
    addRule,
    deleteRule,
    excludeUid,
    includeUid,
    clearAllRules,
    updateSettings,
    showToast,
    stopPolling: stopTriggerPolling,
  };
}

console.log('[ZM-Store] Creating store root...');
export const store = createRoot(createAppStore);
console.log('[ZM-Store] Store created successfully');
