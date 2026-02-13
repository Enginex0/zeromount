import { Show, For, createMemo } from 'solid-js';
import { store } from '../../lib/store';
import type { CapabilityFlags, MountStrategy } from '../../lib/types';
import './ScenarioIndicator.css';

type IndicatorConfig = { label: string; color: string; description: string };

function strategyLabel(s: MountStrategy): string {
  switch (s) {
    case 'Vfs': return 'VFS Redirection';
    case 'Overlay': return 'OverlayFS';
    case 'MagicMount': return 'Magic Mount';
    default: return s;
  }
}

function strategyColor(s: MountStrategy): string {
  switch (s) {
    case 'Vfs': return 'var(--color-success)';
    case 'Overlay': return 'var(--color-info, #3b82f6)';
    case 'MagicMount': return 'var(--color-info, #3b82f6)';
    default: return 'var(--color-success)';
  }
}

// Reflects what's actually running, not just kernel capabilities
function buildConfig(scenario: string, strategy: MountStrategy | null, susfsEnabled: boolean): IndicatorConfig {
  const active = strategy || 'Vfs';
  const susfsLabel = susfsEnabled ? ' + SUSFS' : '';

  switch (scenario) {
    case 'full':
    case 'susfs_frontend':
    case 'kernel_only': {
      if (active === 'Vfs') {
        return {
          label: susfsEnabled ? 'Full Protection' : 'VFS Active',
          color: 'var(--color-success)',
          description: `VFS kernel driver${susfsLabel}`,
        };
      }
      return {
        label: `${strategyLabel(active)} Active`,
        color: strategyColor(active),
        description: `${strategyLabel(active)}${susfsLabel} · VFS available`,
      };
    }
    case 'susfs_only':
      return {
        label: `${strategyLabel(active)} Active`,
        color: strategyColor(active),
        description: `${strategyLabel(active)} + SUSFS protections`,
      };
    case 'none':
      return {
        label: `${strategyLabel(active)} Active`,
        color: strategyColor(active),
        description: `${strategyLabel(active)} module mounts`,
      };
    default:
      return {
        label: 'Initializing',
        color: 'var(--color-info, #3b82f6)',
        description: 'Detecting kernel capabilities...',
      };
  }
}

function getMissingCapabilities(caps: CapabilityFlags | null): string[] {
  if (!caps) return [];
  const missing: string[] = [];
  if (!caps.susfs_kstat) missing.push('kstat');
  if (!caps.susfs_path) missing.push('path');
  if (!caps.susfs_maps) missing.push('maps');
  if (!caps.susfs_open_redirect) missing.push('open_redirect');
  if (!caps.susfs_kstat_redirect) missing.push('kstat_redirect');
  if (!caps.susfs_open_redirect_all) missing.push('open_redirect_all');
  return missing;
}

export function ScenarioIndicator() {
  const scenario = () => store.scenario?.() || 'none';
  const activeStrategy = () => store.runtimeStrategy() || store.activeStrategy();
  const susfsEnabled = () => (store.capabilities?.()?.susfs_available ?? false) && store.settings.susfs.enabled;
  const config = createMemo(() => buildConfig(scenario(), activeStrategy(), susfsEnabled()));
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
      <Show when={scenario() === 'susfs_frontend' && missing().length > 0}>
        <div class="scenario__missing">
          <For each={missing()}>
            {(cap) => (
              <span class="scenario__missing-chip">{cap}</span>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
