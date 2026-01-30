import { createSignal, For, Show, onMount } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Input } from '../components/core/Input';
import { Badge } from '../components/core/Badge';
import { Skeleton } from '../components/core/Skeleton';
import { store } from '../lib/store';
import type { KsuModule } from '../lib/types';
import "./ModulesTab.css";

const t = () => store.currentTheme();

export function ModulesTab() {
  const [searchQuery, setSearchQuery] = createSignal('');
  const [expandedModule, setExpandedModule] = createSignal<string | null>(null);
  const [loadingModules, setLoadingModules] = createSignal<Set<string>>(new Set());

  onMount(() => {
    if (store.ksuModules().length === 0) {
      store.scanKsuModules();
    }
  });

  const filteredModules = () => {
    const query = searchQuery().toLowerCase();
    if (!query) return store.ksuModules();
    return store.ksuModules().filter(
      (mod) =>
        mod.name.toLowerCase().includes(query) ||
        mod.path.toLowerCase().includes(query)
    );
  };

  const handleToggleModule = async (mod: KsuModule) => {
    setLoadingModules(prev => {
      const loading = new Set(prev);
      loading.add(mod.path);
      return loading;
    });

    try {
      if (mod.isLoaded) {
        await store.unloadKsuModule(mod.name, mod.path);
      } else {
        await store.loadKsuModule(mod.name, mod.path);
      }
    } finally {
      setLoadingModules(prev => {
        const updated = new Set(prev);
        updated.delete(mod.path);
        return updated;
      });
    }
  };

  const isModuleLoading = (path: string) => loadingModules().has(path);

  const getPartitionBadges = (mod: KsuModule) => {
    const badges: string[] = [];
    if (mod.hasSystem) badges.push('system');
    if (mod.hasVendor) badges.push('vendor');
    if (mod.hasProduct) badges.push('product');
    return badges;
  };

  return (
    <div class="modules">
      <div class="modules__search-row">
        <div class="modules__search-wrapper">
          <Input
            placeholder="Search modules..."
            value={searchQuery()}
            onInput={(e) => setSearchQuery(e.currentTarget.value)}
            fullWidth
          />
          <svg
            class="modules__search-icon"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill={t().textTertiary}
          >
            <path d="M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z"/>
          </svg>
        </div>
        <Button
          onClick={() => store.scanKsuModules()}
          loading={store.loading.modules}
          style="white-space: nowrap;"
        >
          SCAN
        </Button>
      </div>

      <Show when={store.loading.modules}>
        <div class="modules__list">
          <For each={[1, 2, 3]}>
            {() => (
              <Card>
                <div class="modules__item-row">
                  <Skeleton width="44px" height="44px" borderRadius="12px" />
                  <div class="modules__skeleton-content">
                    <Skeleton width="140px" height="18px" borderRadius="8px" />
                    <Skeleton width="80px" height="14px" borderRadius="6px" />
                  </div>
                  <Skeleton width="72px" height="24px" borderRadius="12px" />
                </div>
              </Card>
            )}
          </For>
        </div>
      </Show>

      <Show when={!store.loading.modules}>
        <div class="modules__list">
          <For each={filteredModules()}>
            {(mod, index) => (
              <Card
                hoverable
                style={`animation: slideInRight 0.3s ease-out ${index() * 0.05}s both; cursor: pointer;`}
                onClick={() => setExpandedModule(expandedModule() === mod.path ? null : mod.path)}
              >
                <div class="modules__item-row">
                  <div
                    class={`modules__item-icon ${mod.isLoaded ? 'modules__item-icon--active' : 'modules__item-icon--inactive'}`}
                  >
                    <svg
                      width="22"
                      height="22"
                      viewBox="0 0 24 24"
                      fill={mod.isLoaded ? 'white' : t().textTertiary}
                    >
                      <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-7 14l-5-5 1.41-1.41L12 14.17l4.59-4.58L18 11l-6 6z"/>
                    </svg>
                  </div>

                  <div class="modules__item-content">
                    <div class="modules__item-name">
                      {mod.name}
                    </div>
                    <div class="modules__item-meta">
                      {mod.fileCount} files
                    </div>
                  </div>

                  <Badge
                    variant={mod.isLoaded ? 'success' : 'default'}
                    size="small"
                  >
                    {mod.isLoaded ? 'Loaded' : 'Not Loaded'}
                  </Badge>
                </div>

                <Show when={expandedModule() === mod.path}>
                  <div class="modules__details">
                    <div class="modules__details-inner">
                      <div>
                        <div class="modules__detail-label">
                          Path
                        </div>
                        <div class="modules__path-value">
                          {mod.path}
                        </div>
                      </div>

                      <div>
                        <div class="modules__detail-label modules__detail-label--partitions">
                          Partitions
                        </div>
                        <div class="modules__partitions">
                          <For each={getPartitionBadges(mod)}>
                            {(partition) => (
                              <span class="modules__partition-badge">
                                /{partition}
                              </span>
                            )}
                          </For>
                        </div>
                      </div>

                      <div class="modules__stats-row">
                        <div>
                          <span class="modules__stat-label">Files:</span>
                          <span class="modules__stat-value">
                            {mod.fileCount.toLocaleString()}
                          </span>
                        </div>
                        <div>
                          <span class="modules__stat-label">Status:</span>
                          <span
                            class={`modules__stat-value ${mod.isLoaded ? 'modules__stat-value--active' : 'modules__stat-value--inactive'}`}
                          >
                            {mod.isLoaded ? 'Active' : 'Inactive'}
                          </span>
                        </div>
                      </div>

                      <Button
                        variant={mod.isLoaded ? 'danger' : 'primary'}
                        size="small"
                        loading={isModuleLoading(mod.path)}
                        onClick={(e) => {
                          e.stopPropagation();
                          handleToggleModule(mod);
                        }}
                        style="margin-top: 8px;"
                      >
                        {mod.isLoaded ? 'HOT UNLOAD' : 'HOT LOAD'}
                      </Button>
                    </div>
                  </div>
                </Show>
              </Card>
            )}
          </For>

          <Show when={filteredModules().length === 0 && !store.loading.modules}>
            <div class="modules__empty">
              <svg class="modules__empty-icon" width="48" height="48" viewBox="0 0 24 24" fill="currentColor">
                <path d="M19 3H5c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2zm-7 14l-5-5 1.41-1.41L12 14.17l4.59-4.58L18 11l-6 6z"/>
              </svg>
              <div class="modules__empty-title">
                {searchQuery() ? 'No modules match your search' : 'No modules with system overlays'}
              </div>
              <div class="modules__empty-subtitle">
                {searchQuery() ? 'Try a different search term' : 'Install KernelSU modules with system/vendor/product directories'}
              </div>
            </div>
          </Show>
        </div>
      </Show>
    </div>
  );
}
