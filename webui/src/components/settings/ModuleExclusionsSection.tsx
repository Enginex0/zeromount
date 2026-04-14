import { createSignal, createMemo, Show, For, onMount } from 'solid-js';
import { Card } from '../core/Card';
import { store } from '../../lib/store';
import { api } from '../../lib/api';
import { t } from '../../lib/i18n';
import './ModuleExclusionsSection.css';

const MODULE_ID_RE = /^[a-zA-Z][a-zA-Z0-9._-]+$/;

interface ModuleEntry {
  name: string;
  locked: boolean;
}

export function ModuleExclusionsSection() {
  const [expanded, setExpanded] = createSignal(false);
  const [excluded, setExcluded] = createSignal<Set<string>>(new Set());

  onMount(async () => {
    const csv = await api.configGet('mount.module_blacklist');
    if (csv) {
      setExcluded(new Set(csv.split(',').map(s => s.trim()).filter(Boolean)));
    }
  });

  const modules = createMemo<ModuleEntry[]>(() => {
    const entries: ModuleEntry[] = store.ksuModules().map(mod => {
      const name = mod.path.split('/').pop() ?? mod.name;
      return { name, locked: name === 'meta-zeromount' };
    });
    if (!entries.some(e => e.name === 'meta-zeromount')) {
      entries.unshift({ name: 'meta-zeromount', locked: true });
    }
    entries.sort((a, b) => {
      if (a.locked !== b.locked) return a.locked ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    return entries;
  });

  const excludedCount = createMemo(
    () => modules().filter(m => excluded().has(m.name)).length,
  );

  const handleCheck = async (name: string, checked: boolean) => {
    if (!MODULE_ID_RE.test(name) || name.length > 256) {
      store.showToast(t('toast.failedSaveKey', { key: 'mount.module_blacklist' }), 'error');
      return;
    }
    const prev = excluded();
    const next = new Set(prev);
    if (checked) next.add(name);
    else next.delete(name);
    setExcluded(next);
    try {
      await api.configSet('mount.module_blacklist', [...next].join(','));
    } catch {
      setExcluded(prev);
      store.showToast(t('toast.failedSaveKey', { key: 'mount.module_blacklist' }), 'error');
    }
  };

  return (
    <Card>
      <h3 class="settings__section-title">
        <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm0 18a8 8 0 1 1 0-16 8 8 0 0 1 0 16zM8 11h8v2H8z"/>
        </svg>
        {t('exclusion.title')}
      </h3>

      <div class="exclusion__header" onClick={() => setExpanded(!expanded())}>
        <div class="exclusion__title-row">
          <div class="exclusion__title-text">
            <div class="settings__item-label">{t('exclusion.label')}</div>
            <div class="settings__item-desc">{t('exclusion.desc')}</div>
          </div>
        </div>
        <div class="exclusion__controls">
          <span class="exclusion__chip exclusion__chip--ok">
            {excludedCount()}/{modules().length}
          </span>
          <svg
            class={`exclusion__chevron${expanded() ? ' exclusion__chevron--open' : ''}`}
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        </div>
      </div>

      <Show when={expanded()}>
        <div class="exclusion__body">
          <div class="exclusion__module-list">
            <For each={modules()}>
              {(mod) => {
                const checked = () => !mod.locked && excluded().has(mod.name);
                return (
                  <div
                    class={`exclusion__check-row${mod.locked ? ' exclusion__check-row--locked' : ''}`}
                    onClick={() => !mod.locked && handleCheck(mod.name, !checked())}
                  >
                    <div class={`exclusion__checkbox${checked() ? ' exclusion__checkbox--on' : ''}`}>
                      <Show when={checked()}>
                        <svg viewBox="0 0 24 24" fill="currentColor">
                          <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                        </svg>
                      </Show>
                    </div>
                    <div class="exclusion__check-label">
                      <span>{mod.name}</span>
                      {mod.locked && (
                        <span class="exclusion__tag exclusion__tag--self">
                          {t('exclusion.tagSelf')}
                        </span>
                      )}
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </div>
      </Show>
    </Card>
  );
}
