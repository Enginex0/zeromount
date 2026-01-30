import { createSignal, For } from 'solid-js';
import { Card } from '../components/core/Card';
import { Button } from '../components/core/Button';
import { Toggle } from '../components/core/Toggle';
import { Modal } from '../components/layout/Modal';
import { store } from '../lib/store';
import { api } from '../lib/api';
import { GITHUB_URL } from '../lib/constants';
import "./SettingsTab.css";

const accentColors = [
  { name: 'Cyan', color: '#00BCD4' },
  { name: 'Orange', color: '#FF8E53' },
  { name: 'Gold', color: '#FFC107' },
  { name: 'Emerald', color: '#00D68F' },
  { name: 'Azure', color: '#00B4D8' },
  { name: 'Purple', color: '#764BA2' },
  { name: 'White', color: '#FFFFFF' },
  { name: 'Slate', color: '#64748B' },
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
            <div class="settings__item-label">Auto System Color</div>
            <div class="settings__item-desc">Use device's accent color</div>
          </div>
          <Toggle
            checked={store.settings.autoAccentColor}
            onChange={async (checked) => {
              store.updateSettings({ autoAccentColor: checked });
              if (checked) {
                const systemColor = await api.fetchSystemColor();
                if (systemColor) {
                  store.updateSettings({ accentColor: systemColor });
                }
              }
            }}
          />
        </div>

        <div class="settings__item">
          <div class="settings__item-content">
            <div class="settings__item-label">Animations</div>
            <div class="settings__item-desc">Enable bouncy animations</div>
          </div>
          <Toggle
            checked={store.settings.animationsEnabled}
            onChange={(checked) => store.updateSettings({ animationsEnabled: checked })}
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
              <div class="settings__item-label">Auto-start on boot</div>
              <div class="settings__item-desc">Start ZeroMount engine automatically</div>
            </div>
            <Toggle
              checked={store.settings.autoStartOnBoot}
              onChange={(checked) => store.updateSettings({ autoStartOnBoot: checked })}
            />
          </div>

          <div class="settings__item">
            <div class="settings__item-content">
              <div class="settings__item-label">Verbose logging</div>
              <div class="settings__item-desc">Log detailed operation info</div>
            </div>
            <Toggle
              checked={store.settings.verboseLogging}
              onChange={async (checked) => {
                store.updateSettings({ verboseLogging: checked });
                await api.setVerboseLogging(checked);
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
