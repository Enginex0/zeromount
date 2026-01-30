import { createSignal, createEffect, For, Show, onMount, onCleanup } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Input } from '../components/core/Input';
import { Badge } from '../components/core/Badge';
import { Toggle } from '../components/core/Toggle';
import { Skeleton } from '../components/core/Skeleton';
import { store } from '../lib/store';
import type { InstalledApp } from '../lib/types';
import type {} from '../lib/ksu.d.ts';
import "./ConfigTab.css";

const t = () => store.currentTheme();

const iconCache = new Map<string, string>();
const observers = new Set<IntersectionObserver>();

const loadIcon = (packageName: string, imgEl: HTMLImageElement) => {
  const showFallback = () => {
    imgEl.style.display = 'none';
    const fallback = imgEl.nextElementSibling as HTMLElement;
    if (fallback) fallback.style.display = 'block';
  };

  if (iconCache.has(packageName)) {
    imgEl.src = iconCache.get(packageName)!;
    imgEl.style.opacity = '1';
    return;
  }

  if (typeof globalThis.ksu !== 'undefined' && typeof globalThis.ksu.getPackagesIcons === 'function') {
    try {
      const result = globalThis.ksu.getPackagesIcons(JSON.stringify([packageName]), 100);
      if (result) {
        const parsed = JSON.parse(result);
        if (parsed?.[0]?.icon) {
          iconCache.set(packageName, parsed[0].icon);
          imgEl.src = parsed[0].icon;
          imgEl.style.opacity = '1';
          return;
        }
      }
    } catch (e) {
      console.error('[ZM] Icon error:', packageName, e);
    }
  }
  showFallback();
};

const setupIconObserver = (container: HTMLElement, packageName: string) => {
  const imgEl = container.querySelector('img') as HTMLImageElement;
  if (!imgEl) return;

  const observer = new IntersectionObserver((entries) => {
    entries.forEach(entry => {
      if (entry.isIntersecting) {
        loadIcon(packageName, imgEl);
        observer.unobserve(container);
        observer.disconnect();
        observers.delete(observer);
      }
    });
  }, { rootMargin: '100px', threshold: 0.1 });

  observers.add(observer);
  observer.observe(container);
};

const AppIcon = (props: { packageName: string; size?: number }) => {
  const size = props.size || 40;
  const imgSize = size - 8;
  const fallbackSize = size - 12;
  const sizeClass = size === 40 ? 'config__icon--40' : 'config__icon--36';

  return (
    <div
      class={`config__icon ${sizeClass}`}
      ref={(el) => el && setupIconObserver(el, props.packageName)}
    >
      <img
        width={imgSize}
        height={imgSize}
        class="config__icon-img"
        onError={(e) => {
          (e.target as HTMLImageElement).style.display = 'none';
          (e.target as HTMLImageElement).nextElementSibling?.removeAttribute('style');
        }}
      />
      <svg
        width={fallbackSize}
        height={fallbackSize}
        viewBox="0 0 24 24"
        fill={t().textTertiary}
        class="config__icon-fallback"
      >
        <path d="M17 1.01L7 1c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-1.99-2-1.99zM17 19H7V5h10v14z" />
      </svg>
    </div>
  );
};

export function ConfigTab() {
  const [searchQuery, setSearchQuery] = createSignal('');
  const [debouncedQuery, setDebouncedQuery] = createSignal('');
  const [showSystemApps, setShowSystemApps] = createSignal(false);
  let debounceTimer: number | undefined;

  onMount(() => {
    store.loadInstalledApps();
  });

  createEffect(() => {
    const query = searchQuery();
    clearTimeout(debounceTimer);
    debounceTimer = window.setTimeout(() => {
      setDebouncedQuery(query);
    }, 300);
  });

  onCleanup(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
    store.stopPolling();
    observers.forEach(obs => obs.disconnect());
    observers.clear();
  });

  const excludedUidSet = () => new Set(store.excludedUids().map(e => e.uid));

  const filteredExcluded = () => {
    const query = debouncedQuery().toLowerCase();
    const excluded = store.excludedUids();
    if (!query) return excluded;
    return excluded.filter(
      item =>
        item.appName.toLowerCase().includes(query) ||
        item.packageName.toLowerCase().includes(query) ||
        item.uid.toString().includes(query)
    );
  };

  const filteredApps = () => {
    const query = debouncedQuery().toLowerCase();
    const excluded = excludedUidSet();
    let apps = store.installedApps().filter(app => !excluded.has(app.uid));

    if (!showSystemApps()) {
      apps = apps.filter(app => !app.isSystemApp);
    }

    if (query) {
      apps = apps.filter(
        app =>
          app.appName.toLowerCase().includes(query) ||
          app.packageName.toLowerCase().includes(query) ||
          app.uid.toString().includes(query)
      );
    }

    return apps.sort((a, b) => a.appName.localeCompare(b.appName));
  };

  const handleExcludeApp = async (app: InstalledApp) => {
    await store.excludeUid(app.uid, app.packageName, app.appName);
  };

  return (
    <div class="config">
      <div class="config__toolbar">
        <div class="config__search">
          <Input
            placeholder="Search apps..."
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            fullWidth
          />
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill={t().textTertiary}
            class="config__search-icon"
          >
            <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z" />
          </svg>
        </div>
      </div>

      <Card>
        <div class="config__section-header">
          <h3 class="config__section-title">
            <svg width="18" height="18" viewBox="0 0 24 24" fill={t().colorError}>
              <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4z" />
            </svg>
            Excluded Apps
          </h3>
          <Badge variant="error" size="small">
            {store.excludedUids().length}
          </Badge>
        </div>

        <p class="config__section-desc">
          These apps bypass spoofing and mounts are visible - caution
        </p>

        <Show
          when={filteredExcluded().length > 0}
          fallback={
            <div class="config__empty">
              {debouncedQuery() ? 'No matches' : 'No excluded apps'}
            </div>
          }
        >
          <div class="config__apps">
            <For each={filteredExcluded()}>
              {(item) => (
                <div class="config__app config__app--excluded">
                  <AppIcon packageName={item.packageName} size={40} />
                  <div class="config__app-info">
                    <div class="config__app-name">{item.appName}</div>
                    <div class="config__app-package">{item.packageName}</div>
                  </div>
                  <Button
                    variant="ghost"
                    size="small"
                    onClick={() => store.includeUid(item.uid)}
                    style={`color: ${t().colorError}; padding: 6px 10px;`}
                  >
                    REMOVE
                  </Button>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Card>

      <Card>
        <div class="config__section-header">
          <h3 class="config__section-title">
            <svg width="18" height="18" viewBox="0 0 24 24" fill={t().textAccent}>
              <path d="M4 8h4V4H4v4zm6 12h4v-4h-4v4zm-6 0h4v-4H4v4zm0-6h4v-4H4v4zm6 0h4v-4h-4v4zm6-10v4h4V4h-4zm-6 4h4V4h-4v4zm6 6h4v-4h-4v4zm0 6h4v-4h-4v4z" />
            </svg>
            {showSystemApps() ? 'All Apps' : 'User Apps'}
          </h3>
          <Badge variant="info" size="small">
            {filteredApps().length}
          </Badge>
        </div>
        <div class="config__system-toggle">
          <span class="config__system-label">System Apps</span>
          <Toggle checked={showSystemApps()} onChange={setShowSystemApps} />
        </div>

        <Show
          when={!store.loading.apps}
          fallback={
            <div class="config__apps">
              <For each={[1, 2, 3, 4, 5]}>
                {() => (
                  <div class="config__app config__app--skeleton">
                    <Skeleton width="36px" height="36px" borderRadius="10px" />
                    <div class="config__skeleton-info">
                      <Skeleton width="120px" height="16px" borderRadius="6px" />
                      <Skeleton width="180px" height="12px" borderRadius="4px" />
                    </div>
                    <Skeleton width="64px" height="28px" borderRadius="14px" />
                  </div>
                )}
              </For>
            </div>
          }
        >
          <Show
            when={filteredApps().length > 0}
            fallback={
              <div class="config__empty">
                {debouncedQuery() ? 'No matches' : showSystemApps() ? 'No apps found' : 'No user apps found'}
              </div>
            }
          >
            <div class="config__apps config__apps--scrollable">
              <For each={filteredApps()}>
                {(app) => (
                  <div class="config__app config__app--available">
                    <AppIcon packageName={app.packageName} size={36} />
                    <div class="config__app-info">
                      <div class="config__app-name config__app-name--with-badge">
                        {app.appName}
                        <Show when={app.isSystemApp}>
                          <span class="config__sys-badge">SYS</span>
                        </Show>
                      </div>
                      <div class="config__app-package config__app-package--small">
                        {app.packageName}
                      </div>
                    </div>
                    <Button
                      variant="ghost"
                      size="small"
                      onClick={() => handleExcludeApp(app)}
                      style="padding: 6px 10px;"
                    >
                      EXCLUDE
                    </Button>
                  </div>
                )}
              </For>
            </div>
          </Show>
        </Show>
      </Card>
    </div>
  );
}
