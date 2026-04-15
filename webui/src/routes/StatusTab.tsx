import { createSignal, createMemo, For, Show, onMount } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Skeleton } from '../components/core/Skeleton';
import { ScenarioIndicator } from '../components/core/ScenarioIndicator';
import { Badge } from '../components/core/Badge';
import { CollapsibleSubgroup } from '../components/ui/CollapsibleSubgroup';
import { GuardSection } from '../components/settings/GuardSection';
import { ModuleExclusionsSection } from '../components/settings/ModuleExclusionsSection';
import { store } from '../lib/store';
import { t } from '../lib/i18n';
import type { MountStrategy } from '../lib/types';
import './StatusTab.css';

const MODE_COLOR_POOL = [
  '#00BCD4', '#9C27B0', '#009688', '#4CAF50',
  '#FFC107', '#7C4DFF', '#FF6D00', '#26A69A',
];

function pickModeColor(accentRgb: string): string {
  const [ar, ag, ab] = accentRgb.split(',').map(s => parseInt(s.trim()));
  const far = MODE_COLOR_POOL.filter(hex => {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return Math.sqrt((r - ar) ** 2 + (g - ag) ** 2 + (b - ab) ** 2) > 120;
  });
  const pool = far.length ? far : MODE_COLOR_POOL;
  return pool[Math.floor(Math.random() * pool.length)];
}

export function StatusTab() {
  const [animatedActiveRules, setAnimatedActiveRules] = createSignal(0);
  const [animatedExcludedUids, setAnimatedExcludedUids] = createSignal(0);
  const [animatedPaths, setAnimatedPaths] = createSignal(0);
  const [animatedMaps, setAnimatedMaps] = createSignal(0);
  const [showAllActivity, setShowAllActivity] = createSignal(false);
  const [showAllModules, setShowAllModules] = createSignal(false);
  const [showAllPaths, setShowAllPaths] = createSignal(false);

  const displayStrategy = createMemo(() => {
    return store.runtimeStrategy() || store.effectiveStrategy();
  });

  const effectiveMode = createMemo(() => {
    const s = store.scenario?.() || 'none';
    if (s === 'susfs_only') return 'susfs_only' as const;

    // Ground truth from last boot — what the pipeline actually executed
    const runtime = store.runtimeStrategy();
    if (runtime) {
      switch (runtime) {
        case 'Vfs': return 'vfs' as const;
        case 'Overlay': return 'overlay' as const;
        case 'MagicMount': return 'magicmount' as const;
      }
    }

    // No runtime data — derive from capabilities-aware effective strategy
    switch (store.effectiveStrategy()) {
      case 'Vfs': return 'vfs' as const;
      case 'Overlay': return 'overlay' as const;
      case 'MagicMount': return 'magicmount' as const;
      default: return 'magicmount' as const;
    }
  });

  const mountModeLabel = createMemo(() => {
    switch (effectiveMode()) {
      case 'vfs': return t('status.modeVfs');
      case 'overlay': return t('status.modeOverlay');
      case 'magicmount': return t('status.modeMagicMount');
      case 'susfs_only': return t('status.modeSusfsOnly');
    }
  });

  const mountModeValue = createMemo(() => {
    const mode = effectiveMode();
    if (mode === 'susfs_only') return t('status.modeValueNoMount');

    const statuses = store.moduleStatuses();
    if (statuses.some(m => m.strategy !== 'Font')) return t('status.modeValueActive');

    const caps = store.capabilities?.();
    const modeSupported = mode === 'overlay' ? caps?.overlay_supported
      : mode === 'vfs' ? caps?.vfs_driver
      : true;

    if (!modeSupported) return t('status.modeValueUnavailable');
    return t('status.modeValueStandby');
  });

  const mountModeDescription = createMemo(() => {
    const mode = effectiveMode();
    const storage = store.settings.mount.storage_mode;
    switch (mode) {
      case 'vfs': return t('status.modeDescVfs');
      case 'overlay': return t('status.modeDescOverlay', { storage });
      case 'magicmount': return t('status.modeDescMagicMount', { storage });
      case 'susfs_only': return t('status.modeDescSusfsOnly');
    }
  });

  const [modeColor] = createSignal(pickModeColor(store.currentTheme().accentRgb));

  const isVfsMode = createMemo(() => effectiveMode() === 'vfs');

  // VFS uses ioctl engine state; mount-based modes are active when modules are mounted
  const isModuleActive = createMemo(() => {
    if (isVfsMode()) return store.engineActive();
    return store.moduleStatuses().some(m => m.strategy !== 'Font');
  });

  const heroStatusLabel = createMemo(() => {
    if (isVfsMode()) return store.engineActive() ? t('status.engineActive') : t('status.engineInactive');
    return isModuleActive() ? t('status.mountsActive') : t('status.noModulesLoaded');
  });

  onMount(() => {
    const animateNumber = (
      target: number,
      setter: (n: number) => void,
      duration: number = 500
    ) => {
      const start = 0;
      const startTime = performance.now();

      const animate = (currentTime: number) => {
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1);
        const eased = 1 - Math.pow(1 - progress, 3);
        setter(Math.round(start + (target - start) * eased));

        if (progress < 1) {
          requestAnimationFrame(animate);
        }
      };

      requestAnimationFrame(animate);
    };

    setTimeout(() => {
      animateNumber(store.stats.activeRules, setAnimatedActiveRules);
      animateNumber(store.stats.excludedUids, setAnimatedExcludedUids);
      animateNumber(store.stats.hiddenPaths, setAnimatedPaths);
      animateNumber(store.stats.hiddenMaps, setAnimatedMaps);
    }, 300);
  });

  const formatTimeAgo = (date: Date) => {
    const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
    if (seconds < 60) return t('status.timeJustNow');
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return t('status.timeMinAgo', { minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t('status.timeHoursAgo', { hours });
    return t('status.timeDaysAgo', { days: Math.floor(hours / 24) });
  };

  const getActivityIcon = (type: string) => {
    const theme = store.currentTheme();
    switch (type) {
      case 'rule_added':
      case 'rule_removed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={theme.colorSuccess}>
            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
          </svg>
        );
      case 'uid_excluded':
      case 'uid_included':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={theme.colorError}>
            <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z"/>
          </svg>
        );
      case 'setting_changed':
      case 'brene_toggle':
      case 'susfs_toggle':
      case 'mount_strategy_changed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={theme.colorInfo || '#3b82f6'}>
            <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
          </svg>
        );
      case 'theme_changed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={theme.textAccent}>
            <path d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8z"/>
          </svg>
        );
      default:
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={theme.colorWarning}>
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        );
    }
  };

  const strategyColor = (s: MountStrategy) => {
    switch (s) {
      case 'Vfs': return '#4CAF50';
      case 'Overlay': return '#26C6DA';
      case 'MagicMount': return '#FFA726';
      case 'Font': return '#CE93D8';
    }
  };

  const pathChips = createMemo(() => {
    const prefixes = new Set<string>();
    store.rules().forEach(rule => {
      const parts = rule.target.split('/').filter(Boolean);
      if (parts.length >= 2) {
        const prefix = '/' + parts[0] + '/' + parts[1];
        if (!prefix.startsWith('/[BLOCKED]')) prefixes.add(prefix);
      }
    });
    // Overlay mount paths aren't VFS rules — pull from status
    store.moduleStatuses().forEach(mod => {
      mod.mount_paths.forEach(mp => {
        const parts = mp.split('/').filter(Boolean);
        if (parts.length >= 2) prefixes.add('/' + parts[0] + '/' + parts[1]);
      });
    });
    return Array.from(prefixes).sort();
  });

  const loadedModulesCount = createMemo(() => {
    const fontIds = new Set(store.fontModules());
    const statusIds = new Set(store.moduleStatuses().map(s => s.id));
    return store.ksuModules().filter(m => {
      const name = m.path.split('/').pop() || '';
      return m.isLoaded || fontIds.has(name) || statusIds.has(name);
    }).length;
  });

  const isInitialLoad = () => store.loading.status && !store.systemInfo.driverVersion;

  return (
    <div class="status">
      {/* Hero Section - Engine Status */}
      <Show
        when={!isInitialLoad()}
        fallback={
          <Card variant="gradient-border" padding="large">
            <div class="status-hero">
              <Skeleton width="120px" height="24px" borderRadius="12px" />
              <Skeleton width="100px" height="100px" borderRadius="50%" class="status-hero__skeleton-shield" />
              <Skeleton width="200px" height="48px" borderRadius="24px" />
            </div>
          </Card>
        }
      >
        <Card variant="gradient-border" padding="large" class="status-hero-card status-hero-card--active">
          <div class="status-hero">
            <div
              class="status-hero__indicator"
              style={{ color: store.currentTheme().textOnAccent }}
            >
              <span
                class="status-hero__dot status-hero__dot--active"
                style={{
                  background: store.currentTheme().textOnAccent,
                  'box-shadow': `0 0 12px rgba(${store.currentTheme().accentRgb}, 0.5)`
                }}
              />
              {heroStatusLabel()}
            </div>

            <ScenarioIndicator color={modeColor()} />

            <div class="status-hero__shield status-hero__shield--active">
              <div
                class="status-hero__glow status-hero__glow--active"
                style={{
                  background: `radial-gradient(circle, ${modeColor()}40 0%, transparent 60%)`
                }}
              />

              <svg
                width="100"
                height="100"
                viewBox="0 0 24 24"
                class="status-hero__shield-svg"
              >
                <path
                  fill={store.currentTheme().textOnAccent}
                  opacity={0.9}
                  d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"
                />
              </svg>
            </div>

            <Show when={isVfsMode()}>
              <Button
                variant={store.engineActive() ? 'primary' : 'secondary'}
                size="large"
                onClick={() => store.toggleEngine()}
                loading={store.loading.engine}
                style="min-width: 200px;"
              >
                {store.engineActive() ? t('status.disableEngine') : t('status.enableEngine')}
              </Button>
            </Show>
          </div>
        </Card>
      </Show>

      {/* Quick Stats */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          {t('status.quickStats')}
        </h3>

        <Show
          when={!isInitialLoad()}
          fallback={
            <div class="status-stats">
              <div class="status-stats__card bg-surface">
                <Skeleton width="48px" height="32px" borderRadius="8px" />
                <Skeleton width="72px" height="14px" borderRadius="6px" />
              </div>
              <div class="status-stats__card bg-surface">
                <Skeleton width="48px" height="32px" borderRadius="8px" />
                <Skeleton width="80px" height="14px" borderRadius="6px" />
              </div>
              <div class="status-stats__card bg-surface">
                <Skeleton width="48px" height="32px" borderRadius="8px" />
                <Skeleton width="64px" height="14px" borderRadius="6px" />
              </div>
            </div>
          }
        >
          <div class="status-stats">
            <div class="status-stats__card bg-surface">
              <div class="status-stats__value">
                {animatedActiveRules()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                {t('status.activeRules')}
              </div>
            </div>

            <div class="status-stats__card bg-surface">
              <div class="status-stats__value">
                {animatedPaths()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                {t('status.pathsHidden')}
              </div>
            </div>

            <div class="status-stats__card bg-surface">
              <div class="status-stats__value">
                {animatedMaps()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                {t('status.mapsHidden')}
              </div>
            </div>
          </div>
        </Show>
      </Card>

      {/* Mode Statistics */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          {t('status.modeStatistics')}
        </h3>
        <div class="status-mode">
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: modeColor() }}
              />
              <span class="status-mode__text color-text-primary">
                {mountModeLabel()}
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {mountModeValue()}
            </span>
          </div>
          <div class="status-mode__desc color-text-tertiary">
            {mountModeDescription()}
          </div>
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: strategyColor(displayStrategy()) }}
              />
              <span class="status-mode__text color-text-primary">
                {t('status.strategy')}
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {displayStrategy()}
            </span>
          </div>
          <div class="status-mode__reboot-hint color-text-tertiary">
            {t('status.switchingModeRequiresReboot')}
          </div>
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: store.capabilities?.()?.susfs_available
                  ? store.currentTheme().colorSuccess
                  : store.currentTheme().colorError }}
              />
              <span class="status-mode__text color-text-primary">
                {t('status.susfs')}
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {store.capabilities?.()?.susfs_available
                ? `${(store.capabilities?.()?.susfs_version ?? store.systemInfo.susfsVersion) || '?'} (${
                    store.capabilities?.()?.susfs_kstat_redirect
                      ? t('status.susfsExtended') : t('status.susfsStock')
                  })`
                : t('status.susfsUnavailable')}
            </span>
          </div>
        </div>

        <Show when={store.moduleStatuses().length > 0}>
          <div class="status-mode__modules">
            <div class="status-mode__modules-header">
              <div class="status-mode__modules-label color-text-tertiary">
                {t('status.perModuleStrategy')}
              </div>
              <Show when={store.moduleStatuses().length > 3}>
                <button
                  onClick={() => setShowAllModules(!showAllModules())}
                  class="status-activity__toggle"
                  style={{ color: store.currentTheme().textAccent }}
                >
                  {showAllModules() ? t('status.showLess') : t('status.showMore', { count: store.moduleStatuses().length - 3 })}
                  <svg class={`status-activity__chevron${showAllModules() ? ' status-activity__chevron--open' : ''}`} width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
                </button>
              </Show>
            </div>
            <For each={store.moduleStatuses().slice(0, showAllModules() ? undefined : 3)}>
              {(mod) => (
                <div class="status-mode__module-row">
                  <span class="status-mode__module-name color-text-primary">{mod.id}</span>
                  <div class="status-mode__module-meta">
                    <span
                      class="badge badge--medium"
                      style={{
                        background: strategyColor(mod.strategy),
                        'box-shadow': `0 0 12px ${strategyColor(mod.strategy)}40`,
                      }}
                    >
                      {mod.strategy}
                    </span>
                    <span class="status-mode__module-rules color-text-tertiary">
                      {mod.rules_applied} {mod.strategy === 'Font' ? t('status.filesUnit') : t('status.rulesUnit')}
                    </span>
                  </div>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>

      {/* Mount Info */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          {t('status.mountInfo')}
        </h3>
        <div class="status-mount__cards">
          <div class="status-mount__card bg-surface">
            <div class="status-mount__card-label color-text-tertiary">
              {t('status.modules')}
            </div>
            <div class="status-mount__card-value color-text-primary">
              {t('status.modulesActive', { count: loadedModulesCount() })}
            </div>
          </div>
          <div class="status-mount__card bg-surface">
            <div class="status-mount__card-label color-text-tertiary">
              {t('status.source')}
            </div>
            <div class="status-mount__card-value color-text-accent">
              {(() => {
                const runtime = store.mountSource();
                if (runtime) return runtime;
                const strategy = store.effectiveStrategy();
                if (strategy === 'Vfs') return 'VFS';
                if (strategy === 'MagicMount') return 'KSU';
                const cfg = store.settings.mount.overlay_source;
                return (cfg && cfg !== 'auto') ? cfg : 'Overlay';
              })()}
            </div>
          </div>
        </div>
        <div class="status-mount__paths-header">
          <div class="status-mount__paths-label color-text-tertiary">
            {t('status.redirectedPaths')}
          </div>
          <Show when={pathChips().length > 5}>
            <button
              onClick={() => setShowAllPaths(!showAllPaths())}
              class="status-activity__toggle"
              style={{ color: store.currentTheme().textAccent }}
            >
              {showAllPaths() ? t('status.showLess') : t('status.showMore', { count: pathChips().length - 5 })}
              <svg class={`status-activity__chevron${showAllPaths() ? ' status-activity__chevron--open' : ''}`} width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
            </button>
          </Show>
        </div>
        <Show
          when={pathChips().length > 0}
          fallback={
            <div class="status-mount__empty color-text-tertiary">
              {t('status.noPathsRedirected')}
            </div>
          }
        >
          <div class="status-mount__paths">
            <For each={pathChips().slice(0, showAllPaths() ? undefined : 5)}>
              {(path) => (
                <div class="status-mount__chip bg-surface color-text-primary">
                  {path}
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>

      <Show when={(() => {
        const c = store.capabilities?.();
        return c && (!c.vfs_driver || !c.susfs_available || !c.overlay_supported || !c.susfs_kstat_redirect);
      })()}>
        <Card>
          <h3 class="status-section__header color-text-secondary">
            {t('capabilities.title')}
          </h3>
          <div class="settings__item-desc color-text-tertiary" style={{ "margin-bottom": "12px" }}>
            {t('capabilities.desc')}
          </div>
          <CollapsibleSubgroup
            label={t('capabilities.expandLabel')}
            hiddenCount={(() => {
              const c = store.capabilities?.()!;
              let n = 0;
              if (!c.vfs_driver) n++;
              if (!c.susfs_available) n++;
              if (!c.overlay_supported) n++;
              if (!c.susfs_kstat_redirect && c.susfs_available) n++;
              return n;
            })()}
            defaultItems={<></>}
            expandedItems={
              <>
                <Show when={!store.capabilities?.()!.vfs_driver}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.vfsDriver')}</div>
                    <div class="settings__item-desc">{t('capabilities.vfsDriverDesc')}</div>
                  </div>
                </Show>
                <Show when={!store.capabilities?.()!.susfs_available}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.susfs')}</div>
                    <div class="settings__item-desc">{t('capabilities.susfsDesc')}</div>
                  </div>
                </Show>
                <Show when={!store.capabilities?.()!.overlay_supported}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.overlayFs')}</div>
                    <div class="settings__item-desc">{t('capabilities.overlayFsDesc')}</div>
                  </div>
                </Show>
                <Show when={store.capabilities?.()!.susfs_available && !store.capabilities?.()!.susfs_kstat_redirect}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.kstatRedirect')}</div>
                    <div class="settings__item-desc">{t('capabilities.kstatRedirectDesc')}</div>
                  </div>
                </Show>
              </>
            }
          />
        </Card>
      </Show>

      {/* System Health */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          {t('status.engineStatus')}
        </h3>
        <div class="status-health">
          <div>
            <div class="status-health__item-header">
              <span
                class="status-health__level"
                style={{ color: store.currentTheme().colorInfo || '#3b82f6' }}
              >
                {t('status.levelInfo')}
              </span>
              <span class="status-health__title color-text-primary">
                {t('status.susfs')}
              </span>
            </div>
            <div class="status-health__message color-text-secondary">
              {store.settings.susfs.enabled
                ? (store.capabilities?.()?.susfs_kstat_redirect
                    ? t('status.healthSusfsExtendedAll')
                    : t('status.healthSusfsActive'))
                : store.systemInfo.susfsVersion && store.systemInfo.susfsVersion !== 'N/A'
                  ? t('status.healthSusfsAvailableDisabled')
                  : t('status.healthSusfsNotDetected')}
            </div>
            <Show when={store.settings.susfs.enabled && store.capabilities?.()}>
              <div class="status-health__features">
                {(() => {
                  const caps = store.capabilities?.();
                  if (!caps) return null;
                  const features = [
                    { key: 'vfs_driver', active: caps.vfs_driver },
                    { key: 'kstat', active: caps.susfs_kstat },
                    { key: 'path', active: caps.susfs_path },
                    { key: 'maps', active: caps.susfs_maps },
                    { key: 'kstat_redirect', active: caps.susfs_kstat_redirect },
                  ].filter(f => f.active);
                  return (
                    <For each={features}>
                      {(f) => (
                        <span class="status-health__feat-chip status-health__feat-chip--active">
                          {'\u2713'} {f.key}
                        </span>
                      )}
                    </For>
                  );
                })()}
              </div>
            </Show>
          </div>
          <Show when={store.degraded()}>
            <div>
              <div class="status-health__item-header">
                <span
                  class="status-health__level"
                  style={{ color: store.currentTheme().colorWarning }}
                >
                  {t('status.levelWarning')}
                </span>
                <span class="status-health__title color-text-primary">
                  {t('status.healthRuleFailures')}
                </span>
              </div>
              <div class="status-health__message status-health__message--warning color-text-secondary">
                {store.degradationReason()}
              </div>
            </div>
          </Show>
          <Show when={store.rules().length === 0}>
            <div>
              <div class="status-health__item-header">
                <span
                  class="status-health__level"
                  style={{ color: store.currentTheme().colorInfo || '#3b82f6' }}
                >
                  {t('status.levelInfo')}
                </span>
                <span class="status-health__title color-text-primary">
                  {t('status.healthRules')}
                </span>
              </div>
              <div class="status-health__message color-text-secondary">
                {t('status.healthNoRulesHint')}
              </div>
            </div>
          </Show>
        </div>
      </Card>

      <GuardSection />

      <ModuleExclusionsSection />

      {/* Recent Activity */}
      <Card>
        <div class="status-activity__header">
          <h3 class="status-section__header status-activity__title color-text-secondary">
            {t('status.recentActivity')}
          </h3>
          <button
            onClick={() => setShowAllActivity(!showAllActivity())}
            class="status-activity__toggle color-text-accent"
          >
            {showAllActivity() ? t('status.showLess') : t('status.viewAll')}
            <svg class={`status-activity__chevron${showAllActivity() ? ' status-activity__chevron--open' : ''}`} width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
          </button>
        </div>

        <div class="status-activity__list">
          <For each={store.activity().slice(0, showAllActivity() ? 10 : 3)}>
            {(item, index) => (
              <div
                class="status-activity__item bg-surface"
                style={{ animation: `slideInRight 0.3s ease-out ${index() * 0.1}s both` }}
              >
                <div class="status-activity__icon">{getActivityIcon(item.type)}</div>
                <div class="status-activity__content">
                  <div class="status-activity__message color-text-primary">
                    {item.message}
                  </div>
                  <div class="status-activity__time color-text-tertiary">
                    {formatTimeAgo(item.timestamp)}
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </Card>

      {/* System Info */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          {t('status.systemInfo')}
        </h3>

        <Show
          when={!isInitialLoad()}
          fallback={
            <div class="status-info__grid">
              <div><Skeleton width="140px" height="16px" /></div>
              <div><Skeleton width="100px" height="16px" /></div>
              <div><Skeleton width="90px" height="16px" /></div>
              <div><Skeleton width="160px" height="16px" /></div>
              <div><Skeleton width="80px" height="16px" /></div>
              <div><Skeleton width="70px" height="16px" /></div>
              <div><Skeleton width="100px" height="16px" /></div>
            </div>
          }
        >
          <div class="status-info__grid">
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoDevice')}</span>
              <span class="color-text-primary">{store.systemInfo.deviceModel}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoAndroid')}</span>
              <span class="color-text-primary">{store.systemInfo.androidVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoSelinux')}</span>
              <span class="color-text-accent">{store.systemInfo.selinuxStatus}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoKernel')}</span>
              <span class="color-text-primary">{store.systemInfo.kernelVersion}</span>
            </div>
            <Show when={store.systemInfo.driverVersion}>
              <div>
                <span class="status-info__label color-text-tertiary">{t('status.infoDriver')}</span>
                <span class="color-text-accent">{store.systemInfo.driverVersion}</span>
              </div>
            </Show>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoEngine')}</span>
              <span class="color-text-accent">
                {(() => {
                  const s = store.runtimeStrategy() || store.effectiveStrategy();
                  return s === 'Vfs' ? t('status.modeVfs') : s === 'Overlay' ? t('status.modeOverlay') : s === 'MagicMount' ? t('status.modeMagicMount') : t('engine.storageAuto');
                })()}
              </span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoSusfs')}</span>
              <span class="color-text-accent">{store.systemInfo.susfsVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">{t('status.infoRoot')}</span>
              <span class="color-text-accent">{store.rootManager() ?? t('status.infoRootUnknown')}</span>
            </div>
            <Show when={store.engineActive()}>
              <div>
                <span class="status-info__label color-text-tertiary">{t('status.infoMisc')}</span>
                <span class="color-text-primary">{t('status.infoMiscValue')}</span>
              </div>
            </Show>
          </div>
        </Show>
      </Card>
    </div>
  );
}
