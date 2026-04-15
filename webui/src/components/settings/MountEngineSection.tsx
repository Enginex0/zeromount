import { createSignal, Show } from 'solid-js';
import { Card } from '../core/Card';
import { Toggle } from '../core/Toggle';
import { Input } from '../core/Input';
import { BottomSheet } from '../ui/BottomSheet';
import { ChipSelect } from '../ui/ChipSelect';
import { CollapsibleSubgroup } from '../ui/CollapsibleSubgroup';
import { store } from '../../lib/store';
import { t } from '../../lib/i18n';
import type { StorageMode } from '../../lib/types';

export function MountEngineSection() {
  const [customOverlaySource, setCustomOverlaySource] = createSignal('');
  const [customMountSource, setCustomMountSource] = createSignal('');
  const [showOverlaySheet, setShowOverlaySheet] = createSignal(false);
  const [showStagingSheet, setShowStagingSheet] = createSignal(false);

  const caps = () => store.capabilities?.() || null;

  return (
    <Card>
      <h3 class="settings__section-title">
        <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
          <path d="M20 6h-8l-2-2H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2zm-6 10H6v-2h8v2zm4-4H6v-2h12v2z"/>
        </svg>
        {t('engine.mountEngine')}
      </h3>

      <div class="settings__group">
        <div class="settings__item-label">{t('engine.mountStrategy')}</div>
        <div class="settings__item-desc" style={{ "margin-bottom": "12px" }}>
          {caps()?.vfs_driver
            ? t('engine.mountStrategyDescVfs')
            : t('engine.mountStrategyDescNoVfs')}
        </div>
        <div class="settings__strategies">
          <button
            class={`settings__strategy${store.effectiveStrategy() === 'Vfs' ? ' settings__strategy--active' : ''}${!caps()?.vfs_driver ? ' settings__strategy--disabled' : ''}`}
            onClick={() => store.setMountStrategy('Vfs')}
            disabled={!caps()?.vfs_driver}
            title={!caps()?.vfs_driver ? t('engine.titleVfsUnavailable') : t('engine.titleVfsAuto')}
          >
            <div style={{ "margin-bottom": "4px" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill={store.effectiveStrategy() === 'Vfs' ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
              </svg>
            </div>
            <div class={`settings__strategy-label${store.effectiveStrategy() === 'Vfs' ? ' settings__strategy-label--active' : ''}`}>
              {t('engine.strategyVfs')}
            </div>
            <div class="settings__strategy-hint">{t('engine.strategyVfsHint')}</div>
          </button>

          <button
            class={`settings__strategy${store.effectiveStrategy() === 'Overlay' ? ' settings__strategy--active' : ''}${!caps()?.overlay_supported ? ' settings__strategy--disabled' : ''}`}
            onClick={() => store.setMountStrategy('Overlay')}
            disabled={!caps()?.overlay_supported}
            title={!caps()?.overlay_supported ? t('engine.titleOverlayUnavailable') : t('engine.titleOverlayPrefer')}
          >
            <div style={{ "margin-bottom": "4px" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill={store.effectiveStrategy() === 'Overlay' ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                <path d="M4 6H2v14c0 1.1.9 2 2 2h14v-2H4V6zm16-4H8c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V4c0-1.1-.9-2-2-2zm0 14H8V4h12v12z"/>
              </svg>
            </div>
            <div class={`settings__strategy-label${store.effectiveStrategy() === 'Overlay' ? ' settings__strategy-label--active' : ''}`}>
              {t('engine.strategyOverlay')}
            </div>
            <div class="settings__strategy-hint">{t('engine.strategyOverlayHint')}</div>
          </button>

          <button
            class={`settings__strategy${store.effectiveStrategy() === 'MagicMount' ? ' settings__strategy--active' : ''}`}
            onClick={() => store.setMountStrategy('MagicMount')}
            title={t('engine.titleMagicMount')}
          >
            <div style={{ "margin-bottom": "4px" }}>
              <svg width="24" height="24" viewBox="0 0 24 24" fill={store.effectiveStrategy() === 'MagicMount' ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                <path d="M17 1.01L7 1c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-1.99-2-1.99zM17 19H7V5h10v14z"/>
              </svg>
            </div>
            <div class={`settings__strategy-label${store.effectiveStrategy() === 'MagicMount' ? ' settings__strategy-label--active' : ''}`}>
              {t('engine.strategyMagic')}
            </div>
            <div class="settings__strategy-hint">{t('engine.strategyMagicHint')}</div>
          </button>
        </div>
        <div class="settings__item-desc" style={{ "margin-top": "8px", "font-style": "italic" }}>
          {t('engine.switchingRequiresReboot')}
        </div>
        <Show when={store.effectiveStrategy() === 'MagicMount' && !caps()?.susfs_available}>
          <div class="settings__item-desc" style={{ "margin-top": "6px", color: "var(--warning)" }}>
            {t('engine.magicMountNoSusfsWarning')}
          </div>
        </Show>
      </div>

      <Show when={store.effectiveStrategy() !== 'Vfs'}>
        <Show when={store.effectiveStrategy() === 'Overlay'}>
          <div class="settings__group" style={{ "margin-top": "16px" }}>
            <div class="settings__item-label">{t('engine.storageBackend')}</div>
            <div class="settings__item-desc" style={{ "margin-bottom": "10px" }}>
              {t('engine.storageBackendDesc')}
              {caps()?.tmpfs_xattr ? '' : ` (${t('engine.storageBackendNoXattr')})`}
            </div>
            <ChipSelect
              value={store.settings.mount.storage_mode}
              onChange={(v) => store.setMountStorageMode(v as StorageMode)}
              options={[
                { value: 'auto', label: t('engine.storageAuto') },
                { value: 'erofs', label: t('engine.storageErofs'), disabled: !caps()?.erofs_supported },
                { value: 'tmpfs', label: t('engine.storageTmpfs'), disabled: !caps()?.tmpfs_xattr },
                { value: 'ext4', label: t('engine.storageExt4') },
              ]}
            />
            <Show when={
              store.resolvedStorageMode() &&
              store.settings.mount.storage_mode !== 'auto' &&
              store.resolvedStorageMode() !== store.settings.mount.storage_mode
            }>
              <div class="settings__item-desc" style={{ color: 'var(--warning)', "margin-top": "8px" }}>
                {t('engine.storageUnavailable', { selected: store.settings.mount.storage_mode, resolved: store.resolvedStorageMode()! })}
              </div>
            </Show>
          </div>
        </Show>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">{t('engine.randomMountPaths')}</div>
            <div class="settings__item-desc">{t('engine.randomMountPathsDesc')}</div>
          </div>
          <Toggle
            checked={store.settings.mount.random_mount_paths}
            onChange={(v) => store.setMountToggle('random_mount_paths', v)}
          />
        </div>

        <Show when={store.effectiveStrategy() !== 'MagicMount'}>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">{t('engine.overlayMountSource')}</div>
              <div class="settings__item-desc">
                {t('engine.overlayMountSourceDesc')}
              </div>
            </div>
            <button class="settings__select-trigger" onClick={() => setShowOverlaySheet(true)}>
              <span>{['auto', 'KSU', 'magisk', 'overlay'].includes(store.settings.mount.overlay_source) ? store.settings.mount.overlay_source : t('engine.customLabel')}</span>
              <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
            </button>
          </div>
          <BottomSheet
            open={showOverlaySheet()}
            onClose={() => setShowOverlaySheet(false)}
            title={t('engine.overlayMountSource')}
            value={['auto', 'KSU', 'magisk', 'overlay'].includes(store.settings.mount.overlay_source) ? store.settings.mount.overlay_source : 'custom'}
            onChange={(val) => {
              if (val !== 'custom') {
                store.setOverlaySource(val);
              } else {
                setCustomOverlaySource('');
              }
            }}
            options={[
              { value: 'auto', label: t('engine.storageAuto'), description: t('engine.overlayOptAuto') },
              { value: 'KSU', label: 'KSU', description: t('engine.overlayOptKsu') },
              { value: 'magisk', label: 'magisk', description: t('engine.overlayOptMagisk') },
              { value: 'overlay', label: 'overlay', description: t('engine.overlayOptOverlay') },
              { value: 'custom', label: t('engine.customLabel'), description: t('engine.overlayOptCustom') },
            ]}
            customInput={{
              placeholder: t('engine.overlayPlaceholder'),
              value: customOverlaySource(),
              onInput: setCustomOverlaySource,
              onConfirm: (v) => store.setOverlaySource(v),
            }}
          />
        </Show>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">{t('engine.stagingMountSource')}</div>
            <div class="settings__item-desc">
              {t('engine.stagingMountSourceDesc')}
            </div>
          </div>
          <button class="settings__select-trigger" onClick={() => setShowStagingSheet(true)}>
            <span>{['auto', 'tmpfs', 'none', 'shmem', 'shm'].includes(store.settings.mount.mount_source) ? store.settings.mount.mount_source : t('engine.customLabel')}</span>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
          </button>
        </div>
        <BottomSheet
          open={showStagingSheet()}
          onClose={() => setShowStagingSheet(false)}
          title={t('engine.stagingMountSource')}
          value={['auto', 'tmpfs', 'none', 'shmem', 'shm'].includes(store.settings.mount.mount_source) ? store.settings.mount.mount_source : 'custom'}
          onChange={(val) => {
            if (val !== 'custom') {
              store.setMountSource(val);
            } else {
              setCustomMountSource('');
            }
          }}
          options={[
            { value: 'auto', label: t('engine.storageAuto'), description: t('engine.stagingOptAuto') },
            { value: 'tmpfs', label: 'tmpfs', description: t('engine.stagingOptTmpfs') },
            { value: 'none', label: 'none', description: t('engine.stagingOptNone') },
            { value: 'shmem', label: 'shmem', description: t('engine.stagingOptShmem') },
            { value: 'shm', label: 'shm', description: t('engine.stagingOptShm') },
            { value: 'custom', label: t('engine.customLabel'), description: t('engine.stagingOptCustom') },
          ]}
          customInput={{
            placeholder: t('engine.stagingPlaceholder'),
            value: customMountSource(),
            onInput: setCustomMountSource,
            onConfirm: (v) => store.setMountSource(v),
          }}
        />

        <CollapsibleSubgroup
          label={t('engine.advancedMounting')}
          hiddenCount={store.settings.mount.storage_mode === 'ext4' ? 1 : 0}
          defaultItems={
            <div class="settings__item">
              <div class="settings__item-content">
                <div class="settings__item-label">{t('engine.restartFramework')}</div>
                <div class="settings__item-desc">{t('engine.restartFrameworkDesc')}</div>
              </div>
              <Toggle
                checked={store.settings.mount.restart_framework}
                onChange={(v) => store.setMountToggle('restart_framework', v)}
              />
            </div>
          }
          expandedItems={<>
            <Show when={store.settings.mount.storage_mode === 'ext4'}>
              <div class="settings__item" style={{ "flex-direction": "column", "align-items": "stretch" }}>
                <div class="settings__item-content">
                  <div class="settings__item-label">{t('engine.ext4ImageSize')}</div>
                  <div class="settings__item-desc">{t('engine.ext4ImageSizeDesc')}</div>
                </div>
                <Input
                  fullWidth
                  type="number"
                  value={String(store.settings.mount.ext4_image_size_mb)}
                  onBlur={(e) => store.setMountField('ext4_image_size_mb', parseInt(e.currentTarget.value, 10) || 0)}
                />
              </div>
            </Show>
          </>}
        />
      </Show>

      <div class="settings__item">
        <div class="settings__item-content">
          <div class="settings__item-label">{t('engine.excludeHosts')}</div>
          <div class="settings__item-desc">{t('engine.excludeHostsDesc')}</div>
        </div>
        <Toggle
          checked={store.settings.mount.exclude_hosts_modules}
          onChange={(v) => store.setMountToggle('exclude_hosts_modules', v)}
        />
      </div>
    </Card>
  );
}
