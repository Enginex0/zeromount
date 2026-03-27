import { Show, For, createMemo } from 'solid-js';
import { store } from '../../lib/store';
import { t } from '../../lib/i18n';
import type { CapabilityFlags, MountStrategy } from '../../lib/types';
import './ScenarioIndicator.css';

type IndicatorConfig = { label: string; color: string; description: string };

const strategyKeys: Record<string, string> = {
  Vfs: 'status.modeVfs',
  Overlay: 'status.modeOverlay',
  MagicMount: 'status.modeMagicMount',
};

function strategyLabel(s: MountStrategy): string {
  return t(strategyKeys[s] ?? s);
}

function strategyColor(s: MountStrategy): string {
  return s === 'Vfs' ? 'var(--color-success)' : 'var(--color-info, #3b82f6)';
}

function buildConfig(scenario: string, strategy: MountStrategy | null, susfsEnabled: boolean, susfsMode?: string): IndicatorConfig {
  const active = strategy || 'Vfs';
  const susfsLabel = susfsMode === 'enhanced'
    ? t('scenario.susfsEnhanced')
    : susfsMode === 'embedded'
      ? t('scenario.susfsEmbedded')
      : susfsEnabled ? t('scenario.susfsSuffix') : '';
  const name = strategyLabel(active);

  switch (scenario) {
    case 'full':
    case 'susfs_frontend':
    case 'kernel_only': {
      if (active === 'Vfs') {
        return {
          label: susfsEnabled ? t('scenario.fullProtection') : t('scenario.vfsActive'),
          color: 'var(--color-success)',
          description: t('scenario.descVfs', { susfs: susfsLabel }),
        };
      }
      return {
        label: t('scenario.strategyActive', { strategy: name }),
        color: strategyColor(active),
        description: t('scenario.descAltVfsAvail', { strategy: name, susfs: susfsLabel }),
      };
    }
    case 'susfs_only':
      return {
        label: t('scenario.strategyActive', { strategy: name }),
        color: strategyColor(active),
        description: t('scenario.descSusfsOnly', { strategy: name }),
      };
    case 'none':
      return {
        label: t('scenario.strategyActive', { strategy: name }),
        color: strategyColor(active),
        description: t('scenario.descModuleMounts', { strategy: name }),
      };
    default:
      return {
        label: t('scenario.initializing'),
        color: 'var(--color-info, #3b82f6)',
        description: t('scenario.descInitializing'),
      };
  }
}

function getMissingCapabilities(caps: CapabilityFlags | null): string[] {
  if (!caps) return [];
  const missing: string[] = [];
  if (!caps.susfs_kstat) missing.push('kstat');
  if (!caps.susfs_path) missing.push('path');
  if (!caps.susfs_maps) missing.push('maps');
  if (!caps.susfs_kstat_redirect) missing.push('kstat_redirect');
  return missing;
}

export function ScenarioIndicator() {
  const scenario = () => store.scenario?.() || 'none';
  const activeStrategy = () => store.runtimeStrategy() || store.effectiveStrategy();
  const susfsEnabled = () => (store.capabilities?.()?.susfs_available ?? false) && store.settings.susfs.enabled;
  const susfsMode = () => store.capabilities?.()?.susfs_mode;
  const config = createMemo(() => buildConfig(scenario(), activeStrategy(), susfsEnabled(), susfsMode()));
  const missing = () => getMissingCapabilities(store.capabilities?.() || null);

  return (
    <div class="scenario">
      <div class="scenario__chip" style={{ 'border-color': config().color }}>
        <span class="scenario__dot" style={{ background: config().color, color: config().color }} />
        <span class="scenario__label">
          {config().label}
        </span>
      </div>
      <div class="scenario__desc">
        {config().description}
      </div>
    </div>
  );
}
