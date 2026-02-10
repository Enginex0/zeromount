import { createSignal, createEffect, createMemo, For, Show, onMount, onCleanup } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Skeleton } from '../components/core/Skeleton';
import { ScenarioIndicator } from '../components/core/ScenarioIndicator';
import { Badge } from '../components/core/Badge';
import { store } from '../lib/store';
import type { MountStrategy } from '../lib/types';
import './StatusTab.css';

export function StatusTab() {
  const [pulseScale, setPulseScale] = createSignal(1);
  const [animatedActiveRules, setAnimatedActiveRules] = createSignal(0);
  const [animatedExcludedUids, setAnimatedExcludedUids] = createSignal(0);
  const [showAllActivity, setShowAllActivity] = createSignal(false);

  // Effective mount mode: user preference gated by detected capabilities
  const effectiveMode = createMemo(() => {
    const s = store.scenario?.() || 'none';
    const strategy = store.activeStrategy();

    if (s === 'susfs_only') return 'susfs_only' as const;

    switch (strategy) {
      case 'Vfs': {
        const caps = store.capabilities();
        if (caps?.vfs_driver) return 'vfs' as const;
        return caps?.overlay_supported ? 'overlay' as const : 'magicmount' as const;
      }
      case 'Overlay': return 'overlay' as const;
      case 'MagicMount': return 'magicmount' as const;
    }
  });

  const mountModeLabel = createMemo(() => {
    switch (effectiveMode()) {
      case 'vfs': return 'VFS Redirection';
      case 'overlay': return 'OverlayFS';
      case 'magicmount': return 'Magic Mount';
      case 'susfs_only': return 'SUSFS Only';
      case 'none': return 'Magic Mount';
    }
  });

  const mountModeValue = createMemo(() => {
    const s = store.scenario?.() || 'none';
    const mode = effectiveMode();
    if (mode === 'vfs' && (s === 'full' || s === 'kernel_only')) return 'Active';
    if (mode === 'susfs_only') return 'No Mount';
    if (mode === 'none') return 'Default';
    if (mode === 'vfs' && !store.capabilities()?.vfs_driver) return 'Unavailable';
    return 'Selected';
  });

  const mountModeColor = createMemo(() => {
    const t = store.currentTheme();
    switch (effectiveMode()) {
      case 'vfs': return t.colorSuccess;
      case 'overlay': return t.colorInfo || '#3b82f6';
      case 'magicmount': return t.colorWarning;
      case 'susfs_only': return '#FF8E53';
      case 'none': return t.colorSuccess;
    }
  });

  const mountModeDescription = createMemo(() => {
    const mode = effectiveMode();
    const storage = store.settings.mount.storage_mode;
    switch (mode) {
      case 'vfs': return 'Kernel VFS driver handling filesystem redirection';
      case 'overlay': return `OverlayFS stacked filesystem \u00b7 Storage: ${storage}`;
      case 'magicmount': return `Bind mounts (KSU default) \u00b7 Storage: ${storage}`;
      case 'susfs_only': return 'SUSFS hiding active, no mount redirection';
      case 'none': return 'KSU default bind mounts, no additional drivers';
    }
  });

  createEffect(() => {
    if (store.engineActive()) {
      const interval = setInterval(() => {
        setPulseScale(1.03);
        setTimeout(() => setPulseScale(1), 150);
      }, 3000);
      onCleanup(() => clearInterval(interval));
    }
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
    const t = store.currentTheme();
    switch (type) {
      case 'rule_added':
      case 'rule_removed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.colorSuccess}>
            <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
          </svg>
        );
      case 'uid_excluded':
      case 'uid_included':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.colorError}>
            <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z"/>
          </svg>
        );
      case 'setting_changed':
      case 'brene_toggle':
      case 'susfs_toggle':
      case 'mount_strategy_changed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.colorInfo || '#3b82f6'}>
            <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
          </svg>
        );
      case 'theme_changed':
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.textAccent}>
            <path d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8z"/>
          </svg>
        );
      default:
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.colorWarning}>
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        );
    }
  };

  const strategyColor = (s: MountStrategy) => {
    switch (s) {
      case 'Vfs': return store.currentTheme().colorSuccess;
      case 'Overlay': return store.currentTheme().colorInfo || '#3b82f6';
      case 'MagicMount': return store.currentTheme().colorWarning;
    }
  };

  const pathChips = createMemo(() => {
    const prefixes = new Set<string>();
    store.rules().forEach(rule => {
      const parts = rule.target.split('/').filter(Boolean);
      if (parts.length > 0) {
        const prefix = '/' + parts[0];
        if (prefix !== '/[BLOCKED]') prefixes.add(prefix);
      }
    });
    return Array.from(prefixes).sort();
  });

  const loadedModulesCount = createMemo(() => store.ksuModules().filter(m => m.isLoaded).length);

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
        <Card variant="gradient-border" padding="large" style={store.engineActive() ? 'animation: glowPulse 3s ease-in-out infinite;' : ''}>
          <div class="status-hero">
            <div
              class="status-hero__indicator"
              style={{
                color: store.engineActive()
                  ? store.currentTheme().textOnAccent
                  : store.currentTheme().textTertiary
              }}
            >
              <span
                class={`status-hero__dot ${store.engineActive() ? 'status-hero__dot--active' : ''}`}
                style={{
                  background: store.engineActive() ? store.currentTheme().textOnAccent : store.currentTheme().textTertiary,
                  'box-shadow': store.engineActive() ? `0 0 12px rgba(${store.currentTheme().accentRgb}, 0.5)` : 'none'
                }}
              />
              {store.engineActive() ? 'Engine Active' : 'Engine Inactive'}
            </div>

            <ScenarioIndicator />

            <div
              class={`status-hero__shield ${store.engineActive() ? 'status-hero__shield--active' : ''}`}
              style={{ transform: `scale(${pulseScale()})` }}
            >
              <div
                class={`status-hero__glow ${store.engineActive() ? 'status-hero__glow--active' : 'status-hero__glow--inactive'}`}
                style={{
                  background: store.engineActive()
                    ? `radial-gradient(circle, ${store.currentTheme().colorSuccessGlow} 0%, transparent 70%)`
                    : 'transparent'
                }}
              />

              <svg
                width="100"
                height="100"
                viewBox="0 0 24 24"
                class="status-hero__shield-svg"
                style={{
                  filter: store.engineActive() ? `drop-shadow(0 0 20px ${store.currentTheme().colorSuccessGlow})` : 'none'
                }}
              >
                <path
                  fill={store.engineActive() ? store.currentTheme().textOnAccent : store.currentTheme().textTertiary}
                  opacity={store.engineActive() ? 0.9 : 0.5}
                  d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"
                />
              </svg>
            </div>

            <Button
              size="large"
              onClick={() => store.toggleEngine()}
              loading={store.loading.engine}
              style={`min-width: 200px; ${!store.engineActive() ? 'opacity: 0.8; filter: grayscale(20%);' : ''}`}
            >
              {store.engineActive() ? 'DISABLE ENGINE' : 'ENABLE ENGINE'}
            </Button>
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
                {animatedExcludedUids()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                Excluded Apps
              </div>
            </div>

            <div class="status-stats__card bg-surface">
              <div class="status-stats__value status-stats__value--small">
                {store.systemInfo.uptime || '—'}
              </div>
              <div class="status-stats__label color-text-tertiary">
                Uptime
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
                style={{ background: strategyColor(store.activeStrategy()) }}
              />
              <span class="status-mode__text color-text-primary">
                Strategy
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {store.activeStrategy()}
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
                ? store.systemInfo.susfsVersion
                : 'Unavailable'}
            </span>
          </div>
        </div>

        <Show when={store.moduleStatuses().length > 0}>
          <div class="status-mode__modules">
            <div class="status-mode__modules-label color-text-tertiary">
              Per-Module Strategy
            </div>
            <For each={store.moduleStatuses()}>
              {(mod) => (
                <div class="status-mode__module-row">
                  <span class="status-mode__module-name color-text-primary">{mod.id}</span>
                  <div class="status-mode__module-meta">
                    <Badge variant={mod.strategy === 'Vfs' ? 'success' : mod.strategy === 'Overlay' ? 'info' : 'default'}>
                      {mod.strategy}
                    </Badge>
                    <span class="status-mode__module-rules color-text-tertiary">
                      {mod.rules_applied} rules
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
              {store.rootManager() ?? 'Unknown'}
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
                System
              </span>
            </div>
            <div class="status-health__message color-text-secondary">
              SUSFS integration active
            </div>
          </div>
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
          <For each={showAllActivity() ? store.activity() : store.activity().slice(0, 3)}>
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
