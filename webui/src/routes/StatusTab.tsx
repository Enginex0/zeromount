import { createSignal, createMemo, For, Show, onMount } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Skeleton } from '../components/core/Skeleton';
import { ScenarioIndicator } from '../components/core/ScenarioIndicator';
import { Badge } from '../components/core/Badge';
import { store } from '../lib/store';
import type { MountStrategy } from '../lib/types';
import './StatusTab.css';

export function StatusTab() {
  const [animatedActiveRules, setAnimatedActiveRules] = createSignal(0);
  const [animatedExcludedUids, setAnimatedExcludedUids] = createSignal(0);
  const [animatedPaths, setAnimatedPaths] = createSignal(0);
  const [animatedMaps, setAnimatedMaps] = createSignal(0);
  const [showAllActivity, setShowAllActivity] = createSignal(false);
  const [showAllModules, setShowAllModules] = createSignal(false);

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
      case 'vfs': return 'VFS Redirection';
      case 'overlay': return 'OverlayFS';
      case 'magicmount': return 'Magic Mount';
      case 'susfs_only': return 'SUSFS Only';
    }
  });

  const mountModeValue = createMemo(() => {
    const mode = effectiveMode();
    if (mode === 'susfs_only') return 'No Mount';

    const statuses = store.moduleStatuses();
    if (statuses.some(m => m.strategy !== 'Font')) return 'Active';

    const s = store.scenario?.() || 'none';
    if (s === 'none') return 'Unavailable';
    return 'Selected';
  });

  const mountModeColor = createMemo(() => {
    const theme = store.currentTheme();
    switch (effectiveMode()) {
      case 'vfs': return theme.colorSuccess;
      case 'overlay': return theme.colorInfo || '#3b82f6';
      case 'magicmount': return theme.colorInfo || '#3b82f6';
      case 'susfs_only': return '#FF8E53';
    }
  });

  const mountModeDescription = createMemo(() => {
    const mode = effectiveMode();
    const storage = store.settings.mount.storage_mode;
    switch (mode) {
      case 'vfs': return 'Kernel VFS driver handling filesystem redirection';
      case 'overlay': return `OverlayFS stacked filesystem \u00b7 Storage: ${storage}`;
      case 'magicmount': return `Bind mounts \u00b7 Storage: ${storage}`;
      case 'susfs_only': return 'SUSFS hiding active, no mount redirection';
    }
  });

  const isVfsMode = createMemo(() => effectiveMode() === 'vfs');

  // VFS uses ioctl engine state; mount-based modes are active when modules are mounted
  const isModuleActive = createMemo(() => {
    if (isVfsMode()) return store.engineActive();
    return store.moduleStatuses().some(m => m.strategy !== 'Font');
  });

  const heroStatusLabel = createMemo(() => {
    if (isVfsMode()) return store.engineActive() ? 'Engine Active' : 'Engine Inactive';
    return isModuleActive() ? 'Mounts Active' : 'No Modules Loaded';
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
    if (seconds < 60) return 'Just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return `${minutes} min ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.floor(hours / 24)} days ago`;
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
        <Card variant="gradient-border" padding="large" class={isModuleActive() ? 'status-hero-card status-hero-card--active' : 'status-hero-card'}>
          <div class="status-hero">
            <div
              class="status-hero__indicator"
              style={{
                color: isModuleActive()
                  ? store.currentTheme().textOnAccent
                  : store.currentTheme().textTertiary
              }}
            >
              <span
                class={`status-hero__dot ${isModuleActive() ? 'status-hero__dot--active' : ''}`}
                style={{
                  background: isModuleActive() ? store.currentTheme().textOnAccent : store.currentTheme().textTertiary,
                  'box-shadow': isModuleActive() ? `0 0 12px rgba(${store.currentTheme().accentRgb}, 0.5)` : 'none'
                }}
              />
              {heroStatusLabel()}
            </div>

            <ScenarioIndicator />

            <div
              class={`status-hero__shield ${isModuleActive() ? 'status-hero__shield--active' : ''}`}
            >
              <div
                class={`status-hero__glow ${isModuleActive() ? 'status-hero__glow--active' : 'status-hero__glow--inactive'}`}
                style={{
                  background: isModuleActive()
                    ? `radial-gradient(circle, ${store.currentTheme().colorSuccessGlow} 0%, transparent 60%)`
                    : 'transparent'
                }}
              />

              <svg
                width="100"
                height="100"
                viewBox="0 0 24 24"
                class="status-hero__shield-svg"
              >
                <path
                  fill={isModuleActive() ? store.currentTheme().textOnAccent : store.currentTheme().textTertiary}
                  opacity={isModuleActive() ? 0.9 : 0.5}
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
                {store.engineActive() ? 'DISABLE ENGINE' : 'ENABLE ENGINE'}
              </Button>
            </Show>
          </div>
        </Card>
      </Show>

      {/* Quick Stats */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          Quick Stats
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
                Active Rules
              </div>
            </div>

            <div class="status-stats__card bg-surface">
              <div class="status-stats__value">
                {animatedPaths()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                Paths Hidden
              </div>
            </div>

            <div class="status-stats__card bg-surface">
              <div class="status-stats__value">
                {animatedMaps()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                Maps Hidden
              </div>
            </div>
          </div>
        </Show>
      </Card>

      {/* Mode Statistics */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          Mode Statistics
        </h3>
        <div class="status-mode">
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: mountModeColor() }}
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
                Strategy
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {displayStrategy()}
            </span>
          </div>
          <div class="status-mode__reboot-hint color-text-tertiary">
            Switching mode requires reboot
          </div>
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: store.systemInfo.susfsVersion && store.systemInfo.susfsVersion !== 'N/A'
                  ? store.currentTheme().colorSuccess
                  : store.currentTheme().colorError }}
              />
              <span class="status-mode__text color-text-primary">
                SUSFS
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {store.systemInfo.susfsVersion && store.systemInfo.susfsVersion !== 'N/A'
                ? `${store.systemInfo.susfsVersion} (${
                    store.capabilities?.()?.susfs_kstat_redirect && store.capabilities?.()?.susfs_open_redirect_all
                      ? 'Extended' : 'Stock'
                  })`
                : 'Unavailable'}
            </span>
          </div>
        </div>

        <Show when={store.moduleStatuses().length > 0}>
          <div class="status-mode__modules">
            <div class="status-mode__modules-header">
              <div class="status-mode__modules-label color-text-tertiary">
                Per-Module Strategy
              </div>
              <Show when={store.moduleStatuses().length > 3}>
                <button
                  onClick={() => setShowAllModules(!showAllModules())}
                  class="status-activity__toggle"
                  style={{ color: store.currentTheme().textAccent }}
                >
                  {showAllModules() ? 'Show Less' : `+${store.moduleStatuses().length - 3} more`}
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
                      {mod.rules_applied} {mod.strategy === 'Font' ? 'files' : 'rules'}
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
          Mount Info
        </h3>
        <div class="status-mount__cards">
          <div class="status-mount__card bg-surface">
            <div class="status-mount__card-label color-text-tertiary">
              Modules
            </div>
            <div class="status-mount__card-value color-text-primary">
              {loadedModulesCount()} active
            </div>
          </div>
          <div class="status-mount__card bg-surface">
            <div class="status-mount__card-label color-text-tertiary">
              Source
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
        <div class="status-mount__paths-label color-text-tertiary">
          Redirected Paths
        </div>
        <Show
          when={pathChips().length > 0}
          fallback={
            <div class="status-mount__empty color-text-tertiary">
              No paths redirected
            </div>
          }
        >
          <div class="status-mount__paths">
            <For each={pathChips()}>
              {(path) => (
                <div class="status-mount__chip bg-surface color-text-primary">
                  {path}
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>

      {/* System Health */}
      <Card>
        <h3 class="status-section__header color-text-secondary">
          System Health
        </h3>
        <div class="status-health">
          <div>
            <div class="status-health__item-header">
              <span
                class="status-health__level"
                style={{ color: store.currentTheme().colorInfo || '#3b82f6' }}
              >
                INFO
              </span>
              <span class="status-health__title color-text-primary">
                SUSFS
              </span>
            </div>
            <div class="status-health__message color-text-secondary">
              {store.settings.susfs.enabled
                ? (store.capabilities?.()?.susfs_kstat_redirect && store.capabilities?.()?.susfs_open_redirect_all
                    ? 'Extended kernel — all features available'
                    : 'Stock kernel — custom commands unavailable')
                : store.systemInfo.susfsVersion && store.systemInfo.susfsVersion !== 'N/A'
                  ? 'SUSFS available but disabled'
                  : 'Running without SUSFS'}
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
                    { key: 'open_redirect', active: caps.susfs_open_redirect },
                    { key: 'kstat_redirect', active: caps.susfs_kstat_redirect },
                    { key: 'open_redirect_all', active: caps.susfs_open_redirect_all },
                  ];
                  return (
                    <For each={features}>
                      {(f) => (
                        <span class={`status-health__feat-chip ${f.active ? 'status-health__feat-chip--active' : 'status-health__feat-chip--missing'}`}>
                          {f.active ? '\u2713' : '\u2717'} {f.key}
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
                  WARNING
                </span>
                <span class="status-health__title color-text-primary">
                  Degraded
                </span>
              </div>
              <div class="status-health__message color-text-secondary">
                {store.degradationReason() || 'System running in degraded mode'}
              </div>
            </div>
          </Show>
          <Show when={store.rules().length === 0}>
            <div>
              <div class="status-health__item-header">
                <span
                  class="status-health__level"
                  style={{ color: store.currentTheme().colorWarning }}
                >
                  WARNING
                </span>
                <span class="status-health__title color-text-primary">
                  Rules
                </span>
              </div>
              <div class="status-health__message status-health__message--warning color-text-secondary">
                No redirection rules configured
              </div>
            </div>
          </Show>
        </div>
      </Card>

      {/* Recent Activity */}
      <Card>
        <div class="status-activity__header">
          <h3 class="status-section__header status-activity__title color-text-secondary">
            Recent Activity
          </h3>
          <button
            onClick={() => setShowAllActivity(!showAllActivity())}
            class="status-activity__toggle color-text-accent"
          >
            {showAllActivity() ? 'Show Less' : 'View All'}
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
          System Info
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
              <span class="status-info__label color-text-tertiary">Device:</span>
              <span class="color-text-primary">{store.systemInfo.deviceModel}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">Android:</span>
              <span class="color-text-primary">{store.systemInfo.androidVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">SELinux:</span>
              <span class="color-text-accent">{store.systemInfo.selinuxStatus}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">Kernel:</span>
              <span class="color-text-primary">{store.systemInfo.kernelVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">Driver:</span>
              <span class="color-text-accent">{store.systemInfo.driverVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">SUSFS:</span>
              <span class="color-text-accent">{store.systemInfo.susfsVersion}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">Root:</span>
              <span class="color-text-accent">{store.rootManager() ?? 'Unknown'}</span>
            </div>
            <div>
              <span class="status-info__label color-text-tertiary">misc:</span>
              <span class="color-text-primary">/dev/zeromount</span>
            </div>
          </div>
        </Show>
      </Card>
    </div>
  );
}
