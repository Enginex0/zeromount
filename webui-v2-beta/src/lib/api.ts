import type { VfsRule, ExcludedUid, SystemInfo, ActivityItem, EngineStats, MountedModule, InstalledApp, KsuModule } from './types';
import type { KsuNativeApi } from './ksu.d.ts';
import { PATHS, APP_VERSION } from './constants';
import { MockAPI } from './api.mock';

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

function escapeShellArg(arg: string): string {
  return "'" + arg.replace(/'/g, "'\\''") + "'";
}

// KSU exec with proper callback pattern
let execCounter = 0;

export function shouldUseMock(): boolean {
  return typeof globalThis.ksu === 'undefined';
}

async function execCommand(cmd: string, timeoutMs = 30000): Promise<KsuExecResult> {
  const ksu = globalThis.ksu;
  if (!ksu?.exec) {
    console.error('[ZM-API] execCommand() FAILED: KSU not available');
    throw new Error('KSU not available');
  }

  return new Promise((resolve, reject) => {
    const callbackName = `exec_cb_${Date.now()}_${execCounter++}`;

    const timeoutId = setTimeout(() => {
      delete (window as any)[callbackName];
      reject(new Error(`Command timed out: ${cmd.substring(0, 50)}...`));
    }, timeoutMs);

    (window as any)[callbackName] = (errno: number, stdout: string, stderr: string) => {
      clearTimeout(timeoutId);
      delete (window as any)[callbackName];
      resolve({ errno, stdout, stderr });
    };

    try {
      ksu.exec(cmd, '{}', callbackName);
    } catch (e) {
      clearTimeout(timeoutId);
      delete (window as any)[callbackName];
      reject(e);
    }
  });
}


function parseRulesOutput(stdout: string): VfsRule[] {
  console.log('[ZM-API] parseRulesOutput() input:', stdout?.slice(0, 200));
  const lines = stdout.trim().split('\n').filter(line => line.trim());
  const rules = lines.map((line, index) => {
    const idx = line.indexOf('->');
    const source = idx === -1 ? line.trim() : line.slice(0, idx).trim();
    const target = idx === -1 ? '[BLOCKED]' : line.slice(idx + 2).trim();
    return {
      id: String(index + 1),
      name: source.split('/').pop() || 'Rule',
      source,
      target,
      hits: 0,
      createdAt: new Date(),
      active: true,
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

async function parseExclusionFiles(): Promise<ExcludedUid[]> {
  console.log('[ZM-API] parseExclusionFiles() called');
  try {
    const { errno: listErr, stdout: listOut } = await execCommand(`cat "${PATHS.EXCLUSION_FILE}"`);
    console.log('[ZM-API] parseExclusionFiles() list result:', { errno: listErr, stdout: listOut?.slice(0, 100) });
    if (listErr !== 0 || !listOut.trim()) {
      console.log('[ZM-API] parseExclusionFiles() no exclusions found');
      return [];
    }

    const uids = listOut.trim().split('\n').map(line => parseInt(line.trim(), 10)).filter(uid => !isNaN(uid));
    console.log('[ZM-API] parseExclusionFiles() parsed UIDs:', uids);

    let meta: ExclusionMeta = {};
    try {
      const { errno: metaErr, stdout: metaOut } = await execCommand(`cat "${PATHS.EXCLUSION_META}"`);
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
    const { errno, stdout } = await execCommand(`tail -50 "${PATHS.ACTIVITY_LOG}"`);
    console.log('[ZM-API] parseActivityLog() result:', { errno, stdout: stdout?.slice(0, 100) });
    if (errno !== 0 || !stdout.trim()) {
      console.log('[ZM-API] parseActivityLog() no activity found');
      return [];
    }

    const validTypes = ['rule_added', 'rule_removed', 'uid_excluded', 'uid_included', 'engine_enabled', 'engine_disabled'];
    const lines = stdout.trim().split('\n').filter(line => line.trim());
    const items = lines.map((line, index) => {
      const match = line.match(/^\[(.+?)\]\s+(\w+):\s+(.+)$/);
      if (match) {
        const [, timestamp, type, message] = match;
        const normalizedType = type.toLowerCase();
        return {
          id: String(index + 1),
          type: (validTypes.includes(normalizedType) ? normalizedType : 'engine_enabled') as ActivityItem['type'],
          message,
          timestamp: new Date(timestamp),
        };
      }
      return {
        id: String(index + 1),
        type: 'engine_enabled' as const,
        message: line,
        timestamp: new Date(),
      };
    });
    console.log('[ZM-API] parseActivityLog() parsed', items.length, 'activity items');
    return items;
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
    await execCommand(`echo ${escapeShellArg(line)} >> "${PATHS.ACTIVITY_LOG}"`);
  } catch (e) {
    console.error('[ZM-API] logActivity() error:', e);
  }
}

export const api = {
  async getVersion(): Promise<string> {
    console.log('[ZM-API] getVersion() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getVersion();
    }
    try {
      const { errno, stdout } = await execCommand(`${PATHS.BINARY} ver`);
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
      return MockAPI.getSystemInfo();
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
        execCommand(`${PATHS.BINARY} ver`).catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('uname -r').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('cat /proc/uptime').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('getprop ro.product.model').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('getprop ro.build.version.release').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('getenforce').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
        execCommand('ksu_susfs show version 2>/dev/null || echo ""').catch(() => ({ errno: 1, stdout: '', stderr: '' })),
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

  async getRules(): Promise<VfsRule[]> {
    console.log('[ZM-API] getRules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getRules();
    }
    try {
      const { errno, stdout } = await execCommand(`${PATHS.BINARY} list`);
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

  async addRule(name: string, source: string, target: string): Promise<VfsRule> {
    console.log('[ZM-API] addRule() called:', { name, source, target, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return MockAPI.addRule(name, source, target);
    }

    const cmd = `${PATHS.BINARY} add ${escapeShellArg(source)} ${escapeShellArg(target)}`;
    console.log('[ZM-API] addRule() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] addRule() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to add rule');
    }

    const newRule = {
      id: Date.now().toString(),
      name,
      source,
      target,
      hits: 0,
      createdAt: new Date(),
      active: true,
    };
    console.log('[ZM-API] addRule() success, returning:', newRule.id);
    await logActivity('RULE_ADDED', `${name}: ${source} -> ${target}`);
    return newRule;
  },

  async deleteRule(sourcePath: string): Promise<void> {
    console.log('[ZM-API] deleteRule() called:', { sourcePath, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return MockAPI.deleteRule(sourcePath);
    }

    const cmd = `${PATHS.BINARY} del ${escapeShellArg(sourcePath)}`;
    console.log('[ZM-API] deleteRule() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] deleteRule() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to delete rule');
    }
    console.log('[ZM-API] deleteRule() success');
    await logActivity('RULE_REMOVED', sourcePath);
  },

  async clearAllRules(): Promise<void> {
    console.log('[ZM-API] clearAllRules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.clearAllRules();
    }

    const cmd = `${PATHS.BINARY} clear`;
    console.log('[ZM-API] clearAllRules() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] clearAllRules() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to clear rules');
    }
    console.log('[ZM-API] clearAllRules() success');
    await logActivity('RULES_CLEARED', 'All rules cleared');
  },

  async getExcludedUids(): Promise<ExcludedUid[]> {
    console.log('[ZM-API] getExcludedUids() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getExcludedUids();
    }
    const result = await parseExclusionFiles();
    console.log('[ZM-API] getExcludedUids() returning', result.length, 'exclusions');
    return result;
  },

  async excludeUid(uid: number, packageName: string, appName: string): Promise<ExcludedUid> {
    console.log('[ZM-API] excludeUid() called:', { uid, packageName, appName, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return MockAPI.excludeUid(uid, packageName, appName);
    }

    // Validation FIRST
    if (!Number.isInteger(uid) || uid < 0) throw new Error('Invalid UID');

    const cmd = `${PATHS.BINARY} blk ${escapeShellArg(String(uid))}`;
    console.log('[ZM-API] excludeUid() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] excludeUid() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to exclude UID');
    }

    console.log('[ZM-API] excludeUid() success');

    await execCommand(`echo ${escapeShellArg(String(uid))} >> "${PATHS.EXCLUSION_FILE}"`).catch(e => console.error('[ZM-API] Exclusion persistence failed:', e));

    try {
      let meta: Record<string, { packageName: string; appName: string; excludedAt: string }> = {};
      const { errno: metaErr, stdout: metaOut } = await execCommand(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
      if (metaErr === 0 && metaOut.trim()) {
        try {
          meta = JSON.parse(metaOut);
        } catch (parseErr) {
          console.error('[ZM-API] Metadata parse error, starting fresh:', parseErr);
          meta = {};
        }
      }
      meta[String(uid)] = { packageName, appName, excludedAt: new Date().toISOString() };
      await execCommand(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
    } catch (e) {
      console.error('[ZM-API] Metadata save failed:', e);
    }

    await logActivity('UID_EXCLUDED', `${appName} (${uid})`);
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
      return MockAPI.includeUid(uid);
    }

    const cmd = `${PATHS.BINARY} unb ${escapeShellArg(String(uid))}`;
    console.log('[ZM-API] includeUid() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] includeUid() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to include UID');
    }
    console.log('[ZM-API] includeUid() success');

    await execCommand(`sed -i '/^${uid}$/d' "${PATHS.EXCLUSION_FILE}"`).catch(e => console.error('[ZM-API] Exclusion removal failed:', e));

    try {
      const { errno: metaErr, stdout: metaOut } = await execCommand(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
      if (metaErr === 0 && metaOut.trim()) {
        try {
          const meta = JSON.parse(metaOut);
          delete meta[String(uid)];
          await execCommand(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
        } catch (parseErr) {
          console.error('[ZM-API] Metadata parse error during cleanup:', parseErr);
        }
      }
    } catch (e) {
      console.error('[ZM-API] Metadata cleanup failed:', e);
    }

    await logActivity('UID_INCLUDED', `UID ${uid}`);
  },

  async getActivity(): Promise<ActivityItem[]> {
    console.log('[ZM-API] getActivity() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getActivity();
    }
    const result = await parseActivityLog();
    console.log('[ZM-API] getActivity() returning', result.length, 'items');
    return result;
  },

  async getStats(): Promise<EngineStats> {
    console.log('[ZM-API] getStats() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getStats();
    }

    const [rules, uids] = await Promise.all([
      this.getRules(),
      this.getExcludedUids(),
    ]);

    const stats = {
      activeRules: rules.length,
      excludedUids: uids.length,
      hitsToday: rules.reduce((sum, r) => sum + r.hits, 0),
    };
    console.log('[ZM-API] getStats() returning:', stats);
    return stats;
  },

  async toggleEngine(enable: boolean): Promise<void> {
    console.log('[ZM-API] toggleEngine() called:', { enable, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return MockAPI.toggleEngine(enable);
    }

    const cmd = enable ? `${PATHS.BINARY} enable` : `${PATHS.BINARY} disable`;
    console.log('[ZM-API] toggleEngine() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] toggleEngine() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to toggle engine');
    }
    console.log('[ZM-API] toggleEngine() success');
    await logActivity(enable ? 'ENGINE_ENABLED' : 'ENGINE_DISABLED', enable ? 'Engine activated' : 'Engine deactivated');
  },

  async setVerboseLogging(enabled: boolean): Promise<void> {
    console.log('[ZM-API] setVerboseLogging() called:', { enabled, mock: shouldUseMock() });
    if (shouldUseMock()) {
      return MockAPI.setVerboseLogging(enabled);
    }

    const cmd = enabled
      ? `touch "${PATHS.VERBOSE_FLAG}"`
      : `rm -f "${PATHS.VERBOSE_FLAG}"`;
    console.log('[ZM-API] setVerboseLogging() executing:', cmd);
    const { errno, stderr } = await execCommand(cmd);
    if (errno !== 0) {
      console.error('[ZM-API] setVerboseLogging() failed:', { errno, stderr });
      throw new Error(stderr || 'Failed to set verbose logging');
    }
    console.log('[ZM-API] setVerboseLogging() success');
  },

  async getModules(): Promise<MountedModule[]> {
    console.log('[ZM-API] getModules() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getModules();
    }

    try {
      const { errno, stdout } = await execCommand(`ls "${PATHS.MODULE_PATHS}" 2>/dev/null`);
      if (errno !== 0 || !stdout.trim()) {
        console.log('[ZM-API] getModules() no modules found');
        return [];
      }

      const names = stdout.trim().split('\n').filter(Boolean);
      console.log('[ZM-API] getModules() found', names.length, 'module names');

      const modules = await Promise.all(
        names.map(async (name) => {
          const fullPath = `${PATHS.MODULE_PATHS}/${name}`;
          const { stdout: countOut } = await execCommand(
            `wc -l < ${escapeShellArg(fullPath)} 2>/dev/null || echo 0`
          );
          return {
            name,
            ruleCount: parseInt(countOut.trim(), 10) || 0,
          };
        })
      );

      console.log('[ZM-API] getModules() returning', modules.length, 'modules');
      return modules;
    } catch (e) {
      console.error('[ZM-API] getModules() error:', e);
      return [];
    }
  },

  async isEngineActive(): Promise<boolean> {
    console.log('[ZM-API] isEngineActive() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.isEngineActive();
    }
    try {
      const { errno } = await execCommand(`[ -e "${PATHS.DEVICE}" ]`);
      const isActive = errno === 0;
      console.log('[ZM-API] isEngineActive() returning:', isActive);
      return isActive;
    } catch (e) {
      console.error('[ZM-API] isEngineActive() error:', e);
      return false;
    }
  },

  async getInstalledApps(): Promise<InstalledApp[]> {
    console.log('[ZM-API] getInstalledApps() called, mock:', shouldUseMock());
    if (shouldUseMock()) {
      return MockAPI.getInstalledApps();
    }

    try {
      const response = await fetch('link/installed_apps.json?t=' + Date.now());
      if (response.ok) {
        const text = await response.text();
        try {
          const apps = JSON.parse(text) as InstalledApp[];
          if (Array.isArray(apps)) {
            console.log('[ZM-API] getInstalledApps() loaded', apps.length, 'apps');
            return apps;
          }
        } catch (parseErr) {
          console.error('[ZM-API] JSON parse error in getInstalledApps:', parseErr);
        }
      }
    } catch (e) {
      console.log('[ZM-API] getInstalledApps() file not ready');
    }
    return [];
  },

  async refreshInstalledApps(): Promise<void> {
    // No-op: App list is now managed by app_monitor.sh daemon and loaded via getInstalledApps()
    console.log('[ZM-API] refreshInstalledApps() called (no-op)');
  },

  async checkAppListStale(cachedCount: number): Promise<boolean> {
    console.log('[ZM-API] checkAppListStale() cached:', cachedCount);
    if (shouldUseMock()) return false;

    try {
      const { errno, stdout } = await execCommand('pm list packages -3 | wc -l');
      if (errno === 0) {
        const currentCount = parseInt(stdout.trim(), 10);
        console.log('[ZM-API] checkAppListStale() current:', currentCount);
        return currentCount !== cachedCount;
      }
    } catch (e) {
      console.error('[ZM-API] checkAppListStale() error:', e);
    }
    return false;
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
      return MockAPI.scanKsuModules();
    }

    try {
      // Single shell call that outputs JSON - replaces ~22 separate exec calls
      const script = `
# Extract unique module base paths from zm list sources (format: source->target)
loaded_modules=$(${PATHS.BINARY} list 2>/dev/null | awk -F'->' '{print $1}' | grep -oE '/data/adb/modules/[^/]+' | sort -u || echo "")
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
      const { errno, stdout } = await execCommand(script);
      if (errno !== 0) {
        console.log('[ZM-API] scanKsuModules() script failed');
        return [];
      }

      const modules = JSON.parse(stdout.trim()) as KsuModule[];
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
      return MockAPI.loadKsuModule(moduleName, modulePath);
    }

    try {
      const { stdout } = await execCommand(
        `find ${escapeShellArg(modulePath)} -type f \\( -path "*/system/*" -o -path "*/vendor/*" -o -path "*/product/*" \\) 2>/dev/null`
      );
      const files = stdout.trim().split('\n').filter(Boolean);
      let addedCount = 0;

      for (const filePath of files) {
        const relativePath = filePath.replace(modulePath, '');
        let targetPath = '';

        if (relativePath.startsWith('/system/')) {
          targetPath = relativePath;
        } else if (relativePath.startsWith('/vendor/')) {
          targetPath = relativePath;
        } else if (relativePath.startsWith('/product/')) {
          targetPath = relativePath;
        } else {
          continue;
        }

        const cmd = `${PATHS.BINARY} add ${escapeShellArg(filePath)} ${escapeShellArg(targetPath)}`;
        const { errno } = await execCommand(cmd);
        if (errno === 0) addedCount++;
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
      return MockAPI.unloadKsuModule(moduleName, modulePath);
    }

    try {
      const rules = await this.getRules();
      const moduleRules = rules.filter(r => r.source.startsWith(modulePath));
      let removedCount = 0;

      for (const rule of moduleRules) {
        const cmd = `${PATHS.BINARY} del ${escapeShellArg(rule.source)}`;
        const { errno } = await execCommand(cmd);
        if (errno === 0) removedCount++;
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
      return MockAPI.fetchSystemColor();
    }
    try {
      const { errno, stdout } = await execCommand(
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
};
