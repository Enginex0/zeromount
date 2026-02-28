import { createSignal, Show } from 'solid-js';
import { Card } from '../core/Card';
import { Toggle } from '../core/Toggle';
import { Input } from '../core/Input';
import { CollapsibleSubgroup } from '../ui/CollapsibleSubgroup';
import { UnameSheet } from '../ui/UnameSheet';
import { store } from '../../lib/store';
import type { BreneSettings, SusfsSettings } from '../../lib/types';

export function SusfsSection() {
  const [showAdvanced, setShowAdvanced] = createSignal(false);
  const [showVbh, setShowVbh] = createSignal(false);
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
        SUSFS Integration
      </h3>

      <div class="settings__item">
        <div class="settings__item-content">
          <div class="settings__item-label">SUSFS Integration</div>
          <div class="settings__item-desc">
            {susfsAvailable()
              ? `Version ${caps()?.susfs_version || 'unknown'} — ${susfsEnabled() ? 'active' : 'disabled'}`
              : 'Not detected on this kernel'}
          </div>
        </div>
        <Toggle
          checked={susfsEnabled()}
          onChange={(v) => {
            handleSusfsToggle('enabled', v);
            const ext = externalModule();
            if (ext) {
              store.showToast(
                v ? 'Taking SUSFS ownership from zeromount' : `Deferred to ${ext}`,
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
              <div class="settings__item-label">Hide Sus Mounts</div>
              <div class="settings__item-desc">Hide module mounts from non-root processes</div>
            </div>
            <Toggle checked={store.settings.brene.hide_sus_mounts} onChange={(v) => handleBreneToggle('hide_sus_mounts', v)} disabled={!susfsEnabled()} />
          </div>
          <div class={`settings__item settings__item--sub${!susfsEnabled() ? ' settings__item--disabled' : ''}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">Path Hiding</div>
              <div class="settings__item-desc">Hide paths from detection apps</div>
            </div>
            <Toggle checked={store.settings.susfs.path_hide} onChange={(v) => handleSusfsToggle('path_hide', v)} disabled={susfsDisabled()} />
          </div>
          <div class={`settings__item settings__item--sub${susfsDisabled() ? ' settings__item--disabled' : ''}${susfsItemClass()}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">Kstat Spoofing</div>
              <div class="settings__item-desc">Spoof file metadata for redirected files</div>
            </div>
            <Toggle checked={store.settings.susfs.kstat} onChange={(v) => handleSusfsToggle('kstat', v)} disabled={susfsDisabled()} />
          </div>
          <div class={`settings__item settings__item--sub${susfsDisabled() ? ' settings__item--disabled' : ''}${susfsItemClass()}`}>
            <div class="settings__item-content">
              <div class="settings__item-label">Maps Hiding</div>
              <div class="settings__item-desc">Hide module entries from /proc/maps</div>
            </div>
            <Toggle checked={store.settings.susfs.maps_hide} onChange={(v) => handleSusfsToggle('maps_hide', v)} disabled={susfsDisabled()} />
          </div>
        </div>

        <button class={`settings__advanced-toggle${showAdvanced() ? ' settings__advanced-toggle--open' : ''}`} onClick={() => setShowAdvanced(!showAdvanced())}>
          <svg class={`settings__advanced-chevron${showAdvanced() ? ' settings__advanced-chevron--open' : ''}`} viewBox="0 0 24 24" fill="currentColor">
            <path d="M7 10l5 5 5-5z"/>
          </svg>
          <span>Advanced Settings</span>
          <span class="settings__advanced-badge">17</span>
        </button>

        <Show when={showAdvanced()}>
          <div class="settings__advanced-content">
            <CollapsibleSubgroup
              label="SUSFS Control"
              hiddenCount={5}
              defaultItems={
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Emulate Vold App Data</div>
                    <div class="settings__item-desc">Hide app data paths via sus_path on /sdcard/Android/data</div>
                  </div>
                  <Toggle checked={store.settings.brene.emulate_vold_app_data} onChange={(v) => handleBreneToggle('emulate_vold_app_data', v)} />
                </div>
              }
              expandedItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Force Hide LSPosed</div>
                    <div class="settings__item-desc">Unmount dex2oat paths to hide LSPosed injection</div>
                  </div>
                  <Toggle checked={store.settings.brene.force_hide_lsposed} onChange={(v) => handleBreneToggle('force_hide_lsposed', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide KSU Loop Devices</div>
                    <div class="settings__item-desc">Hide loop device entries in /proc/fs</div>
                  </div>
                  <Toggle checked={store.settings.brene.hide_ksu_loops} onChange={(v) => handleBreneToggle('hide_ksu_loops', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">AVC Log Spoofing</div>
                    <div class="settings__item-desc">Suppress SELinux audit log entries</div>
                  </div>
                  <Toggle checked={store.settings.brene.avc_log_spoofing} onChange={(v) => handleBreneToggle('avc_log_spoofing', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Spoof Cmdline</div>
                    <div class="settings__item-desc">Replace verifiedbootstate and hwname in /proc/cmdline</div>
                  </div>
                  <Toggle checked={store.settings.brene.spoof_cmdline} onChange={(v) => handleBreneToggle('spoof_cmdline', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">SUSFS Debug Log</div>
                    <div class="settings__item-desc">Enable kernel-level SUSFS logging</div>
                  </div>
                  <Toggle checked={store.settings.brene.susfs_log} onChange={(v) => handleBreneToggle('susfs_log', v)} />
                </div>
              </>}
            />

            <CollapsibleSubgroup
              label="Auto-Hiding"
              hiddenCount={8}
              defaultItems={
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Rooted App Folders</div>
                    <div class="settings__item-desc">Hide Magisk/KSU app data directories</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_rooted_folders} onChange={(v) => handleBreneToggle('auto_hide_rooted_folders', v)} />
                </div>
              }
              expandedItems={<>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Module Injections</div>
                    <div class="settings__item-desc">Hide injected module files from /proc/maps</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_injections} onChange={(v) => handleBreneToggle('auto_hide_injections', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Injected APKs</div>
                    <div class="settings__item-desc">Hide module APKs in vendor/product/system_ext</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_apk} onChange={(v) => handleBreneToggle('auto_hide_apk', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Zygisk .so in Maps</div>
                    <div class="settings__item-desc">Remove zygisk entries from /proc/maps</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_zygisk} onChange={(v) => handleBreneToggle('auto_hide_zygisk', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Font Files in Maps</div>
                    <div class="settings__item-desc">Remove custom font entries from /proc/maps</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_fonts} onChange={(v) => handleBreneToggle('auto_hide_fonts', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide Recovery Folders</div>
                    <div class="settings__item-desc">Hide TWRP/OrangeFox directories</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_recovery} onChange={(v) => handleBreneToggle('auto_hide_recovery', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Hide /data/local/tmp</div>
                    <div class="settings__item-desc">Hide temp files used by detection tools</div>
                  </div>
                  <Toggle checked={store.settings.brene.auto_hide_tmp} onChange={(v) => handleBreneToggle('auto_hide_tmp', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Kernel Umount</div>
                    <div class="settings__item-desc">Enable kernel-level module unmounting via ksud</div>
                  </div>
                  <Toggle checked={store.settings.brene.kernel_umount} onChange={(v) => handleBreneToggle('kernel_umount', v)} />
                </div>
                <div class="settings__item">
                  <div class="settings__item-content">
                    <div class="settings__item-label">Auto Try Umount</div>
                    <div class="settings__item-desc">Discover and unmount KSU bind mounts in app namespaces</div>
                  </div>
                  <Toggle checked={store.settings.brene.try_umount} onChange={(v) => handleBreneToggle('try_umount', v)} />
                </div>
              </>}
            />

            <CollapsibleSubgroup
              label="Spoofing"
              hiddenCount={0}
              defaultItems={<>
                <div class="settings__item" onClick={() => setShowUnameSheet(true)} style={{ cursor: 'pointer' }}>
                  <div class="settings__item-content">
                    <div class="settings__item-label">Uname Spoofing</div>
                    <div class="settings__item-desc">Spoof kernel version string</div>
                  </div>
                  <button class="settings__select-trigger">
                    <span>{store.settings.uname.mode === 'disabled' ? 'Disabled' : store.settings.uname.mode === 'static' ? 'Static' : 'Dynamic'}</span>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
                  </button>
                </div>
                <div class="settings__glass-row" onClick={() => setShowVbh(!showVbh())}>
                  <div class="settings__item-content">
                    <div class="settings__item-label">Verified Boot Hash</div>
                    <div class="settings__item-desc">Stock vbmeta digest for Play Integrity</div>
                  </div>
                  <svg class={`settings__glass-chevron${showVbh() ? ' settings__glass-chevron--open' : ''}`} width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M7 10l5 5 5-5z"/>
                  </svg>
                </div>
                <Show when={showVbh()}>
                  <div class="settings__glass-slider">
                    <Input
                      value={store.settings.brene.verified_boot_hash}
                      placeholder="SHA256 hex digest (64 chars)"
                      onBlur={(e) => store.setBreneField('verified_boot_hash', e.currentTarget.value)}
                    />
                  </div>
                </Show>
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
