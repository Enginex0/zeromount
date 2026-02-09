import { Show, For } from 'solid-js';
import { store } from '../../lib/store';
import type { CapabilityFlags } from '../../lib/types';
import './ScenarioIndicator.css';

const scenarioConfig: Record<string, { label: string; color: string; description: string }> = {
  full: {
    label: 'Full Protection',
    color: 'var(--color-success)',
    description: 'VFS redirection + full SUSFS capabilities',
  },
  susfs_frontend: {
    label: 'Partial Protection',
    color: 'var(--color-warning)',
    description: 'VFS redirection + limited SUSFS capabilities',
  },
  kernel_only: {
    label: 'VFS Only',
    color: '#FF8E53',
    description: 'VFS redirection only, no SUSFS',
  },
  susfs_only: {
    label: 'SUSFS Only',
    color: 'var(--color-success)',
    description: 'SUSFS protections active, overlay/magic mount for modules',
  },
  none: {
    label: 'Mount Fallback',
    color: 'var(--color-error)',
    description: 'Using OverlayFS/magic mount (no kernel patches)',
  },
};

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
  const config = () => scenarioConfig[scenario()] || scenarioConfig.none;
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
