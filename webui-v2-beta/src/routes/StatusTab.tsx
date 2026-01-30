import { createSignal, createEffect, createMemo, For, Show, onMount, onCleanup } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Skeleton } from '../components/core/Skeleton';
import { theme, needsDarkText } from '../lib/theme';
import { store } from '../lib/store';
import './StatusTab.css';

export function StatusTab() {
  const [pulseScale, setPulseScale] = createSignal(1);
  const [animatedActiveRules, setAnimatedActiveRules] = createSignal(0);
  const [animatedExcludedUids, setAnimatedExcludedUids] = createSignal(0);
  const [animatedHitsToday, setAnimatedHitsToday] = createSignal(0);
  const [showAllActivity, setShowAllActivity] = createSignal(false);

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
      animateNumber(store.stats.hitsToday, setAnimatedHitsToday);
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
      default:
        return (
          <svg width="20" height="20" viewBox="0 0 24 24" fill={t.colorWarning}>
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        );
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

  const loadedModulesCount = createMemo(() => store.rules().length > 0 ? 1 : 0);

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
                  ? (needsDarkText(store.settings.accentColor) ? '#1A1A2E' : store.currentTheme().colorSuccess)
                  : store.currentTheme().textTertiary
              }}
            >
              <span
                class={`status-hero__dot ${store.engineActive() ? 'status-hero__dot--active' : ''}`}
                style={{
                  background: store.engineActive() ? store.currentTheme().colorSuccess : store.currentTheme().textTertiary,
                  'box-shadow': store.engineActive() ? `0 0 12px ${store.currentTheme().colorSuccessGlow}` : 'none'
                }}
              />
              {store.engineActive() ? 'Engine Active' : 'Engine Inactive'}
            </div>

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
                <defs>
                  <linearGradient id="shieldGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                    <stop offset="0%" style={`stop-color: ${store.engineActive() ? '#E8B4A0' : store.currentTheme().textTertiary}`} />
                    <stop offset="50%" style={`stop-color: ${store.engineActive() ? '#D4A574' : store.currentTheme().textTertiary}`} />
                    <stop offset="100%" style={`stop-color: ${store.engineActive() ? '#C9B896' : store.currentTheme().textTertiary}`} />
                  </linearGradient>
                </defs>
                <path
                  fill="url(#shieldGradient)"
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
              <div class="status-stats__value">
                {animatedHitsToday()}
              </div>
              <div class="status-stats__label color-text-tertiary">
                Hits Today
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
                style={{ background: store.currentTheme().colorSuccess }}
              />
              <span class="status-mode__text color-text-primary">
                VFS Redirection
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {store.rules().length}
            </span>
          </div>
          <div class="status-mode__row">
            <div class="status-mode__label">
              <span
                class="status-mode__dot"
                style={{ background: store.currentTheme().colorInfo || '#3b82f6' }}
              />
              <span class="status-mode__text color-text-primary">
                SUSFS Available
              </span>
            </div>
            <span class="status-mode__value color-text-accent">
              {store.systemInfo.susfsVersion && store.systemInfo.susfsVersion !== 'N/A'
                ? `Yes (${store.systemInfo.susfsVersion})`
                : 'No'}
            </span>
          </div>
        </div>
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
              KSU
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
              <span class="status-info__label color-text-tertiary">Uptime:</span>
              <span class="color-text-primary">{store.systemInfo.uptime}</span>
            </div>
          </div>
        </Show>
      </Card>
    </div>
  );
}
