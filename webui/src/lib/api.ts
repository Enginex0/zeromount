import type { VfsRule, ExcludedUid, SystemInfo, ActivityItem, EngineStats, InstalledApp, KsuModule, RuntimeStatus, WebUiInitResponse } from './types';
import { ksuExec } from './ksuApi';
import { PATHS, APP_VERSION } from './constants';
// Lazy-load mock module only in dev. The import() is behind import.meta.env.DEV
// so Vite replaces it with `false` in prod and never emits the api.mock chunk.
let _mockModule: typeof import('./api.mock') | undefined;
async function getMock() {
  if (import.meta.env.DEV && !_mockModule) _mockModule = await import('./api.mock');
  return _mockModule!.MockAPI;
}

export function escapeShellArg(arg: string): string {
  return "'" + arg.replace(/'/g, "'\\''") + "'";
}

export function shouldUseMock(): boolean {
  return import.meta.env.DEV && typeof globalThis.ksu === 'undefined';
}


function parseRulesOutput(stdout: string): VfsRule[] {
  console.log('[ZM-API] parseRulesOutput() input:', stdout?.slice(0, 200));
  const lines = stdout.trim().split('\n').filter(line => line.trim());
  // Kernel outputs: real_path->virtual_path (zeromount-core.patch:1021)
  // source = real_path (module file providing content)
  // target = virtual_path (system path where it appears)
  const rules = lines.map((line, index) => {
    const idx = line.indexOf('->');
    const source = idx === -1 ? line.trim() : line.slice(0, idx).trim();
    const target = idx === -1 ? '[BLOCKED]' : line.slice(idx + 2).trim();
    return {
      id: String(index + 1),
      name: target.split('/').pop() || 'Rule',
      source,
      target,
      createdAt: new Date(),
    };
  });
  console.log('[ZM-API] parseRulesOutput() parsed', rules.length, 'rules');
  return rules;
}

interface ExclusionMeta {
  [uid: string]: {
    packageName: string;
    appName: string;
    excludedAt: string;
  };
}

// Serialize EXCLUSION_META read-modify-write to prevent concurrent overwrites
let metaMutex: Promise<void> = Promise.resolve();
function withMetaLock<T>(fn: () => Promise<T>): Promise<T> {
  const prev = metaMutex;
  let resolve: () => void;
  metaMutex = new Promise<void>(r => { resolve = r; });
  return prev.then(fn).finally(() => resolve!());
}

async function parseExclusionFiles(): Promise<ExcludedUid[]> {
  console.log('[ZM-API] parseExclusionFiles() called');
  try {
    const { errno: listErr, stdout: listOut } = await ksuExec(`cat "${PATHS.EXCLUSION_FILE}"`);
    console.log('[ZM-API] parseExclusionFiles() list result:', { errno: listErr, stdout: listOut?.slice(0, 100) });
    if (listErr !== 0 || !listOut.trim()) {
      console.log('[ZM-API] parseExclusionFiles() no exclusions found');
      return [];
    }

    const uids = listOut.trim().split('\n').map(line => parseInt(line.trim(), 10)).filter(uid => !isNaN(uid));
    console.log('[ZM-API] parseExclusionFiles() parsed UIDs:', uids);

    let meta: ExclusionMeta = {};
    try {
      const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}"`);
      console.log('[ZM-API] parseExclusionFiles() meta result:', { errno: metaErr, stdout: metaOut?.slice(0, 100) });
      if (metaErr === 0 && metaOut.trim()) {
        meta = JSON.parse(metaOut);
        console.log('[ZM-API] parseExclusionFiles() parsed meta for', Object.keys(meta).length, 'UIDs');
      }
    } catch (e) {
      console.log('[ZM-API] parseExclusionFiles() meta parse error (optional):', e);
      // Meta file optional
    }

    const result = uids.map(uid => {
      const info = meta[String(uid)];
      return {
        uid,
        packageName: info?.packageName || `app_${uid}`,
        appName: info?.appName || `UID ${uid}`,
        excludedAt: info?.excludedAt ? new Date(info.excludedAt) : new Date(),
      };
    });
    console.log('[ZM-API] parseExclusionFiles() returning', result.length, 'exclusions');
    return result;
  } catch (e) {
    console.error('[ZM-API] parseExclusionFiles() error:', e);
    return [];
  }
}

async function parseActivityLog(): Promise<ActivityItem[]> {
  console.log('[ZM-API] parseActivityLog() called');
  try {
    const { errno, stdout } = await ksuExec(`tail -10 "${PATHS.ACTIVITY_LOG}"`);
    console.log('[ZM-API] parseActivityLog() result:', { errno, stdout: stdout?.slice(0, 100) });
    if (errno !== 0 || !stdout.trim()) {
      console.log('[ZM-API] parseActivityLog() no activity found');
      return [];
    }

    const validTypes = ['rule_added', 'rule_removed', 'uid_excluded', 'uid_included', 'engine_enabled', 'engine_disabled', 'setting_changed', 'mount_strategy_changed', 'susfs_toggle', 'brene_toggle', 'theme_changed'];
    const lines = stdout.trim().split('\n').filter(line => line.trim());
    const items: ActivityItem[] = [];
    for (let i = 0; i < lines.length; i++) {
      const match = lines[i].match(/^\[(.+?)\]\s+(\w+):\s+(.+)$/);
      if (!match) continue;
      const [, timestamp, type, message] = match;
      const parsed = new Date(timestamp);
      if (isNaN(parsed.getTime())) continue;
      const normalizedType = type.toLowerCase();
      items.push({
        id: String(i + 1),
        type: (validTypes.includes(normalizedType) ? normalizedType : 'engine_enabled') as ActivityItem['type'],
        message,
        timestamp: parsed,
      });
    }
    items.sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime());
    console.log('[ZM-API] parseActivityLog() parsed', items.length, 'activity items');
    return items.slice(0, 10);
  } catch (e) {
    console.error('[ZM-API] parseActivityLog() error:', e);
    return [];
  }
}

async function logActivity(type: string, message: string): Promise<void> {
  if (shouldUseMock()) return;
  try {
    const timestamp = new Date().toISOString();
    const line = `[${timestamp}] ${type.toUpperCase()}: ${message}`;
    await ksuExec(`echo ${escapeShellArg(line)} >> "${PATHS.ACTIVITY_LOG}"`);
  } catch (e) {
    console.error('[ZM-API] logActivity() error:', e);
  }
}

export const api = {
  async getVersion(): Promise<string> {
    console.log('[ZM-API] getVersion() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getVersion();
    }
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} version`);
      if (errno === 0 && stdout) {
        console.log('[ZM-API] getVersion() returning:', stdout.trim());
        return stdout.trim();
      }
      console.log('[ZM-API] getVersion() command failed, using fallback');
    } catch (e) {
      console.error('[ZM-API] getVersion() error:', e);
      // Fallback
    }
    console.log('[ZM-API] getVersion() returning fallback:', `v${APP_VERSION}`);
    return `v${APP_VERSION}`;
  },

  async getSystemInfo(): Promise<SystemInfo> {
    console.log('[ZM-API] getSystemInfo() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getSystemInfo();
    }

    const info: SystemInfo = {
      driverVersion: `v${APP_VERSION}`,
      kernelVersion: '-',
      susfsVersion: '-',
      uptime: '-',
      deviceModel: '-',
      androidVersion: '-',
      selinuxStatus: '-',
    };

    try {
      console.log('[ZM-API] getSystemInfo() fetching system info...');
      const [verRes, kernRes, uptimeRes, modelRes, androidRes, selinuxRes, susfsRes] = await Promise.all([
        ksuExec(`${PATHS.BINARY} version`).catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('uname -r').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('cat /proc/uptime').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('getprop ro.product.model').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('getprop ro.build.version.release').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('getenforce').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        ksuExec('ksu_susfs show version 2>/dev/null || echo ""').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
      ]);

      if (verRes.errno === 0 && verRes.stdout) {
        info.driverVersion = verRes.stdout.trim();
      }
      if (kernRes.errno === 0 && kernRes.stdout) {
        info.kernelVersion = kernRes.stdout.trim();
      }
      if (uptimeRes.errno === 0 && uptimeRes.stdout) {
        const seconds = parseInt(uptimeRes.stdout.split(' ')[0], 10);
        if (!isNaN(seconds)) {
          const hours = Math.floor(seconds / 3600);
          const mins = Math.floor((seconds % 3600) / 60);
          info.uptime = `${hours}h ${mins}m`;
        }
      }
      if (modelRes.errno === 0 && modelRes.stdout.trim()) {
        info.deviceModel = modelRes.stdout.trim();
      }
      if (androidRes.errno === 0 && androidRes.stdout.trim()) {
        info.androidVersion = androidRes.stdout.trim();
      }
      if (selinuxRes.errno === 0 && selinuxRes.stdout.trim()) {
        info.selinuxStatus = selinuxRes.stdout.trim();
      }
      if (susfsRes.errno === 0 && susfsRes.stdout.trim()) {
        info.susfsVersion = susfsRes.stdout.trim();
      }
      console.log('[ZM-API] getSystemInfo() returning:', info);
    } catch (e) {
      console.error('[ZM-API] getSystemInfo() error:', e);
    }

    return info;
  },

  async getSystemInfoBatched(): Promise<SystemInfo> {
    if (shouldUseMock()) {
      return (await getMock()).getSystemInfo();
    }

    const info: SystemInfo = {
      driverVersion: `v${APP_VERSION}`,
      kernelVersion: '-',
      susfsVersion: '-',
      uptime: '-',
      deviceModel: '-',
      androidVersion: '-',
      selinuxStatus: '-',
    };

    try {
      const { errno, stdout } = await ksuExec(
        [
          `${PATHS.BINARY} version 2>/dev/null || echo ''`,
          'uname -r',
          'cat /proc/uptime',
          'getprop ro.product.model',
          'getprop ro.build.version.release',
          'getenforce 2>/dev/null || echo Permissive',
          "ksu_susfs show version 2>/dev/null || echo ''",
        ].map(c => `echo "$(${c})"`).join(' && echo "---DELIM---" && ')
      );

      if (errno === 0 && stdout) {
        const parts = stdout.split('---DELIM---').map(s => s.trim());
        if (parts[0]) info.driverVersion = parts[0];
        if (parts[1]) info.kernelVersion = parts[1];
        if (parts[2]) {
          const seconds = parseInt(parts[2].split(' ')[0], 10);
          if (!isNaN(seconds)) {
            const hours = Math.floor(seconds / 3600);
            const mins = Math.floor((seconds % 3600) / 60);
            info.uptime = `${hours}h ${mins}m`;
          }
        }
        if (parts[3]) info.deviceModel = parts[3];
        if (parts[4]) info.androidVersion = parts[4];
        if (parts[5]) info.selinuxStatus = parts[5];
        if (parts[6]) info.susfsVersion = parts[6];
      }
    } catch (e) {
      console.error('[ZM-API] getSystemInfoBatched() error:', e);
    }

    return info;
  },

  async getRules(): Promise<VfsRule[]> {
    console.log('[ZM-API] getRules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getRules();
    }
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} vfs list`);
      if (errno === 0 && stdout.trim()) {
        const rules = parseRulesOutput(stdout);
        console.log('[ZM-API] getRules() returning', rules.length, 'rules');
        return rules;
      }
      console.log('[ZM-API] getRules() command returned no data');
    } catch (e) {
      console.error('[ZM-API] getRules() error:', e);
      // Fallback
    }
    console.log('[ZM-API] getRules() returning empty array');
    return [];
  },

  async clearAllRules(): Promise<void> {
    console.log('[ZM-API] clearAllRules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).clearAllRules();
    }

    const cmd = `${PATHS.BINARY} vfs clear`;
    console.log('[ZM-API] clearAllRules() executing:', cmd);
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] clearAllRules() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to clear rules');
    }
    console.log('[ZM-API] clearAllRules() success');
  },

  async getExcludedUids(): Promise<ExcludedUid[]> {
    console.log('[ZM-API] getExcludedUids() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getExcludedUids();
    }
    const result = await parseExclusionFiles();
    console.log('[ZM-API] getExcludedUids() returning', result.length, 'exclusions');
    return result;
  },

  async excludeUid(uid: number, packageName: string, appName: string): Promise<ExcludedUid> {
    console.log('[ZM-API] excludeUid() called:', { uid, packageName, appName, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return (await getMock()).excludeUid(uid, packageName, appName);
    }

    // Validation FIRST
    if (!Number.isInteger(uid) || uid < 0) throw new Error('Invalid UID');

    const cmd = `${PATHS.BINARY} uid block ${escapeShellArg(String(uid))}`;
    console.log('[ZM-API] excludeUid() executing:', cmd);
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] excludeUid() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to exclude UID');
    }

    console.log('[ZM-API] excludeUid() success');

    await ksuExec(`echo ${escapeShellArg(String(uid))} >> "${PATHS.EXCLUSION_FILE}"`).catch(e => console.error('[ZM-API] Exclusion persistence failed:', e));

    await withMetaLock(async () => {
      try {
        let meta: Record<string, { packageName: string; appName: string; excludedAt: string }> = {};
        const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
        if (metaErr === 0 && metaOut.trim()) {
          try {
            meta = JSON.parse(metaOut);
          } catch (parseErr) {
            console.error('[ZM-API] Metadata parse error, starting fresh:', parseErr);
            meta = {};
          }
        }
        meta[String(uid)] = { packageName, appName, excludedAt: new Date().toISOString() };
        await ksuExec(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
      } catch (e) {
        console.error('[ZM-API] Metadata save failed:', e);
      }
    });

    return {
      uid,
      packageName,
      appName,
      excludedAt: new Date(),
    };
  },

  async includeUid(uid: number): Promise<void> {
    console.log('[ZM-API] includeUid() called:', { uid, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return (await getMock()).includeUid(uid);
    }

    const cmd = `${PATHS.BINARY} uid unblock ${escapeShellArg(String(uid))}`;
    console.log('[ZM-API] includeUid() executing:', cmd);
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] includeUid() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to include UID');
    }
    console.log('[ZM-API] includeUid() success');

    if (!/^\d+$/.test(String(uid))) throw new Error('invalid uid');
    await ksuExec(`sed -i '/^${uid}$/d' "${PATHS.EXCLUSION_FILE}"`).catch(e => console.error('[ZM-API] Exclusion removal failed:', e));

    await withMetaLock(async () => {
      try {
        const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
        if (metaErr === 0 && metaOut.trim()) {
          try {
            const meta = JSON.parse(metaOut);
            delete meta[String(uid)];
            await ksuExec(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
          } catch (parseErr) {
            console.error('[ZM-API] Metadata parse error during cleanup:', parseErr);
          }
        }
      } catch (e) {
        console.error('[ZM-API] Metadata cleanup failed:', e);
      }
    });

  },

  async getActivity(): Promise<ActivityItem[]> {
    console.log('[ZM-API] getActivity() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getActivity();
    }
    const result = await parseActivityLog();
    console.log('[ZM-API] getActivity() returning', result.length, 'items');
    return result;
  },

  async getStats(): Promise<EngineStats> {
    console.log('[ZM-API] getStats() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).getStats();
    }

    const [rules, uids] = await Promise.all([
      this.getRules(),
      this.getExcludedUids(),
    ]);

    const stats = {
      activeRules: rules.length,
      excludedUids: uids.length,
      hiddenPaths: 0,
      hiddenMaps: 0,
    };
    console.log('[ZM-API] getStats() returning:', stats);
    return stats;
  },

  async toggleEngine(enable: boolean): Promise<void> {
    console.log('[ZM-API] toggleEngine() called:', { enable, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return (await getMock()).toggleEngine(enable);
    }

    const cmd = enable ? `${PATHS.BINARY} vfs enable` : `${PATHS.BINARY} vfs disable`;
    console.log('[ZM-API] toggleEngine() executing:', cmd);
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] toggleEngine() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to toggle engine');
    }
    console.log('[ZM-API] toggleEngine() success');
  },

  async setVerboseLogging(enabled: boolean): Promise<void> {
    if (shouldUseMock()) {
      return (await getMock()).setVerboseLogging(enabled);
    }

    const subcmd = enabled ? 'enable' : 'disable';
    const cmd = `${PATHS.BINARY} log ${subcmd}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(stderr || 'Failed to set verbose logging');
    }
  },

  async getVerboseLogging(): Promise<boolean> {
    if (shouldUseMock()) {
      return false;
    }

    const cmd = `${PATHS.BINARY} config get logging.verbose`;
    const { errno, stdout } = await ksuExec(cmd);
    if (errno !== 0) {
      return false;
    }
    return stdout.trim() === 'true';
  },

  async getInstalledApps(): Promise<InstalledApp[]> {
    if (shouldUseMock()) return (await getMock()).getInstalledApps();
    return [];
  },

  // Check if daemon has signaled a refresh (returns trigger timestamp or null)
  async getRefreshTrigger(): Promise<number | null> {
    if (shouldUseMock()) return null;

    try {
      const response = await fetch('link/.refresh_trigger?t=' + Date.now());
      if (response.ok) {
        const text = await response.text();
        const timestamp = parseInt(text.trim(), 10);
        if (!isNaN(timestamp)) {
          return timestamp;
        }
      }
    } catch (e) {
      // Trigger file doesn't exist yet
    }
    return null;
  },

  async scanKsuModules(): Promise<KsuModule[]> {
    console.log('[ZM-API] scanKsuModules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).scanKsuModules();
    }

    try {
      // Single shell call that outputs JSON - replaces ~22 separate exec calls
      const script = `
# Extract unique module base paths from zm list (format: system_path->module_file_path)
loaded_modules=$(${PATHS.BINARY} vfs list 2>/dev/null | awk -F'->' '{print $2}' | grep -oE '/data/adb/modules/[^/]+' | sort -u || echo "")
echo "["
first=1
for dir in /data/adb/modules/*/; do
  [ -d "$dir" ] || continue
  name=$(basename "$dir")
  path="/data/adb/modules/$name"

  has_sys=0; has_ven=0; has_prod=0
  [ -d "$path/system" ] && has_sys=1
  [ -d "$path/vendor" ] && has_ven=1
  [ -d "$path/product" ] && has_prod=1

  [ $has_sys -eq 0 ] && [ $has_ven -eq 0 ] && [ $has_prod -eq 0 ] && continue

  count=$(find "$path" -type f \\( -path "*/system/*" -o -path "*/vendor/*" -o -path "*/product/*" \\) 2>/dev/null | wc -l)

  display_name="$name"
  if [ -f "$path/module.prop" ]; then
    prop_name=$(grep "^name=" "$path/module.prop" 2>/dev/null | cut -d= -f2)
    [ -n "$prop_name" ] && display_name="$prop_name"
  fi

  is_loaded=0
  echo "$loaded_modules" | grep -qx "$path" && is_loaded=1

  [ $first -eq 0 ] && echo ","
  first=0
  printf '{"name":"%s","path":"%s","hasSystem":%s,"hasVendor":%s,"hasProduct":%s,"isLoaded":%s,"fileCount":%d}' \\
    "$display_name" "$path" \\
    $([ $has_sys -eq 1 ] && echo true || echo false) \\
    $([ $has_ven -eq 1 ] && echo true || echo false) \\
    $([ $has_prod -eq 1 ] && echo true || echo false) \\
    $([ $is_loaded -eq 1 ] && echo true || echo false) \\
    "$count"
done
echo "]"
`;
      const { errno, stdout } = await ksuExec(script);
      if (errno !== 0) {
        console.log('[ZM-API] scanKsuModules() script failed');
        return [];
      }

      // Filesystem corruption can produce duplicate directory entries (same path).
      // Dedup by path to avoid showing the same module twice.
      const raw = JSON.parse(stdout.trim()) as KsuModule[];
      const seen = new Set<string>();
      const modules = raw.filter(m => {
        if (seen.has(m.path)) return false;
        seen.add(m.path);
        return true;
      });
      console.log('[ZM-API] scanKsuModules() returning', modules.length, 'modules');
      return modules;
    } catch (e) {
      console.error('[ZM-API] scanKsuModules() error:', e);
      return [];
    }
  },

  async loadKsuModule(moduleName: string, modulePath: string): Promise<number> {
    console.log('[ZM-API] loadKsuModule() called:', { moduleName, modulePath, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return (await getMock()).loadKsuModule(moduleName, modulePath);
    }

    try {
      const { stdout } = await ksuExec(
        `find ${escapeShellArg(modulePath)} -type f \\( -path "*/system/*" -o -path "*/vendor/*" -o -path "*/product/*" \\) 2>/dev/null`
      );
      const files = stdout.trim().split('\n').filter(Boolean);
      const cmds = files.flatMap(filePath => {
        const rel = filePath.replace(modulePath, '');
        if (!rel.startsWith('/system/') && !rel.startsWith('/vendor/') && !rel.startsWith('/product/')) return [];
        return [`${PATHS.BINARY} vfs add ${escapeShellArg(rel)} ${escapeShellArg(filePath)} && echo OK || echo FAIL`];
      });

      let addedCount = 0;
      if (cmds.length > 0) {
        const { stdout: batchOut } = await ksuExec(cmds.join('\n'));
        addedCount = (batchOut.match(/\bOK\b/g) ?? []).length;
      }

      await logActivity('MODULE_LOADED', `${moduleName}: ${addedCount} rules added`);
      console.log('[ZM-API] loadKsuModule() added', addedCount, 'rules');
      return addedCount;
    } catch (e) {
      console.error('[ZM-API] loadKsuModule() error:', e);
      throw e;
    }
  },

  async unloadKsuModule(moduleName: string, modulePath: string): Promise<number> {
    console.log('[ZM-API] unloadKsuModule() called:', { moduleName, modulePath, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return (await getMock()).unloadKsuModule(moduleName, modulePath);
    }

    try {
      const rules = await this.getRules();
      const moduleRules = rules.filter(r => r.source.startsWith(modulePath));
      const cmds = moduleRules.map(r => `${PATHS.BINARY} vfs del ${escapeShellArg(r.target)} && echo OK || echo FAIL`);

      let removedCount = 0;
      if (cmds.length > 0) {
        const { stdout: batchOut } = await ksuExec(cmds.join('\n'));
        removedCount = (batchOut.match(/\bOK\b/g) ?? []).length;
      }

      await logActivity('MODULE_UNLOADED', `${moduleName}: ${removedCount} rules removed`);
      console.log('[ZM-API] unloadKsuModule() removed', removedCount, 'rules');
      return removedCount;
    } catch (e) {
      console.error('[ZM-API] unloadKsuModule() error:', e);
      throw e;
    }
  },

  async fetchSystemColor(): Promise<string | null> {
    console.log('[ZM-API] fetchSystemColor() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return (await getMock()).fetchSystemColor();
    }
    try {
      const { errno, stdout } = await ksuExec(
        'settings get secure theme_customization_overlay_packages'
      );
      if (errno !== 0 || !stdout.trim()) {
        console.log('[ZM-API] fetchSystemColor() no system color found');
        return null;
      }
      const match = /#?([0-9a-fA-F]{6,8})/i.exec(stdout);
      if (match) {
        const color = '#' + match[1].slice(0, 6);
        console.log('[ZM-API] fetchSystemColor() found:', color);
        return color;
      }
      console.log('[ZM-API] fetchSystemColor() no hex color in output');
      return null;
    } catch (e) {
      console.error('[ZM-API] fetchSystemColor() error:', e);
      return null;
    }
  },

  async getRuntimeStatus(): Promise<RuntimeStatus | null> {
    if (shouldUseMock()) {
      return (await getMock()).getRuntimeStatus();
    }
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} status --json`, 5000);
      if (errno === 0 && stdout.trim()) {
        return JSON.parse(stdout.trim()) as RuntimeStatus;
      }
    } catch (e) {
      console.error('[ZM-API] getRuntimeStatus() error:', e);
    }
    return null;
  },

  async configDump(): Promise<Record<string, any> | null> {
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} config dump --json`);
      if (errno === 0 && stdout.trim()) {
        return JSON.parse(stdout.trim());
      }
    } catch (e) {
      console.error('[ZM-API] configDump() error:', e);
    }
    return null;
  },

  async configGet(key: string): Promise<string | null> {
    if (shouldUseMock()) {
      return (await getMock()).configGet(key);
    }
    try {
      const { errno, stdout } = await ksuExec(
        `${PATHS.BINARY} config get ${escapeShellArg(key)}`
      );
      if (errno === 0 && stdout.trim()) {
        return stdout.trim();
      }
    } catch (e) {
      console.error('[ZM-API] configGet() error:', e);
    }
    return null;
  },

  async configSet(key: string, value: string): Promise<void> {
    if (shouldUseMock()) {
      return (await getMock()).configSet(key, value);
    }
    const { errno, stderr } = await ksuExec(
      `${PATHS.BINARY} config set ${escapeShellArg(key)} ${escapeShellArg(value)}`
    );
    if (errno !== 0) {
      throw new Error(stderr || `Failed to set config: ${key}`);
    }
  },

  async setSusfsAvcSpoofing(enabled: boolean): Promise<void> {
    if (shouldUseMock()) {
      console.log(`[ZM-API] mock: ksu_susfs enable_avc_log_spoofing ${enabled ? 1 : 0}`);
      return;
    }
    await ksuExec(`ksu_susfs enable_avc_log_spoofing ${enabled ? 1 : 0}`);
  },

  async logActivity(type: string, message: string): Promise<void> {
    return logActivity(type, message);
  },

  async setSusfsLog(enabled: boolean): Promise<void> {
    if (shouldUseMock()) return;
    await ksuExec(`ksu_susfs enable_log ${enabled ? 1 : 0}`);
  },

  async setSusfsHideMounts(enabled: boolean): Promise<void> {
    if (shouldUseMock()) return;
    // v2.0.0+ uses hide_sus_mnts_for_all_procs, fallback to non_su_procs
    const { errno } = await ksuExec(`ksu_susfs hide_sus_mnts_for_all_procs ${enabled ? 1 : 0}`);
    if (errno !== 0) {
      await ksuExec(`ksu_susfs hide_sus_mnts_for_non_su_procs ${enabled ? 1 : 0}`);
    }
  },

  async writeSusfsConfigVar(key: string, value: string): Promise<void> {
    if (shouldUseMock()) return;
    const configPath = '/data/adb/susfs4ksu/config.sh';
    await ksuExec(`sed -i 's/^${escapeShellArg(key)}=.*/${escapeShellArg(key)}=${escapeShellArg(value)}/' ${escapeShellArg(configPath)}`);
  },

  async readSusfsConfigVar(key: string): Promise<string | null> {
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(
        `grep -m1 '^${escapeShellArg(key)}=' /data/adb/susfs4ksu/config.sh | cut -d= -f2`
      );
      if (errno === 0 && stdout.trim()) return stdout.trim();
    } catch (e) {
      console.error('[ZM-API] readSusfsConfigVar() error:', e);
    }
    return null;
  },

  async webuiInit(): Promise<WebUiInitResponse | null> {
    console.log('[ZM-API] webuiInit() called');
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} webui-init`, 10000);
      if (errno === 0 && stdout.trim()) {
        const parsed = JSON.parse(stdout.trim()) as WebUiInitResponse;
        console.log('[ZM-API] webuiInit() success');
        return parsed;
      }
      console.log('[ZM-API] webuiInit() command failed, errno:', errno);
    } catch (e) {
      console.error('[ZM-API] webuiInit() error:', e);
    }
    return null;
  },
};
