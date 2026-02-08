import { createSignal, Show, For } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Toggle } from '../components/core/Toggle';
import { Input } from '../components/core/Input';
import { Modal } from '../components/layout/Modal';
import { store } from '../lib/store';
import { GITHUB_URL } from '../lib/constants';
import type { BreneSettings, SusfsSettings, UnameMode } from '../lib/types';
import "./SettingsTab.css";

const accentColors = [
  { name: 'Orange', color: '#FF8E53' },
  { name: 'Emerald', color: '#00D68F' },
  { name: 'Azure', color: '#00B4D8' },
  { name: 'Slate', color: '#64748B' },
  { name: 'Indigo', color: '#6366F1' },
  { name: 'Coral', color: '#FF6B6B' },
];

export function SettingsTab() {
  const [showClearConfirm, setShowClearConfirm] = createSignal(false);
  const selectedAccent = () => store.settings.accentColor;

  const handleThemeChange = (newTheme: 'dark' | 'light' | 'auto' | 'amoled') => {
    store.updateSettings({ theme: newTheme });
  };

  const handleClearAll = async () => {
    await store.clearAllRules();
    setShowClearConfirm(false);
  };

  const copyDebugInfo = () => {
    const info = `
Zero-Mount
Driver: ${store.systemInfo.driverVersion}
Kernel: ${store.systemInfo.kernelVersion}
SUSFS: ${store.systemInfo.susfsVersion}
Active Rules: ${store.stats.activeRules}
Excluded Apps: ${store.stats.excludedUids}
Engine: ${store.engineActive() ? 'Active' : 'Inactive'}
    `.trim();

    navigator.clipboard.writeText(info).then(() => {
      store.showToast('Debug info copied to clipboard', 'success');
    });
  };

  const isThemeActive = (themeName: string) => store.settings.theme === themeName;

  const caps = () => store.capabilities?.() || null;
  const susfsAvailable = () => caps()?.susfs_available ?? false;

  const handleBreneToggle = (key: keyof BreneSettings, value: boolean) => {
    store.setBreneToggle(key, value);
  };

  const handleSusfsToggle = (key: keyof SusfsSettings, value: boolean) => {
    store.setSusfsToggle(key, value);
  };

  return (
    <div class="settings">
      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 3c-4.97 0-9 4.03-9 9s4.03 9 9 9c.83 0 1.5-.67 1.5-1.5 0-.39-.15-.74-.39-1.01-.23-.26-.38-.61-.38-.99 0-.83.67-1.5 1.5-1.5H16c2.76 0 5-2.24 5-5 0-4.42-4.03-8-9-8zm-5.5 9c-.83 0-1.5-.67-1.5-1.5S5.67 9 6.5 9 8 9.67 8 10.5 7.33 12 6.5 12zm3-4C8.67 8 8 7.33 8 6.5S8.67 5 9.5 5s1.5.67 1.5 1.5S10.33 8 9.5 8zm5 0c-.83 0-1.5-.67-1.5-1.5S13.67 5 14.5 5s1.5.67 1.5 1.5S15.33 8 14.5 8zm3 4c-.83 0-1.5-.67-1.5-1.5S16.67 9 17.5 9s1.5.67 1.5 1.5-.67 1.5-1.5 1.5z"/>
          </svg>
          Appearance
        </h3>

        <div class="settings__group">
          <div class="settings__label">Theme</div>
          <div class="settings__themes">
            <button
              class={`settings__theme ${isThemeActive('dark') ? 'settings__theme--active' : ''}`}
              onClick={() => handleThemeChange('dark')}
            >
              <div class="settings__theme-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill={isThemeActive('dark') ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                  <path d="M12.43 2.3c-2.38-.59-4.68-.27-6.63.64-.35.16-.41.64-.1.86C8.3 5.6 10 8.6 10 12c0 3.4-1.7 6.4-4.3 8.2-.32.22-.26.7.09.86 1.28.6 2.71.94 4.21.94 6.05 0 10.85-5.38 9.87-11.6-.61-3.92-3.59-7.16-7.44-8.1z"/>
                </svg>
              </div>
              <div class={`settings__theme-label ${isThemeActive('dark') ? 'settings__theme-label--active' : ''}`}>
                Dark
              </div>
            </button>

            <button
              class={`settings__theme ${isThemeActive('light') ? 'settings__theme--active' : ''}`}
              onClick={() => handleThemeChange('light')}
            >
              <div class="settings__theme-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill={isThemeActive('light') ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                  <path d="M6.76 4.84l-1.8-1.79-1.41 1.41 1.79 1.79 1.42-1.41zM4 10.5H1v2h3v-2zm9-9.95h-2V3.5h2V.55zm7.45 3.91l-1.41-1.41-1.79 1.79 1.41 1.41 1.79-1.79zm-3.21 13.7l1.79 1.8 1.41-1.41-1.8-1.79-1.4 1.4zM20 10.5v2h3v-2h-3zm-8-5c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6-2.69-6-6-6zm-1 16.95h2V19.5h-2v2.95zm-7.45-3.91l1.41 1.41 1.79-1.8-1.41-1.41-1.79 1.8z"/>
                </svg>
              </div>
              <div class={`settings__theme-label ${isThemeActive('light') ? 'settings__theme-label--active' : ''}`}>
                Light
              </div>
            </button>

            <button
              class={`settings__theme ${isThemeActive('auto') ? 'settings__theme--active' : ''}`}
              onClick={() => handleThemeChange('auto')}
            >
              <div class="settings__theme-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill={isThemeActive('auto') ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                  <path d="M12 4V2A10 10 0 0 0 2 12h2a8 8 0 0 1 8-8zm0 16a8 8 0 0 1-8-8H2a10 10 0 0 0 10 10v-2zm8-8a8 8 0 0 1-8 8v2a10 10 0 0 0 10-10h-2zm-8-8a8 8 0 0 1 8 8h2A10 10 0 0 0 12 2v2z"/>
                </svg>
              </div>
              <div class={`settings__theme-label ${isThemeActive('auto') ? 'settings__theme-label--active' : ''}`}>
                Auto
              </div>
            </button>

            <button
              class={`settings__theme ${isThemeActive('amoled') ? 'settings__theme--active' : ''}`}
              onClick={() => handleThemeChange('amoled')}
            >
              <div class="settings__theme-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill={isThemeActive('amoled') ? 'var(--text-accent)' : 'var(--text-secondary)'}>
                  <circle cx="12" cy="12" r="10"/>
                </svg>
              </div>
              <div class={`settings__theme-label ${isThemeActive('amoled') ? 'settings__theme-label--active' : ''}`}>
                AMOLED
              </div>
            </button>
          </div>
        </div>

        <div class={`settings__group ${store.settings.autoAccentColor ? 'settings__group--disabled' : ''}`}>
          <div class="settings__label">Accent Color</div>
          <div class="settings__colors">
            <For each={accentColors}>
              {(accent) => (
                <button
                  class={`settings__color ${selectedAccent() === accent.color ? 'settings__color--active' : ''} ${store.settings.autoAccentColor ? 'settings__color--disabled' : ''}`}
                  onClick={() => {
                    if (!store.settings.autoAccentColor) {
                      store.updateSettings({ accentColor: accent.color });
                    }
                  }}
                  disabled={store.settings.autoAccentColor}
                  style={{
                    background: accent.color,
                    "box-shadow": selectedAccent() === accent.color ? `0 0 0 3px ${accent.color}40` : 'none'
                  }}
                />
              )}
            </For>
          </div>
        </div>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">Random Accent</div>
            <div class="settings__item-desc">Change accent color each session</div>
          </div>
          <Toggle
            checked={store.settings.autoAccentColor}
            onChange={async (checked) => {
              store.updateSettings({ autoAccentColor: checked });
              if (checked) {
                await store.fetchSystemColor();
              }
            }}
          />
        </div>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">Fix Bottom Nav</div>
            <div class="settings__item-desc">Pin navigation to bottom of screen</div>
          </div>
          <Toggle
            checked={store.settings.fixedNav}
            onChange={(checked) => store.updateSettings({ fixedNav: checked })}
          />
        </div>
      </Card>

      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M19.14 12.94c.04-.31.06-.63.06-.94 0-.31-.02-.63-.06-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.04.31-.06.63-.06.94s.02.63.06.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z"/>
          </svg>
          Engine
        </h3>

        <div class="settings__engine-controls">
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Verbose logging</div>
              <div class="settings__item-desc">Log detailed operation info</div>
            </div>
            <Toggle
              checked={store.settings.verboseLogging}
              onChange={async (checked) => {
                await store.setVerboseLogging(checked);
              }}
            />
          </div>

          <Button
            variant="danger"
            fullWidth
            onClick={() => setShowClearConfirm(true)}
          >
            CLEAR ALL RULES
          </Button>
        </div>
      </Card>

      {/* SUSFS Integration -- capability-aware hierarchical toggles */}
      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"/>
          </svg>
          SUSFS Integration
        </h3>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">SUSFS Available</div>
            <div class="settings__item-desc">
              {susfsAvailable()
                ? `Version ${caps()?.susfs_version || 'unknown'}`
                : 'Not detected on this kernel'}
            </div>
          </div>
          <Toggle
            checked={susfsAvailable()}
            onChange={() => {}}
            disabled
          />
        </div>

        <Show when={susfsAvailable()}>
          <div class="settings__sub-toggles">
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Path Hiding</div>
                <div class="settings__item-desc">Hide paths from detection apps</div>
              </div>
              <Toggle checked={store.settings.susfs.path_hide} onChange={(v) => handleSusfsToggle('path_hide', v)} />
            </div>
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Kstat Spoofing</div>
                <div class="settings__item-desc">Spoof file metadata for redirected files</div>
              </div>
              <Toggle checked={store.settings.susfs.kstat} onChange={(v) => handleSusfsToggle('kstat', v)} />
            </div>
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Maps Hiding</div>
                <div class="settings__item-desc">Hide module entries from /proc/maps</div>
              </div>
              <Toggle checked={store.settings.susfs.maps_hide} onChange={(v) => handleSusfsToggle('maps_hide', v)} />
            </div>
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Font Redirect</div>
                <div class="settings__item-desc">Redirect font files via open_redirect</div>
              </div>
              <Toggle checked={store.settings.susfs.open_redirect} onChange={(v) => handleSusfsToggle('open_redirect', v)} />
            </div>
          </div>
        </Show>
      </Card>

      {/* BRENE Automation -- persists to config.toml */}
      <Show when={susfsAvailable()}>
        <Card>
          <h3 class="settings__section-title">
            <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
              <path d="M17 1H7c-1.1 0-2 .9-2 2v18c0 1.1.9 2 2 2h10c1.1 0 2-.9 2-2V3c0-1.1-.9-2-2-2zm0 18H7V5h10v14z"/>
            </svg>
            BRENE Automation
          </h3>

          <div class="settings__group-label color-text-tertiary">Auto-Hiding</div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide Injected APKs</div>
              <div class="settings__item-desc">Hide module APKs in vendor/product/system_ext</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_apk}
              onChange={(v) => handleBreneToggle('auto_hide_apk', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide Zygisk .so in Maps</div>
              <div class="settings__item-desc">Remove zygisk entries from /proc/maps</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_zygisk}
              onChange={(v) => handleBreneToggle('auto_hide_zygisk', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide Font Files in Maps</div>
              <div class="settings__item-desc">Remove custom font entries from /proc/maps</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_fonts}
              onChange={(v) => handleBreneToggle('auto_hide_fonts', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide Rooted App Folders</div>
              <div class="settings__item-desc">Hide Magisk/KSU app data directories</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_rooted_folders}
              onChange={(v) => handleBreneToggle('auto_hide_rooted_folders', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide Recovery Folders</div>
              <div class="settings__item-desc">Hide TWRP/OrangeFox directories</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_recovery}
              onChange={(v) => handleBreneToggle('auto_hide_recovery', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide /data/local/tmp</div>
              <div class="settings__item-desc">Hide temp files used by detection tools</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_tmp}
              onChange={(v) => handleBreneToggle('auto_hide_tmp', v)}
            />
          </div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Hide /sdcard/Android/data</div>
              <div class="settings__item-desc">Hide sensitive app data on sdcard</div>
            </div>
            <Toggle
              checked={store.settings.brene.auto_hide_sdcard_data}
              onChange={(v) => handleBreneToggle('auto_hide_sdcard_data', v)}
            />
          </div>

          <div class="settings__group-label color-text-tertiary">Logging</div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">SUSFS Debug Log</div>
              <div class="settings__item-desc">Enable kernel-level SUSFS logging</div>
            </div>
            <Toggle
              checked={store.settings.brene.susfs_log}
              onChange={(v) => handleBreneToggle('susfs_log', v)}
            />
          </div>

          <div class="settings__group-label color-text-tertiary">Spoofing</div>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Uname Spoofing</div>
              <div class="settings__item-desc">Spoof kernel version string</div>
            </div>
            <select
              class="settings__select"
              value={store.settings.uname.mode}
              onChange={(e) => store.setUnameMode(e.currentTarget.value as UnameMode)}
            >
              <option value="disabled">Disabled</option>
              <option value="static">Static</option>
              <option value="dynamic">Dynamic</option>
            </select>
          </div>
          <Show when={store.settings.uname.mode !== 'disabled'}>
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Release</div>
                <div class="settings__item-desc">e.g. 5.10.0-android12-gki</div>
              </div>
              <Input
                value={store.settings.uname.release}
                placeholder="5.10.0-gki"
                onBlur={(e) => store.setUnameField('release', e.currentTarget.value)}
              />
            </div>
            <div class="settings__item settings__item--sub">
              <div class="settings__item-content">
                <div class="settings__item-label">Version</div>
                <div class="settings__item-desc">e.g. #1 SMP PREEMPT</div>
              </div>
              <Input
                value={store.settings.uname.version}
                placeholder="#1 SMP PREEMPT"
                onBlur={(e) => store.setUnameField('version', e.currentTarget.value)}
              />
            </div>
          </Show>
          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">AVC Log Spoofing</div>
              <div class="settings__item-desc">Suppress SELinux audit log entries</div>
            </div>
            <Toggle
              checked={store.settings.brene.avc_log_spoofing}
              onChange={(v) => handleBreneToggle('avc_log_spoofing', v)}
            />
          </div>
        </Card>
      </Show>

      {/* Property Spoofing -- separate section, uses resetprop not SUSFS */}
      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M14 2H6c-1.1 0-1.99.9-1.99 2L4 20c0 1.1.89 2 1.99 2H18c1.1 0 2-.9 2-2V8l-6-6zm2 16H8v-2h8v2zm0-4H8v-2h8v2zM13 9V3.5L18.5 9H13z"/>
          </svg>
          Property Spoofing
        </h3>
        <div class="settings__item-desc settings__prop-note color-text-tertiary">
          Uses resetprop, independent of SUSFS
        </div>
        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">Build Properties</div>
            <div class="settings__item-desc">Spoof ro.build.* properties for detection bypass</div>
          </div>
          <Toggle checked={store.settings.brene.prop_spoofing} onChange={(v) => handleBreneToggle('prop_spoofing', v)} />
        </div>
      </Card>

      <Card>
        <h3 class="settings__section-title">
          <svg class="settings__section-icon" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z"/>
          </svg>
          About
        </h3>

        <div class="settings__about">
          <div class="logo-container">
            <div class="logo-ring" />
            <div class="logo-inner">
              <svg width="56" height="56" viewBox="0 0 24 24" fill="url(#shieldGradient)">
                <defs>
                  <linearGradient id="shieldGradient" x1="0%" y1="0%" x2="100%" y2="100%">
                    <stop offset="0%" style="stop-color: #FF6B6B" />
                    <stop offset="50%" style="stop-color: #FF8E53" />
                    <stop offset="100%" style="stop-color: #FFC107" />
                  </linearGradient>
                </defs>
                <path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V5l-9-4zm0 10.99h7c-.53 4.12-3.28 7.79-7 8.94V12H5V6.3l7-3.11v8.8z"/>
              </svg>
            </div>
          </div>

          <div class="settings__about-title">ZeroMount</div>

          <div class="settings__about-badge">GHOST</div>

          <button
            class="settings__repo-btn"
            onClick={() => window.open(GITHUB_URL, '_blank')}
          >
            <svg class="settings__repo-icon" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
            </svg>
            Repository
          </button>

          <div class="settings__actions">
            <Button variant="secondary" size="small" onClick={copyDebugInfo}>
              Copy Debug Info
            </Button>
            <Button
              variant="secondary"
              size="small"
              onClick={() => {
                const config = {
                  rules: store.rules(),
                  excludedUids: store.excludedUids(),
                  settings: store.settings,
                  exportDate: new Date().toISOString(),
                };
                const blob = new Blob([JSON.stringify(config, null, 2)], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const a = document.createElement('a');
                a.href = url;
                a.download = 'zeromount-config.json';
                a.click();
                URL.revokeObjectURL(url);
                store.showToast('Config exported', 'success');
              }}
            >
              Export Config
            </Button>
          </div>

          <div class="settings__footer">
            Made with <span class="glow-text">passion</span> for the Android <span class="glow-text">community</span>
          </div>
        </div>
      </Card>

      <Modal
        open={showClearConfirm()}
        onClose={() => setShowClearConfirm(false)}
        title="Clear All Rules?"
      >
        <div class="settings__modal-content">
          <p class="settings__modal-text">
            This will permanently delete all {store.stats.activeRules} VFS rules.
            This action cannot be undone.
          </p>

          <div class="settings__modal-actions">
            <Button
              variant="ghost"
              onClick={() => setShowClearConfirm(false)}
            >
              Cancel
            </Button>
            <Button
              variant="danger"
              onClick={handleClearAll}
              loading={store.loading.rules}
            >
              CLEAR ALL
            </Button>
          </div>
        </div>
      </Modal>
    </div>
  );
}
