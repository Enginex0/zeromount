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
  try {
    const { errno: listErr, stdout: listOut } = await ksuExec(`cat "${PATHS.EXCLUSION_FILE}"`);
    if (listErr !== 0 || !listOut.trim()) {
      return [];
    }

    const uids = listOut.trim().split('\n').map(line => parseInt(line.trim(), 10)).filter(uid => !isNaN(uid));

    let meta: ExclusionMeta = {};
    try {
      const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}"`);
      if (metaErr === 0 && metaOut.trim()) {
        meta = JSON.parse(metaOut);
      }
    } catch (e) {
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
    return result;
  } catch (e) {
    return [];
  }
}

async function parseActivityLog(): Promise<ActivityItem[]> {
  try {
    const { errno, stdout } = await ksuExec(`tail -10 "${PATHS.ACTIVITY_LOG}"`);
    if (errno !== 0 || !stdout.trim()) {
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
    return items.slice(0, 10);
  } catch (e) {
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
  }
}

export const api = {
  async getVersion(): Promise<string> {
    if (shouldUseMock()) {
      return (await getMock()).getVersion();
    }
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} version`);
      if (errno === 0 && stdout) {
        return stdout.trim();
      }
    } catch (e) {
      // Fallback
    }
    return `v${APP_VERSION}`;
  },

  async getSystemInfo(): Promise<SystemInfo> {
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
    } catch (e) {
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
    }

    return info;
  },

  async getRules(): Promise<VfsRule[]> {
    if (shouldUseMock()) {
      return (await getMock()).getRules();
    }
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} vfs list`);
      if (errno === 0 && stdout.trim()) {
        return parseRulesOutput(stdout);
      }
    } catch (e) {
      // Fallback
    }
    return [];
  },

  async clearAllRules(): Promise<void> {
    if (shouldUseMock()) {
      return (await getMock()).clearAllRules();
    }

    const cmd = `${PATHS.BINARY} vfs clear`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(stderr || 'Failed to clear rules');
    }
  },

  async getExcludedUids(): Promise<ExcludedUid[]> {
    if (shouldUseMock()) {
      return (await getMock()).getExcludedUids();
    }
    return parseExclusionFiles();
  },

  async excludeUid(uid: number, packageName: string, appName: string): Promise<ExcludedUid> {
    if (shouldUseMock()) {
      return (await getMock()).excludeUid(uid, packageName, appName);
    }

    // Validation FIRST
    if (!Number.isInteger(uid) || uid < 0) throw new Error('Invalid UID');

    const cmd = `${PATHS.BINARY} uid block ${escapeShellArg(String(uid))}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(stderr || 'Failed to exclude UID');
    }

    await ksuExec(`echo ${escapeShellArg(String(uid))} >> "${PATHS.EXCLUSION_FILE}"`).catch(() => {});

    await withMetaLock(async () => {
      try {
        let meta: Record<string, { packageName: string; appName: string; excludedAt: string }> = {};
        const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
        if (metaErr === 0 && metaOut.trim()) {
          try {
            meta = JSON.parse(metaOut);
          } catch (parseErr) {
            meta = {};
          }
        }
        meta[String(uid)] = { packageName, appName, excludedAt: new Date().toISOString() };
        await ksuExec(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
      } catch (e) {
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
    if (shouldUseMock()) {
      return (await getMock()).includeUid(uid);
    }

    const cmd = `${PATHS.BINARY} uid unblock ${escapeShellArg(String(uid))}`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(stderr || 'Failed to include UID');
    }

    if (!/^\d+$/.test(String(uid))) throw new Error('invalid uid');
    await ksuExec(`sed -i '/^${uid}$/d' "${PATHS.EXCLUSION_FILE}"`).catch(() => {});

    await withMetaLock(async () => {
      try {
        const { errno: metaErr, stdout: metaOut } = await ksuExec(`cat "${PATHS.EXCLUSION_META}" 2>/dev/null`);
        if (metaErr === 0 && metaOut.trim()) {
          try {
            const meta = JSON.parse(metaOut);
            delete meta[String(uid)];
            await ksuExec(`echo ${escapeShellArg(JSON.stringify(meta))} > "${PATHS.EXCLUSION_META}"`);
          } catch (parseErr) {
          }
        }
      } catch (e) {
      }
    });

  },

  async getActivity(): Promise<ActivityItem[]> {
    if (shouldUseMock()) {
      return (await getMock()).getActivity();
    }
    return parseActivityLog();
  },

  async getStats(): Promise<EngineStats> {
    if (shouldUseMock()) {
      return (await getMock()).getStats();
    }

    const [rules, uids] = await Promise.all([
      this.getRules(),
      this.getExcludedUids(),
    ]);

    return {
      activeRules: rules.length,
      excludedUids: uids.length,
      hiddenPaths: 0,
      hiddenMaps: 0,
    };
  },

  async toggleEngine(enable: boolean): Promise<void> {
    if (shouldUseMock()) {
      return (await getMock()).toggleEngine(enable);
    }

    const cmd = enable ? `${PATHS.BINARY} vfs enable` : `${PATHS.BINARY} vfs disable`;
    const { errno, stderr } = await ksuExec(cmd);
    if (errno !== 0) {
      throw new Error(stderr || 'Failed to toggle engine');
    }
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

  async reboot(): Promise<void> {
    await ksuExec('svc power reboot');
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

  async getVerboseDumpPath(): Promise<string | null> {
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(`cat /data/adb/zeromount/.dump_path 2>/dev/null`);
      if (errno === 0 && stdout.trim()) return stdout.trim();
    } catch (e) {
    }
    return null;
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
      return modules;
    } catch (e) {
      return [];
    }
  },

  async loadKsuModule(moduleName: string, modulePath: string): Promise<number> {
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
      return addedCount;
    } catch (e) {
      throw e;
    }
  },

  async unloadKsuModule(moduleName: string, modulePath: string): Promise<number> {
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
      return removedCount;
    } catch (e) {
      throw e;
    }
  },

  async fetchSystemColor(): Promise<string | null> {
    if (shouldUseMock()) {
      return (await getMock()).fetchSystemColor();
    }
    try {
      const { errno, stdout } = await ksuExec(
        'settings get secure theme_customization_overlay_packages'
      );
      if (errno !== 0 || !stdout.trim()) return null;
      const match = /#?([0-9a-fA-F]{6,8})/i.exec(stdout);
      if (match) return '#' + match[1].slice(0, 6);
      return null;
    } catch (e) {
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
    if (shouldUseMock()) return;
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

  async setKernelUmount(enabled: boolean): Promise<void> {
    if (shouldUseMock()) return;
    const ksud = '/data/adb/ksu/bin/ksud';
    const apKsud = '/data/adb/ap/bin/ksud';
    await ksuExec(`{ [ -x ${ksud} ] && ${ksud} feature set kernel_umount ${enabled ? 1 : 0}; } || { [ -x ${apKsud} ] && ${apKsud} feature set kernel_umount ${enabled ? 1 : 0}; } || true`);
  },

  async writeSusfsConfigVar(key: string, value: string): Promise<void> {
    if (shouldUseMock()) return;
    const configPath = '/data/adb/susfs4ksu/config.sh';
    await ksuExec(`sed -i 's/^${escapeShellArg(key)}=.*/${escapeShellArg(key)}=${escapeShellArg(value)}/' ${escapeShellArg(configPath)}`);
  },

  async bridgeWrite(_key: string, _value: string): Promise<void> {
    if (shouldUseMock()) return;
    await ksuExec(`${PATHS.BINARY} bridge write`);
  },

  async readSusfsConfigVar(key: string, basePath = '/data/adb/susfs4ksu/config.sh'): Promise<string | null> {
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(
        `grep -m1 '^${escapeShellArg(key)}=' ${escapeShellArg(basePath)} | cut -d= -f2`
      );
      if (errno === 0 && stdout.trim()) return stdout.trim();
    } catch (e) {
    }
    return null;
  },

  async readAllBridgeValues(basePath: string): Promise<Record<string, string>> {
    if (shouldUseMock()) return {};
    try {
      const { errno, stdout } = await ksuExec(
        `grep -E '^[a-zA-Z_]+=.' ${escapeShellArg(basePath)} 2>/dev/null`
      );
      if (errno !== 0 || !stdout.trim()) return {};
      const result: Record<string, string> = {};
      for (const line of stdout.trim().split('\n')) {
        const eq = line.indexOf('=');
        if (eq === -1) continue;
        result[line.slice(0, eq)] = line.slice(eq + 1);
      }
      return result;
    } catch (e) {
      console.error('[ZM-API] readAllBridgeValues() error:', e);
      return {};
    }
  },

  async webuiInit(): Promise<WebUiInitResponse | null> {
    if (shouldUseMock()) return null;
    try {
      const { errno, stdout } = await ksuExec(`${PATHS.BINARY} webui-init`, 10000);
      if (errno === 0 && stdout.trim()) {
        return JSON.parse(stdout.trim()) as WebUiInitResponse;
      }
    } catch (e) {
    }
    return null;
  },
};
