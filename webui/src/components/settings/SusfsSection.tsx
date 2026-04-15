import { createSignal, Show } from 'solid-js';
import { Card } from '../core/Card';
import { Toggle } from '../core/Toggle';
import { CollapsibleSubgroup } from '../ui/CollapsibleSubgroup';
import { UnameSheet } from '../ui/UnameSheet';
import { store } from '../../lib/store';
import { t } from '../../lib/i18n';
import type { BreneSettings, SusfsSettings } from '../../lib/types';

export function SusfsSection() {
  const [showAdvanced, setShowAdvanced] = createSignal(false);
  const [showUnameSheet, setShowUnameSheet] = createSignal(false);

  const caps = () => store.capabilities?.() || null;
  const susfsAvailable = () => caps()?.susfs_available ?? false;
  const susfsEnabled = () => susfsAvailable() && store.settings.susfs.enabled;
  const ownership = () => store.susfsOwnership();
  const susfsDisabled = () => ownership() !== 'embedded_active';
  const externalModule = () => store.externalSusfsModule();

  const susfsItemClass = () => {
    const o = ownership();
    return o === 'disabled' ? ' settings__item--susfs-unavailable'
      : o === 'deferred_external' ? ' settings__item--susfs-deferred'
      : '';
  };

  const handleBreneToggle = (key: keyof BreneSettings, value: boolean) => {
    store.setBreneToggle(key, value);
  };

  const handleSusfsToggle = (key: keyof SusfsSettings, value: boolean) => {
    store.setSusfsToggle(key, value);
  };

  return (
    <Card>
      <h3 class="settings__section-title">
        <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"/>
        </svg>
        {t('susfs.title')}
      </h3>

      <div class="settings__item">
        <div class="settings__item-content">
          <div class="settings__item-label">{t('susfs.title')}</div>
          <div class="settings__item-desc">
            {susfsAvailable()
              ? t('susfs.integrationDescAvailable', { version: caps()?.susfs_version || 'unknown', status: susfsEnabled() ? 'active' : 'disabled' })
              : t('susfs.integrationDescNotDetected')}
          </div>
        </div>
        <Toggle
          checked={susfsEnabled()}
          onChange={(v) => {
            handleSusfsToggle('enabled', v);
            const ext = externalModule();
            if (ext) {
              store.showToast(
                v ? t('susfs.toastTakingOwnership') : t('susfs.toastDeferred', { module: ext }),
                'info'
              );
            }
          }}
          disabled={!susfsAvailable()}
          variant="rainbow"
        />
      </div>

      <Show when={susfsAvailable()}>
        <div class="settings__sub-toggles">
          <div class={`settings__item settings__item--sub${susfsDisabled() ? ' settings__item--disabled' : ''}${susfsItemClass()}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">{t('susfs.hideSusMounts')}</div>
              <div class="settings__item-desc">{t('susfs.hideSusMountsDesc')}</div>
            </div>
            <Toggle checked={store.settings.brene.hide_sus_mounts} onChange={(v) => handleBreneToggle('hide_sus_mounts', v)} disabled={!susfsEnabled()} />
          </div>
          <Show when={store.settings.brene.hide_sus_mounts}>
            <div class={`settings__item settings__item--sub settings__item--nested${!susfsEnabled() ? ' settings__item--disabled' : ''}`}>
              <div class="settings__item-content">
                <div class="settings__item-label">{t('susfs.keepMountsAfterBoot')}</div>
                <div class="settings__item-desc">{t('susfs.keepMountsAfterBootDesc')}</div>
              </div>
              <Toggle checked={!store.settings.brene.hide_sus_mounts_off_after_boot} onChange={(v) => handleBreneToggle('hide_sus_mounts_off_after_boot', !v)} disabled={!susfsEnabled()} />
            </div>
          </Show>
          <div class={`settings__item settings__item--sub${!susfsEnabled() ? ' settings__item--disabled' : ''}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">{t('susfs.pathHiding')}</div>
              <div class="settings__item-desc">{t('susfs.pathHidingDesc')}</div>
            </div>
            <Toggle checked={store.settings.susfs.path_hide} onChange={(v) => handleSusfsToggle('path_hide', v)} disabled={susfsDisabled()} />
          </div>
          <div class={`settings__item settings__item--sub${susfsDisabled() ? ' settings__item--disabled' : ''}${susfsItemClass()}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">{t('susfs.kstatSpoofing')}</div>
              <div class="settings__item-desc">{t('susfs.kstatSpoofingDesc')}</div>
            </div>
            <Toggle checked={store.settings.susfs.kstat} onChange={(v) => handleSusfsToggle('kstat', v)} disabled={susfsDisabled()} />
          </div>
          <div class={`settings__item settings__item--sub${susfsDisabled() ? ' settings__item--disabled' : ''}${susfsItemClass()}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">{t('susfs.mapsHiding')}</div>
              <div class="settings__item-desc">{t('susfs.mapsHidingDesc')}</div>
            </div>
            <Toggle checked={store.settings.susfs.maps_hide} onChange={(v) => handleSusfsToggle('maps_hide', v)} disabled={susfsDisabled()} />
          </div>
        </div>

        <button class={`settings__advanced-toggle${showAdvanced() ? ' settings__advanced-toggle--open' : ''}`} onClick={() => setShowAdvanced(!showAdvanced())}>
          <svg class={`settings__advanced-chevron${showAdvanced() ? ' settings__advanced-chevron--open' : ''}`} viewBox="0 0 24 24" fill="currentColor">
            <path d="M7 10l5 5 5-5z"/>
          </svg>
          <span>{t('susfs.advancedSettings')}</span>
          <span class="settings__advanced-badge">20</span>
        </button>

        <Show when={showAdvanced()}>
          <div class="settings__advanced-content">
            <CollapsibleSubgroup
              label={t('susfs.controlLabel')}
              hiddenCount={5}
              defaultItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.emulateVoldAppData')}</div>
                    <div class="settings__item-desc">{t('susfs.emulateVoldAppDataDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.emulate_vold_app_data} onChange={(v) => handleBreneToggle('emulate_vold_app_data', v)} />
                </div>
                <Show when={store.settings.brene.emulate_vold_app_data}>
                  <div class="settings__item settings__item--nested">
                    <div class="settings__item-content">
                      <div class="settings__item-label">{t('susfs.voldUsePathLoop')}</div>
                      <div class="settings__item-desc">{t('susfs.voldUsePathLoopDesc')}</div>
                    </div>
                    <Toggle checked={store.settings.brene.vold_use_path_loop} onChange={(v) => handleBreneToggle('vold_use_path_loop', v)} />
                  </div>
                </Show>
              </>}
              expandedItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.forceHideLsposed')}</div>
                    <div class="settings__item-desc">{t('susfs.forceHideLsposedDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.force_hide_lsposed} onChange={(v) => handleBreneToggle('force_hide_lsposed', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideKsuLoopDevices')}</div>
                    <div class="settings__item-desc">{t('susfs.hideKsuLoopDevicesDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.hide_ksu_loops} onChange={(v) => handleBreneToggle('hide_ksu_loops', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.avcLogSpoofing')}</div>
                    <div class="settings__item-desc">{t('susfs.avcLogSpoofingDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.avc_log_spoofing} onChange={(v) => handleBreneToggle('avc_log_spoofing', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.spoofCmdline')}</div>
                    <div class="settings__item-desc">{t('susfs.spoofCmdlineDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.spoof_cmdline} onChange={(v) => handleBreneToggle('spoof_cmdline', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.susfsDebugLog')}</div>
                    <div class="settings__item-desc">{t('susfs.susfsDebugLogDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.susfs_log} onChange={(v) => handleBreneToggle('susfs_log', v)} />
                </div>
              </>}
            />

            <CollapsibleSubgroup
              label={t('susfs.autoHidingLabel')}
              hiddenCount={8}
              defaultItems={
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideRootedAppFolders')}</div>
                    <div class="settings__item-desc">{t('susfs.hideRootedAppFoldersDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_rooted_folders} onChange={(v) => handleBreneToggle('auto_hide_rooted_folders', v)} />
                </div>
              }
              expandedItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideModuleInjections')}</div>
                    <div class="settings__item-desc">{t('susfs.hideModuleInjectionsDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_injections} onChange={(v) => handleBreneToggle('auto_hide_injections', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideInjectedApks')}</div>
                    <div class="settings__item-desc">{t('susfs.hideInjectedApksDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_apk} onChange={(v) => handleBreneToggle('auto_hide_apk', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideZygiskMaps')}</div>
                    <div class="settings__item-desc">{t('susfs.hideZygiskMapsDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_zygisk} onChange={(v) => handleBreneToggle('auto_hide_zygisk', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideFontFilesMaps')}</div>
                    <div class="settings__item-desc">{t('susfs.hideFontFilesMapsDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_fonts} onChange={(v) => handleBreneToggle('auto_hide_fonts', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideRecoveryFolders')}</div>
                    <div class="settings__item-desc">{t('susfs.hideRecoveryFoldersDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_recovery} onChange={(v) => handleBreneToggle('auto_hide_recovery', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideDataLocalTmp')}</div>
                    <div class="settings__item-desc">{t('susfs.hideDataLocalTmpDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_tmp} onChange={(v) => handleBreneToggle('auto_hide_tmp', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.kernelUmount')}</div>
                    <div class="settings__item-desc">{t('susfs.kernelUmountDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.kernel_umount} onChange={(v) => handleBreneToggle('kernel_umount', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.autoTryUmount')}</div>
                    <div class="settings__item-desc">{t('susfs.autoTryUmountDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.try_umount} onChange={(v) => handleBreneToggle('try_umount', v)} />
                </div>
                <Show when={store.settings.brene.try_umount}>
                  <div class="settings__item settings__item--nested">
                    <div class="settings__item-content">
                      <div class="settings__item-label">{t('susfs.skipLegitMounts')}</div>
                      <div class="settings__item-desc">{t('susfs.skipLegitMountsDesc')}</div>
                    </div>
                    <Toggle checked={store.settings.brene.skip_legit_mounts} onChange={(v) => handleBreneToggle('skip_legit_mounts', v)} />
                  </div>
                </Show>
              </>}
            />

            <CollapsibleSubgroup
              label={t('susfs.customRomLabel')}
              hiddenCount={0}
              defaultItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.hideCusrom')}</div>
                    <div class="settings__item-desc">{t('susfs.hideCusromDesc')}</div>
                  </div>
                  <Toggle checked={store.settings.brene.hide_cusrom > 0} onChange={(v) => store.setBreneNumeric('hide_cusrom', v ? 1 : 0)} />
                </div>
                <Show when={store.settings.brene.hide_cusrom > 0}>
                  <div class="settings__item settings__item--nested" style={{ "flex-direction": "column", "align-items": "stretch" }}>
                    <div class="settings__item-content">
                      <div class="settings__item-label">{t('susfs.hideCusromLevel')} {store.settings.brene.hide_cusrom}</div>
                      <div class="settings__item-desc">
                        {store.settings.brene.hide_cusrom === 1 ? t('susfs.hideCusromLevel1')
                          : store.settings.brene.hide_cusrom === 2 ? t('susfs.hideCusromLevel2')
                          : store.settings.brene.hide_cusrom === 3 ? t('susfs.hideCusromLevel3')
                          : store.settings.brene.hide_cusrom === 4 ? t('susfs.hideCusromLevel4')
                          : t('susfs.hideCusromLevel5')}
                      </div>
                    </div>
                    <div class="settings__slider-row">
                      <input
                        type="range"
                        min="1"
                        max="5"
                        step="1"
                        value={store.settings.brene.hide_cusrom}
                        onInput={(e) => store.setBreneNumeric('hide_cusrom', parseInt(e.currentTarget.value))}
                        class="settings__slider"
                      />
                      <div class="settings__slider-labels">
                        <span>1</span><span>2</span><span>3</span><span>4</span><span>5</span>
                      </div>
                    </div>
                  </div>
                </Show>
              </>}
            />

            <CollapsibleSubgroup
              label={t('susfs.spoofingLabel')}
              hiddenCount={0}
              defaultItems={<>
                <div class="settings__item" onClick={() => setShowUnameSheet(true)} style={{ cursor: 'pointer' }}>
                  <div class="settings__item-content">
                    <div class="settings__item-label">{t('susfs.unameSpoofing')}</div>
                    <div class="settings__item-desc">{t('susfs.unameSpoofingDesc')}</div>
                  </div>
                  <button class="settings__select-trigger">
                    <span>{store.settings.uname.mode === 'disabled' ? t('susfs.unameModeDisabled') : store.settings.uname.mode === 'static' ? t('susfs.unameModeStatic') : t('susfs.unameModeDynamic')}</span>
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
                  </button>
                </div>
              </>}
            />
          </div>
        </Show>
      </Show>

      <UnameSheet
        open={showUnameSheet()}
        onClose={() => setShowUnameSheet(false)}
        mode={store.settings.uname.mode}
        release={store.settings.uname.release}
        version={store.settings.uname.version}
        onModeChange={(m) => store.setUnameMode(m)}
        onReleaseChange={(v) => store.setUnameField('release', v)}
        onVersionChange={(v) => store.setUnameField('version', v)}
      />
    </Card>
  );
}
