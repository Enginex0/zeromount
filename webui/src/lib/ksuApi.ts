import { listPackages as ksuListPackages, getPackagesInfo as ksuGetPackagesInfo } from 'kernelsu';
export { spawn } from 'kernelsu';

interface PackagesInfo {
  packageName: string;
  versionName: string;
  versionCode: number;
  appLabel: string;
  isSystem: boolean;
  uid: number;
}

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

let execCounter = 0;

const VALID_PACKAGE_PATTERN = /^[a-zA-Z][a-zA-Z0-9_.]*$/;

function isValidPackageName(name: string): boolean {
  return VALID_PACKAGE_PATTERN.test(name) && name.length <= 256;
}

export async function ksuExec(cmd: string, options?: { cwd?: string; env?: Record<string, string>; timeout?: number }): Promise<KsuExecResult> {
  const ksu = globalThis.ksu;
  if (!ksu?.exec) {
    return { errno: -1, stdout: '', stderr: 'KSU not available' };
  }

  const timeoutMs = options?.timeout ?? 30000;
  const execOpts: Record<string, unknown> = {};
  if (options?.cwd) execOpts.cwd = options.cwd;
  if (options?.env) execOpts.env = options.env;

  return new Promise((resolve) => {
    const callbackName = `ksu_api_cb_${Date.now()}_${execCounter++}`;
    const win = window as unknown as Record<string, unknown>;

    const timeoutId = setTimeout(() => {
      delete win[callbackName];
      resolve({ errno: -1, stdout: '', stderr: 'timeout' });
    }, timeoutMs);

    win[callbackName] = (errno: number, stdout: string, stderr: string) => {
      clearTimeout(timeoutId);
      delete win[callbackName];
      resolve({ errno, stdout, stderr });
    };

    try {
      ksu.exec(cmd, JSON.stringify(execOpts), callbackName);
    } catch {
      clearTimeout(timeoutId);
      delete win[callbackName];
      resolve({ errno: -1, stdout: '', stderr: 'exec failed' });
    }
  });
}

export async function listPackages(type: 'all' | 'user' | 'system'): Promise<string[]> {
  if (globalThis.ksu?.listPackages) {
    try {
      const result = ksuListPackages(type);
      if (Array.isArray(result) && result.length > 0) return result;
    } catch { /* fallback */ }
  }

  const pmFlags = { all: '', user: '-3', system: '-s' };
  const { stdout, errno } = await ksuExec(`pm list packages ${pmFlags[type]} | sed 's/package://'`);
  if (errno === 0 && stdout.trim()) {
    return stdout.trim().split('\n').filter(Boolean);
  }
  return [];
}

export async function getPackagesInfo(packageNames: string[]): Promise<PackagesInfo[]> {
  if (!packageNames.length) return [];

  if (globalThis.ksu?.getPackagesInfo) {
    try {
      const result = ksuGetPackagesInfo(packageNames);
      if (Array.isArray(result) && result.length > 0) return result;
    } catch { /* fallback */ }
  }

  const valid = packageNames.filter(isValidPackageName);
  const invalid = packageNames.filter(p => !isValidPackageName(p));

  const results: PackagesInfo[] = invalid.map(packageName => ({
    packageName, appLabel: packageName, versionName: '', versionCode: 0, isSystem: false, uid: -1,
  }));

  if (valid.length > 0) {
    const script = valid.map(pkg =>
      `label=$(pm path ${pkg} 2>/dev/null | head -1 | sed 's/package://' | xargs -I{} aapt dump badging {} 2>/dev/null | grep "application-label:" | head -1 | sed "s/application-label:'\\(.*\\)'/\\1/"); printf '%s\\t%s\\n' ${pkg} "\${label:-}"`
    ).join('\n');
    const { stdout } = await ksuExec(script);
    const lines = stdout.trim().split('\n').filter(Boolean);
    const labelMap = new Map(lines.map(l => { const [pkg, ...rest] = l.split('\t'); return [pkg, rest.join('\t')]; }));
    for (const packageName of valid) {
      const appLabel = labelMap.get(packageName)?.trim() || packageName;
      results.push({ packageName, appLabel, versionName: '', versionCode: 0, isSystem: false, uid: -1 });
    }
  }

  return results;
}

export async function ksuWriteFile(content: string, path: string): Promise<KsuExecResult> {
  const escaped = content.replace(/'/g, "'\\''");
  return ksuExec(`printf '%s' '${escaped}' > '${path}'`);
}

export async function getAppLabelViaAapt(packageName: string): Promise<string | null> {
  if (!isValidPackageName(packageName)) return null;
  const { stdout, errno } = await ksuExec(
    `pm path ${packageName} 2>/dev/null | head -1 | sed 's/package://' | xargs -I{} aapt dump badging {} 2>/dev/null | grep "application-label:" | head -1 | sed "s/application-label:'\\(.*\\)'/\\1/"`
  );
  return errno === 0 && stdout.trim() ? stdout.trim() : null;
}
