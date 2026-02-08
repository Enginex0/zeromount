import type { KsuPackageInfo, KsuPackageIcon } from './ksu.d.ts';

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

let execCounter = 0;

const VALID_PACKAGE_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.]*$/;

function isKsuPackageInfo(obj: unknown): obj is KsuPackageInfo {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'packageName' in obj &&
    typeof (obj as KsuPackageInfo).packageName === 'string'
  );
}

function isKsuPackageIcon(obj: unknown): obj is KsuPackageIcon {
  return (
    typeof obj === 'object' &&
    obj !== null &&
    'packageName' in obj &&
    'icon' in obj &&
    typeof (obj as KsuPackageIcon).packageName === 'string' &&
    typeof (obj as KsuPackageIcon).icon === 'string'
  );
}

function isValidPackageName(name: string): boolean {
  return VALID_PACKAGE_PATTERN.test(name) && name.length <= 256;
}

async function ksuExec(cmd: string, timeoutMs = 30000): Promise<KsuExecResult> {
  const ksu = globalThis.ksu;
  if (!ksu?.exec) {
    return { errno: -1, stdout: '', stderr: 'KSU not available' };
  }

  return new Promise((resolve) => {
    const callbackName = `ksu_api_cb_${Date.now()}_${execCounter++}` as const;

    const timeoutId = setTimeout(() => {
      delete window[callbackName];
      resolve({ errno: -1, stdout: '', stderr: 'timeout' });
    }, timeoutMs);

    window[callbackName] = (errno: number, stdout: string, stderr: string) => {
      clearTimeout(timeoutId);
      delete window[callbackName];
      resolve({ errno, stdout, stderr });
    };

    try {
      ksu.exec(cmd, '{}', callbackName);
    } catch {
      clearTimeout(timeoutId);
      delete window[callbackName];
      resolve({ errno: -1, stdout: '', stderr: 'exec failed' });
    }
  });
}

export async function listPackages(type: 'all' | 'user' | 'system'): Promise<string[]> {
  const ksu = globalThis.ksu;

  const methodMap = { all: 'listAllPackages', user: 'listUserPackages', system: 'listSystemPackages' } as const;
  const methodName = methodMap[type];

  if (ksu?.[methodName]) {
    try {
      const result = (ksu[methodName] as () => string)();
      if (result) {
        const parsed = JSON.parse(result);
        if (Array.isArray(parsed)) return parsed;
      }
    } catch { /* fallback */ }
  }

  const pmFlags = { all: '', user: '-3', system: '-s' };
  const { stdout, errno } = await ksuExec(`pm list packages ${pmFlags[type]} | sed 's/package://'`);
  if (errno === 0 && stdout.trim()) {
    return stdout.trim().split('\n').filter(Boolean);
  }

  return [];
}

export async function getPackagesInfo(packageNames: string[]): Promise<KsuPackageInfo[]> {
  if (!packageNames.length) return [];

  const ksu = globalThis.ksu;

  if (ksu?.getPackagesInfo) {
    try {
      const result = ksu.getPackagesInfo(JSON.stringify(packageNames));
      if (result) {
        const parsed: unknown = JSON.parse(result);
        if (Array.isArray(parsed) && parsed.every(isKsuPackageInfo)) {
          return parsed;
        }
      }
    } catch { /* fallback */ }
  }

  // Full shell fallback only when KSU API unavailable
  const results: KsuPackageInfo[] = [];
  for (const packageName of packageNames) {
    if (!isValidPackageName(packageName)) {
      results.push({ packageName, appLabel: packageName });
      continue;
    }
    const { stdout, errno } = await ksuExec(
      `pm path ${packageName} 2>/dev/null | head -1 | sed 's/package://' | xargs -I{} aapt dump badging {} 2>/dev/null | grep "application-label:" | head -1 | sed "s/application-label:'\\(.*\\)'/\\1/"`
    );
    results.push({
      packageName,
      appLabel: errno === 0 && stdout.trim() ? stdout.trim() : packageName,
    });
  }
  return results;
}

// Fetch label for a single app via aapt (used for newly installed apps)
export async function getAppLabelViaAapt(packageName: string): Promise<string | null> {
  if (!isValidPackageName(packageName)) return null;
  const { stdout, errno } = await ksuExec(
    `pm path ${packageName} 2>/dev/null | head -1 | sed 's/package://' | xargs -I{} aapt dump badging {} 2>/dev/null | grep "application-label:" | head -1 | sed "s/application-label:'\\(.*\\)'/\\1/"`
  );
  return errno === 0 && stdout.trim() ? stdout.trim() : null;
}

export async function getPackagesIcons(
  packageNames: string[],
  size = 100
): Promise<KsuPackageIcon[]> {
  if (!packageNames.length) return [];

  const ksu = globalThis.ksu;

  if (ksu?.getPackagesIcons) {
    try {
      const result = ksu.getPackagesIcons(JSON.stringify(packageNames), size);
      if (result) {
        const parsed: unknown = JSON.parse(result);
        if (Array.isArray(parsed) && parsed.every(isKsuPackageIcon)) {
          return parsed;
        }
      }
    } catch { /* fallback */ }
  }

  // No shell fallback for icons - native API required
  return packageNames.map((packageName) => ({
    packageName,
    icon: '',
  }));
}
