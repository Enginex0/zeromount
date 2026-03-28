import { createSignal, createMemo, onMount, Show, For } from 'solid-js';
import { Card } from '../core/Card';
import { Toggle } from '../core/Toggle';
import { CollapsibleSubgroup } from '../ui/CollapsibleSubgroup';
import { store } from '../../lib/store';
import { api } from '../../lib/api';
import { runShell } from '../../lib/ksuApi';
import { t } from '../../lib/i18n';
import './GuardSection.css';

interface ModuleEntry {
  name: string;
  disabled: boolean;
  locked: boolean;
}

export function GuardSection() {
  const [expanded, setExpanded] = createSignal(false);
  const [modules, setModules] = createSignal<ModuleEntry[]>([]);
  const gs = () => store.guardStatus();
  const allowed = createMemo(() => new Set(gs().allowedModules));

  const fetchModules = async () => {
    const { errno, stdout } = await runShell(
      'for d in /data/adb/modules/*/; do [ -d "$d" ] || continue; n=$(basename "$d"); [ -f "$d/disable" ] && echo "1:$n" || echo "0:$n"; done'
    );
    if (errno !== 0) return;
    const entries: ModuleEntry[] = stdout.trim().split('\n').filter(Boolean).map(line => {
      const disabled = line.startsWith('1:');
      const name = line.substring(2);
      return { name, disabled, locked: name === 'meta-zeromount' };
    });
    if (!entries.some(e => e.name === 'meta-zeromount')) {
      entries.unshift({ name: 'meta-zeromount', disabled: false, locked: true });
    }
    entries.sort((a, b) => {
      if (a.locked !== b.locked) return a.locked ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    setModules(entries);
  };

  onMount(fetchModules);

  const protectedCount = createMemo(() =>
    modules().filter(m => allowed().has(m.name)).length
  );

  const handleCheck = (name: string, checked: boolean) => {
    if (checked) store.guardAllowModule(name);
    else store.guardDisallowModule(name);
  };

  const markerChipClass = (count: number) =>
    count > 0 ? 'guard__chip guard__chip--warn' : 'guard__chip guard__chip--ok';

  return (
    <Card>
      <h3 class="settings__section-title">
        <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm-2 16l-4-4 1.41-1.41L10 14.17l6.59-6.59L18 9l-8 8z"/>
        </svg>
        {t('guard.title')}
      </h3>

      <div
        class="guard__header"
        onClick={(e) => {
          if ((e.target as HTMLElement).closest('.toggle')) return;
          setExpanded(!expanded());
        }}
      >
        <div class="guard__title-row">
          <div class="guard__title-text">
            <div class="settings__item-label">{t('guard.label')}</div>
            <div class="settings__item-desc">{t('guard.desc')}</div>
          </div>
        </div>
        <div class="guard__controls">
          <Toggle
            checked={gs().enabled}
            onChange={(v) => store.setGuardToggle('enabled', v)}
          />
          <svg
            class={`guard__chevron${expanded() ? ' guard__chevron--open' : ''}`}
            viewBox="0 0 24 24"
            fill="currentColor"
          >
            <path d="M7 10l5 5 5-5z"/>
          </svg>
        </div>
      </div>

      <Show when={expanded()}>
        <div class="guard__body">
          <Show when={gs().recoveryLockout}>
            <div class="guard__lockout-banner">
              <span>{t('guard.lockoutActive')}</span>
              <button class="guard__lockout-clear" onClick={() => store.guardClearLockout()}>
                {t('guard.clearLockout')}
              </button>
            </div>
          </Show>

          <div class="guard__status-row">
            <span class={`guard__chip ${gs().disabled ? 'guard__chip--warn' : 'guard__chip--ok'}`}>
              {gs().disabled ? t('guard.moduleDisabled') : t('guard.moduleActive')}
            </span>
            <span class="guard__chip guard__chip--ok">
              {t('guard.bootcount', { count: gs().bootcount })}
            </span>
          </div>

          <Show when={gs().lastRecovery}>
            <div class="guard__recovery settings__item-desc">
              {t('guard.lastRecovery', { date: gs().lastRecovery! })}
            </div>
          </Show>

          <div class="settings__sub-toggles">
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">{t('guard.systemuiMonitor')}</div>
                <div class="settings__item-desc">{t('guard.systemuiMonitorDesc')}</div>
              </div>
              <Toggle
                checked={store.settings.guard.systemui_monitor_enabled}
                onChange={(v) => store.setGuardToggle('systemui_monitor_enabled', v)}
              />
            </div>
          </div>

          <CollapsibleSubgroup
            label={t('guard.protectedModules', { protected: protectedCount(), total: modules().length })}
            hiddenCount={modules().length}
            defaultItems={<></>}
            expandedItems={
              <div class="guard__module-list">
                <For each={modules()}>
                  {(mod) => {
                    const checked = () => mod.locked || allowed().has(mod.name);
                    return (
                      <div
                        class={`guard__check-row${mod.locked ? ' guard__check-row--locked' : ''}`}
                        onClick={() => !mod.locked && handleCheck(mod.name, !checked())}
                      >
                        <div class={`guard__checkbox${checked() ? ' guard__checkbox--on' : ''}`}>
                          <Show when={checked()}>
                            <svg viewBox="0 0 24 24" fill="currentColor"><path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/></svg>
                          </Show>
                        </div>
                        <div class="guard__check-label">
                          <span>{mod.name}</span>
                          {mod.locked && <span class="guard__tag guard__tag--locked">{t('guard.tagAlways')}</span>}
                          {mod.disabled && <span class="guard__tag guard__tag--disabled">{t('guard.tagDisabled')}</span>}
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            }
          />

          <CollapsibleSubgroup
            label={t('guard.thresholds')}
            hiddenCount={4}
            defaultItems={<></>}
            expandedItems={
              <div class="guard__threshold-list">
                <ThresholdRow label={t('guard.bootTimeout')} configKey="guard.boot_timeout_secs" value={store.settings.guard.boot_timeout_secs} />
                <ThresholdRow label={t('guard.zygoteMaxRestarts')} configKey="guard.zygote_max_restarts" value={store.settings.guard.zygote_max_restarts} />
                <ThresholdRow label={t('guard.systemuiMaxRestarts')} configKey="guard.systemui_max_restarts" value={store.settings.guard.systemui_max_restarts} />
                <ThresholdRow label={t('guard.systemuiAbsentTimeout')} configKey="guard.systemui_absent_timeout_secs" value={store.settings.guard.systemui_absent_timeout_secs} />
              </div>
            }
          />
        </div>
      </Show>
    </Card>
  );
}

function ThresholdRow(props: { label: string; configKey: string; value: number }) {
  const handleBlur = async (e: FocusEvent & { currentTarget: HTMLInputElement }) => {
    const val = parseInt(e.currentTarget.value, 10);
    if (isNaN(val) || val === props.value) return;
    try {
      await api.configSet(props.configKey, String(val));
      store.showToast(`${props.label} → ${val}`, 'success');
    } catch {
      store.showToast(t('toast.failedSaveKey', { key: props.label }), 'error');
    }
  };

  return (
    <div class="guard__threshold-row">
      <span class="settings__item-desc">{props.label}</span>
      <input
        type="number"
        value={props.value}
        onBlur={handleBlur}
        class="guard__threshold-input"
      />
    </div>
  );
}
