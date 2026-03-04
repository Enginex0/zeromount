import { Card } from '../core/Card';
import { Button } from '../core/Button';
import { store } from '../../lib/store';
import { GITHUB_URL, PATHS } from '../../lib/constants';
import { ksuExec, ksuWriteFile } from '../../lib/ksuApi';
import type { Settings } from '../../lib/types';

export function AboutSection() {
  const copyDebugInfo = async () => {
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} log dump`);
      if (errno === 0) {
        const parsed = JSON.parse(stdout);
        store.showToast(`Logs saved to ${parsed.zip}`, 'success');
      } else {
        store.showToast('Failed to dump logs', 'error');
      }
    } catch {
      store.showToast('Failed to dump logs', 'error');
    }
  };

  const exportConfig = async () => {
    try {
      const { errno, stdout } = await ksuExec('cat /data/adb/zeromount/config.toml');
      if (errno !== 0 || !stdout) {
        store.showToast('Failed to read config', 'error');
        return;
      }
      const backup = JSON.stringify({
        version: 1,
        backend: stdout,
        webui: {
          theme: localStorage.getItem('zeromount-theme'),
          accentColor: localStorage.getItem('zeromount-accent'),
          autoAccentColor: localStorage.getItem('zeromount-autoAccent'),
          fixedNav: localStorage.getItem('zeromount-fixedNav'),
          bgOpacity: localStorage.getItem('zeromount-bgOpacity'),
        },
      }, null, 2);
      const { errno: writeErr } = await ksuWriteFile(backup, '/sdcard/Download/zeromount-backup.json');
      if (writeErr === 0) {
        store.showToast('Config exported to Downloads', 'success');
      } else {
        store.showToast('Failed to export config', 'error');
      }
    } catch {
      store.showToast('Failed to export config', 'error');
    }
  };

  const importConfig = () => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '*/*';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const content = await file.text();
        if (!content.trim()) {
          store.showToast('Selected file is empty', 'error');
          return;
        }

        let backendConfig = content;
        let webuiSettings: Record<string, string | null> | null = null;

        try {
          const parsed = JSON.parse(content);
          if (parsed.version && parsed.backend) {
            backendConfig = parsed.backend;
            webuiSettings = parsed.webui || null;
          }
        } catch {
          // Not JSON — treat as raw TOML config
        }

        const { errno } = await ksuWriteFile(backendConfig, '/data/adb/zeromount/config.toml');
        if (errno !== 0) {
          store.showToast('Failed to write config', 'error');
          return;
        }

        if (webuiSettings) {
          const keyMap: Record<string, string> = {
            theme: 'zeromount-theme',
            accentColor: 'zeromount-accent',
            autoAccentColor: 'zeromount-autoAccent',
            fixedNav: 'zeromount-fixedNav',
            bgOpacity: 'zeromount-bgOpacity',
          };
          for (const [key, storageKey] of Object.entries(keyMap)) {
            const val = webuiSettings[key];
            if (val != null) localStorage.setItem(storageKey, val);
          }

          const uiUpdates: Partial<Settings> = {};
          if (webuiSettings.theme) uiUpdates.theme = webuiSettings.theme as Settings['theme'];
          if (webuiSettings.accentColor) uiUpdates.accentColor = webuiSettings.accentColor;
          if (webuiSettings.autoAccentColor != null) uiUpdates.autoAccentColor = webuiSettings.autoAccentColor === 'true';
          if (webuiSettings.fixedNav != null) uiUpdates.fixedNav = webuiSettings.fixedNav === 'true';
          if (Object.keys(uiUpdates).length) store.updateSettings(uiUpdates);
          if (webuiSettings.bgOpacity != null) store.setBgOpacity(parseFloat(webuiSettings.bgOpacity));
        }

        await store.loadInitialData();
        store.showToast(`Imported ${file.name} — reload complete`, 'success');
      } catch {
        store.showToast('Failed to import config', 'error');
      }
    };
    input.click();
  };

  return (
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
          onClick={() => ksuExec(`am start -a android.intent.action.VIEW -d '${GITHUB_URL}'`)}
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
          <Button variant="secondary" size="small" onClick={exportConfig}>
            Export Config
          </Button>
          <Button variant="secondary" size="small" onClick={importConfig}>
            Import Config
          </Button>
        </div>

        <div class="settings__footer">
          Made with <span class="gradient-text">passion</span> for the Android <span class="gradient-text">community</span>
        </div>
      </div>
    </Card>
  );
}
