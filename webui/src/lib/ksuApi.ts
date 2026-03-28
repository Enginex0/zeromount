import { exec, listPackages as altListPackages, getPackagesInfo as altGetPackagesInfo } from 'kernelsu-alt';
import type { ExecResult } from 'kernelsu-alt';

export type { ExecResult };

export interface PackagesInfo {
  packageName: string;
  versionName: string;
  versionCode: number;
  appLabel: string;
  isSystem: boolean;
  uid: number;
}

const VALID_PACKAGE_RE = /^[a-zA-Z][a-zA-Z0-9_.]*$/;

function isValidPackageName(name: string): boolean {
  return VALID_PACKAGE_RE.test(name) && name.length <= 256;
}

export function isRealEnvironment(): boolean {
  return typeof ksu !== 'undefined';
}

export function runShell(command: string): Promise<ExecResult> {
  if (!isRealEnvironment()) return Promise.resolve({ errno: -1, stdout: '', stderr: 'KSU not available' });
  return exec(command);
}

export async function listPackages(type: 'all' | 'user' | 'system'): Promise<string[]> {
  if (!isRealEnvironment()) return [];
  try {
    const result = await altListPackages(type);
    if (Array.isArray(result) && result.length > 0) return result;
  } catch {}

  const pmFlags = { all: '', user: '-3', system: '-s' };
  const { stdout, errno } = await runShell(`pm list packages ${pmFlags[type]} | sed 's/package://'`);
  if (errno === 0 && stdout.trim()) {
    return stdout.trim().split('\n').filter(Boolean);
  }
  return [];
}

export async function getPackagesInfo(packageNames: string[]): Promise<PackagesInfo[]> {
  if (!packageNames.length) return [];

  if (isRealEnvironment()) {
    try {
      const result = await altGetPackagesInfo(packageNames);
      if (Array.isArray(result) && result.length > 0) return result;
    } catch {}
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
    const { stdout } = await runShell(script);
    const lines = stdout.trim().split('\n').filter(Boolean);
    const labelMap = new Map(lines.map(l => { const [pkg, ...rest] = l.split('\t'); return [pkg, rest.join('\t')]; }));
    for (const packageName of valid) {
      const appLabel = labelMap.get(packageName)?.trim() || packageName;
      results.push({ packageName, appLabel, versionName: '', versionCode: 0, isSystem: false, uid: -1 });
    }
  }

  return results;
}

export async function ksuWriteFile(content: string, path: string): Promise<ExecResult> {
  const escaped = content.replace(/'/g, "'\\''");
  return runShell(`printf '%s' '${escaped}' > '${path}'`);
}

export async function getAppLabelViaAapt(packageName: string): Promise<string | null> {
  if (!isValidPackageName(packageName)) return null;
  const { stdout, errno } = await runShell(
    `pm path ${packageName} 2>/dev/null | head -1 | sed 's/package://' | xargs -I{} aapt dump badging {} 2>/dev/null | grep "application-label:" | head -1 | sed "s/application-label:'\\(.*\\)'/\\1/"`
  );
  return errno === 0 && stdout.trim() ? stdout.trim() : null;
}
