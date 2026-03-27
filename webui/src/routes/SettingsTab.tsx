import { createSignal, createMemo, Show, For } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Toggle } from '../components/core/Toggle';
import { Modal } from '../components/layout/Modal';
import { CollapsibleSubgroup } from '../components/ui/CollapsibleSubgroup';
import { AppearanceSection } from '../components/settings/AppearanceSection';
import { MountEngineSection } from '../components/settings/MountEngineSection';
import { SusfsSection } from '../components/settings/SusfsSection';
import { AboutSection } from '../components/settings/AboutSection';
import { store } from '../lib/store';
import { runShell } from '../lib/ksuApi';
import { t } from '../lib/i18n';
import type { BreneSettings } from '../lib/types';
import "./SettingsTab.css";

export function SettingsTab() {
  const [showClearConfirm, setShowClearConfirm] = createSignal(false);

  const caps = () => store.capabilities?.() || null;

  const handleClearAll = async () => {
    await store.clearAllRules();
    setShowClearConfirm(false);
  };

  const handleBreneToggle = (key: keyof BreneSettings, value: boolean) => {
    store.setBreneToggle(key, value);
  };

  return (
    <div class="settings">
      <AppearanceSection />

      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
          </svg>
          {t('settings.engine')}
        </h3>

        <div class="settings__engine-controls">
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">{t('settings.verboseLogging')}</div>
              <div class="settings__item-desc">{t('settings.verboseLoggingDesc')}</div>
            </div>
            <Toggle
              checked={store.settings.verboseLogging}
              onChange={async (checked) => {
                await store.setVerboseLogging(checked);
              }}
            />
          </div>
          {store.settings.verboseLogging && (
            <div class="settings__item-desc color-text-tertiary">
              {store.verboseDumpPath()
                ? t('settings.verboseLogsAt', { path: store.verboseDumpPath()! })
                : t('settings.verboseModeActive')}
            </div>
          )}

          <Button
            variant="danger"
            fullWidth
            disabled={!caps()?.vfs_driver}
            onClick={() => setShowClearConfirm(true)}
          >
            {caps()?.vfs_driver ? t('settings.clearAllRules') : t('settings.vfsUnavailable')}
          </Button>
        </div>
      </Card>

      <MountEngineSection />

      <Show when={(() => {
        const c = caps();
        return c && (!c.vfs_driver || !c.susfs_available || !c.overlay_supported || !c.susfs_kstat_redirect);
      })()}>
        <Card>
          <h3 class="settings__section-title">
            <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
            </svg>
            {t('capabilities.title')}
          </h3>
          <div class="settings__item-desc color-text-tertiary" style={{ "margin-bottom": "12px" }}>
            {t('capabilities.desc')}
          </div>
          <CollapsibleSubgroup
            label={t('capabilities.expandLabel')}
            hiddenCount={(() => {
              const c = caps()!;
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
                <Show when={!caps()!.vfs_driver}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.vfsDriver')}</div>
                    <div class="settings__item-desc">{t('capabilities.vfsDriverDesc')}</div>
                  </div>
                </Show>
                <Show when={!caps()!.susfs_available}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.susfs')}</div>
                    <div class="settings__item-desc">{t('capabilities.susfsDesc')}</div>
                  </div>
                </Show>
                <Show when={!caps()!.overlay_supported}>
                  <div class="settings__item settings__item--stacked">
                    <div class="settings__item-label">{t('capabilities.overlayFs')}</div>
                    <div class="settings__item-desc">{t('capabilities.overlayFsDesc')}</div>
                  </div>
                </Show>
                <Show when={caps()!.susfs_available && !caps()!.susfs_kstat_redirect}>
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

      <SusfsSection />

      {/* Property Spoofing — uses resetprop, independent of SUSFS */}
      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zM13 9V3.5L18.5 9H13z"/>
          </svg>
          {t('settings.propertySpoofing')}
        </h3>
        <div class="settings__item-desc settings__prop-note color-text-tertiary">
          {t('settings.propNote')}
        </div>
        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">{t('settings.buildProperties')}</div>
            <div class="settings__item-desc">{t('settings.buildPropertiesDesc')}</div>
          </div>
          <Toggle checked={store.settings.brene.prop_spoofing} onChange={(v) => handleBreneToggle('prop_spoofing', v)} />
        </div>
        <CollapsibleSubgroup
          label={t('settings.androidSettings')}
          hiddenCount={3}
          defaultItems={<></>}
          expandedItems={<>
            <div class="settings__item">
              <div class="settings__item-content">
                <div class="settings__item-label">{t('settings.developerOptions')}</div>
                <div class="settings__item-desc">{t('settings.developerOptionsDesc')}</div>
              </div>
              <Toggle
                checked={store.settings.adb.developer_options}
                onChange={async (v) => {
                  store.setAdbToggle('developer_options', v);
                  await runShell(`settings put global development_settings_enabled ${v ? 1 : 0}`);
                }}
              />
            </div>
            <div class="settings__item">
              <div class="settings__item-content">
                <div class="settings__item-label">{t('settings.usbDebugging')}</div>
                <div class="settings__item-desc">{t('settings.developerOptionsDesc')}</div>
              </div>
              <Toggle
                checked={store.settings.adb.usb_debugging}
                onChange={async (v) => {
                  store.setAdbToggle('usb_debugging', v);
                  await runShell(`settings put global adb_enabled ${v ? 1 : 0}`);
                }}
              />
            </div>
            <div class="settings__item">
              <div class="settings__item-content">
                <div class="settings__item-label">{t('settings.adbRoot')}</div>
                <div class="settings__item-desc">{t('settings.adbRootDesc')}</div>
              </div>
              <Toggle
                checked={store.settings.adb.adb_root}
                onChange={(v) => store.setAdbToggle('adb_root', v)}
              />
            </div>
          </>}
        />
      </Card>

      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M20.38 8.57l-1.23 1.85a8 8 0 0 1-.22 7.58H5.07A8 8 0 0 1 15.58 6.85l1.85-1.23A10 10 0 0 0 3.35 19a2 2 0 0 0 1.72 1h13.85a2 2 0 0 0 1.74-1 10 10 0 0 0-.27-10.44z"/>
            <path d="M10.59 15.41a2 2 0 0 0 2.83 0l5.66-8.49-8.49 5.66a2 2 0 0 0 0 2.83z"/>
          </svg>
          {t('settings.performance')}
        </h3>
        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">{t('settings.perfTweak')}</div>
            <div class="settings__item-desc">{t('settings.perfTweakDesc')}</div>
          </div>
          <Toggle
            checked={store.settings.perf.enabled}
            onChange={(v) => store.setPerfToggle('enabled', v)}
          />
        </div>
      </Card>

      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <circle cx="8.5" cy="9.5" r="1.5"/>
            <circle cx="15.5" cy="9.5" r="1.5"/>
            <path d="M12 18c-3.31 0-6-2.69-6-6h1.5c0 2.76 2.24 5 4.5 5s4.5-2.24 4.5-5H18c0 3.31-2.69 6-6 6z"/>
            <path d="M11.99 2C6.47 2 2 6.48 2 12s4.47 10 9.99 10C17.52 22 22 17.52 22 12S17.52 2 11.99 2zM12 20c-4.42 0-8-3.58-8-8s3.58-8 8-8 8 3.58 8 8-3.58 8-8 8z"/>
          </svg>
          {t('settings.emoji')}
        </h3>
        <div class={`settings__item${store.emojiConflict() ? ' settings__item--disabled' : ''}`}>
          <div class="settings__item-content">
            <div class="settings__item-label">{t('settings.facebookEmojis')}</div>
            <div class="settings__item-desc">
              {store.emojiConflict()
                ? t('settings.emojiConflict', { module: store.emojiConflict()! })
                : t('settings.facebookEmojisDesc')}
            </div>
          </div>
          <Toggle
            checked={store.settings.emoji.enabled}
            onChange={(v) => store.setEmojiToggle('enabled', v)}
            disabled={!!store.emojiConflict()}
          />
        </div>
      </Card>

      <AboutSection />

      <Modal
        open={showClearConfirm()}
        onClose={() => setShowClearConfirm(false)}
        title={t('modal.clearAllRulesTitle')}
      >
        <div class="settings__modal-content">
          <p class="settings__modal-text">
            {t('modal.clearAllRulesText', { count: store.stats.activeRules })}
          </p>

          <div class="settings__modal-actions">
            <Button
              variant="ghost"
              onClick={() => setShowClearConfirm(false)}
            >
              {t('modal.cancel')}
            </Button>
            <Button
              variant="danger"
              onClick={handleClearAll}
              loading={store.loading.rules}
            >
              {t('modal.clearAll')}
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
