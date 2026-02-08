interface KsuPackageInfo {
  packageName: string;
  appLabel: string;
  versionName?: string;
  versionCode?: number;
  uid?: number;
  targetSdkVersion?: number;
  isSystemApp?: boolean;
}

interface KsuPackageIcon {
  packageName: string;
  icon: string;
}

interface KsuNativeApi {
  exec(cmd: string, options: string, callbackName: string): void;

  listAllPackages?(): string;
  listUserPackages?(): string;
  listSystemPackages?(): string;
  getPackagesInfo?(packageNamesJson: string): string;
  getPackagesIcons?(packageNamesJson: string, size: number): string;
}

declare global {
  var ksu: KsuNativeApi | undefined;

  interface Window {
    [key: `ksu_api_cb_${string}`]: ((errno: number, stdout: string, stderr: string) => void) | undefined;
    [key: `exec_cb_${string}`]: ((errno: number, stdout: string, stderr: string) => void) | undefined;
  }
}

export type { KsuNativeApi, KsuPackageInfo, KsuPackageIcon };
