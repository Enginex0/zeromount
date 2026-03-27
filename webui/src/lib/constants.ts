const MODULE_ID = (() => {
  try {
    const id = globalThis.ksu?.moduleInfo?.();
    if (id && /^[a-zA-Z][a-zA-Z0-9._-]+$/.test(id)) return id;
  } catch {}
  return 'meta-zeromount';
})();

export const PATHS = {
  BINARY: `/data/adb/modules/${MODULE_ID}/bin/zm`,
  DATA_DIR: '/data/adb/zeromount/',
  MODULE_PATHS: '/data/adb/zeromount/module_paths',
  EXCLUSION_FILE: '/data/adb/zeromount/.exclusion_list',
  EXCLUSION_META: '/data/adb/zeromount/.exclusion_meta.json',
  ACTIVITY_LOG: '/data/adb/zeromount/activity.log',
  VERBOSE_FLAG: '/data/adb/zeromount/.verbose',
};

export const GITHUB_URL = 'https://github.com/Enginex0/zeromount';

export const APP_VERSION = '2.0.0-dev';
