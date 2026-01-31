# Zero-Mount Reference

Comprehensive reference for the Zero-Mount kernel subsystem and KernelSU metamodule.

**Project Path:** `/home/claudetest/zero-mount/nomount/`

---

## Overview

Zero-Mount is a Linux kernel subsystem providing VFS-level path redirection without filesystem mounts. When an application opens a file, the kernel transparently serves content from a different location.

**Key Distinction:**
- **Zero-Mount:** Path REDIRECTION (returns different file content)
- **SUSFS:** Path HIDING (returns ENOENT)

| Identity | Value |
|----------|-------|
| Name | Zero-Mount (with hyphen) |
| Binary | `zm` |
| Device | `/dev/zeromount` |
| Kernel config | `CONFIG_ZEROMOUNT` |
| Magic byte | `0x5A` ('Z') |

---

## Architecture

### VFS Hook Location

The primary hook intercepts `getname_flags()` in `fs/namei.c`:

```
patches/zeromount-core.patch:269-302
```

```c
struct filename *zeromount_getname_hook(struct filename *name)
{
    // Critical process bypass (PID 1, 2)
    if (zeromount_is_critical_process())
        return name;

    // Disabled, blocked UID, or non-absolute path bypass
    if (ZEROMOUNT_DISABLED() || zeromount_is_uid_blocked(current_uid().val) || !name || name->name[0] != '/')
        return name;

    // Per-CPU recursion guard
    if (this_cpu_inc_return(zeromount_recursion_depth) > 1) {
        this_cpu_dec(zeromount_recursion_depth);
        return name;
    }

    target_path = zeromount_resolve_path(name->name);
    if (!target_path) {
        this_cpu_dec(zeromount_recursion_depth);
        return name;
    }

    new_name = getname_kernel(target_path);
    // ... swap and return
}
```

### Hash Tables (4 Total)

```
patches/zeromount-core.patch:57-60
```

| Hash Table | Purpose | Key |
|------------|---------|-----|
| `zeromount_rules_ht` | Virtual path -> real path mapping | `full_name_hash(virtual_path)` |
| `zeromount_dirs_ht` | Directory injection metadata | `full_name_hash(dir_path)` |
| `zeromount_uid_ht` | Blocked UIDs (bypass redirection) | `uid` |
| `zeromount_ino_ht` | Inode tracking for injected files | `ino ^ dev` |

### Atomic State

```
patches/zeromount-core.patch:64-65
```

```c
atomic_t zeromount_enabled = ATOMIC_INIT(0);
#define ZEROMOUNT_DISABLED() (atomic_read(&zeromount_enabled) == 0)
```

---

## Kernel Implementation

### Data Structures

```
include/linux/zeromount.h:1007-1040
```

**zeromount_rule:**
```c
struct zeromount_rule {
    struct hlist_node node;      // Rules hash table
    struct hlist_node ino_node;  // Inode hash table
    struct list_head list;       // Global list for iteration
    size_t vp_len;               // Virtual path length
    char *virtual_path;          // Source path (what app requests)
    char *real_path;             // Target path (what kernel serves)
    unsigned long real_ino;      // Real file inode
    dev_t real_dev;              // Real file device
    bool is_new;                 // True if file didn't exist in stock
    u32 flags;                   // ZM_FLAG_ACTIVE, ZM_FLAG_IS_DIR
    struct rcu_head rcu;         // RCU-safe deletion
};
```

**zeromount_uid_node:**
```c
struct zeromount_uid_node {
    uid_t uid;
    struct hlist_node node;
    struct rcu_head rcu;
};
```

**zeromount_dir_node:**
```c
struct zeromount_dir_node {
    struct hlist_node node;
    char *dir_path;
    struct list_head children_names;  // List of zeromount_child_name
    struct rcu_head rcu;
};
```

### IOCTL Handlers

```
patches/zeromount-core.patch:860-881
```

| IOCTL | Code | Handler |
|-------|------|---------|
| `ZEROMOUNT_IOC_ADD_RULE` | `_IOW(0x5A, 1, ...)` | `zeromount_ioctl_add_rule()` |
| `ZEROMOUNT_IOC_DEL_RULE` | `_IOW(0x5A, 2, ...)` | `zeromount_ioctl_del_rule()` |
| `ZEROMOUNT_IOC_CLEAR_ALL` | `_IO(0x5A, 3)` | `zeromount_ioctl_clear_rules()` |
| `ZEROMOUNT_IOC_GET_VERSION` | `_IOR(0x5A, 4, ...)` | Returns `ZEROMOUNT_VERSION` |
| `ZEROMOUNT_IOC_ADD_UID` | `_IOW(0x5A, 5, ...)` | `zeromount_ioctl_add_uid()` |
| `ZEROMOUNT_IOC_DEL_UID` | `_IOW(0x5A, 6, ...)` | `zeromount_ioctl_del_uid()` |
| `ZEROMOUNT_IOC_GET_LIST` | `_IOR(0x5A, 7, ...)` | `zeromount_ioctl_list_rules()` |
| `ZEROMOUNT_IOC_ENABLE` | `_IO(0x5A, 8)` | `zeromount_ioctl_enable()` |
| `ZEROMOUNT_IOC_DISABLE` | `_IO(0x5A, 9)` | `zeromount_ioctl_disable()` |

### Exported Functions

```
patches/zeromount-core.patch:97
```

```c
bool zeromount_is_uid_blocked(uid_t uid);  // EXPORT_SYMBOL
```

Used by SUSFS to check exclusion status.

---

## zm CLI Reference

```
module/bin/zm (userspace binary)
```

| Command | Usage | Effect |
|---------|-------|--------|
| `add` | `zm add <virtual> <real>` | Create redirection rule |
| `del` | `zm del <virtual>` | Delete rule by virtual path |
| `clear` | `zm clear` | Clear all rules AND blocked UIDs |
| `blk` | `zm blk <uid>` | Block UID (sees real files) |
| `unb` | `zm unb <uid>` | Unblock UID |
| `list` | `zm list` | List all rules (format: `real->virtual`) |
| `ver` | `zm ver` | Return version (currently `1`) |
| `enable` | `zm enable` | Enable VFS hooks |
| `disable` | `zm disable` | Disable VFS hooks |

### UID Exclusion Semantics (Inverted)

| State | ZeroMount VFS | SUSFS Hiding | App Sees |
|-------|---------------|--------------|----------|
| `zm blk <uid>` | Bypassed | Bypassed | REAL filesystem |
| Not blocked | Active | Active | MODULE files |
| Root (UID 0) | Bypassed | Bypassed | REAL filesystem |

---

## Module Scripts

### metamount.sh (Boot Sequence)

```
module/metamount.sh:1-275
```

Runs at KernelSU `post-fs-data` hook.

**Execution Flow:**
1. Bootloop protection (3-strike counter)
2. Config backup
3. Clear stale rules: `zm clear`
4. Conflict detection between modules
5. Scan `/data/adb/modules/*/system|vendor|product/*`
6. For each file: `zm add /virtual/path /real/module/path`
7. Track paths per module in `/data/adb/zeromount/module_paths/<name>`
8. SUSFS integration: `zm_register_rule_with_susfs()`
9. Enable engine: `zm enable`
10. Start monitor daemon
11. Notify KernelSU: `/data/adb/ksud kernel notify-module-mounted`

**Key Variables:**
```bash
MODULES_DIR="/data/adb/modules"
ZEROMOUNT_DATA="/data/adb/zeromount"
TARGET_PARTITIONS="system vendor product system_ext odm oem"
```

### service.sh (Late Init)

```
module/service.sh:1-56
```

Runs at KernelSU `late_start service`.

**Tasks:**
1. Hide `/dev/zeromount` via SUSFS
2. Hide `/sys/kernel/zeromount` via SUSFS
3. Apply UID exclusions from `.exclusion_list`
4. Create symlink: `webroot/link -> /data/adb/zeromount`

### monitor.sh (Daemon)

```
module/monitor.sh:1-319
```

Background daemon started by metamount.sh.

**Features:**
- Process camouflage as `kworker/u*:zm`
- Single-instance check via PID file
- 5-second polling interval
- Module change detection (add/remove/update)
- App install/uninstall watching (inotifywait/logcat/poll)
- Status cache generation for instant WebUI load

**Status Cache:**
```
module/monitor.sh:65-88
```

```json
{
  "engineActive": true,
  "rulesCount": 150,
  "excludedCount": 2,
  "driverVersion": "1",
  "kernelVersion": "5.10.209",
  "deviceModel": "Pixel 6",
  "androidVersion": "14",
  "uptime": "2h 30m",
  "selinuxStatus": "Enforcing",
  "susfsVersion": "1.5.0",
  "loadedModules": "tricky,playintegrityfix",
  "timestamp": 1706745600000
}
```

### logging.sh (Logging System)

```
module/logging.sh:1-394
```

Unified logging library.

**Log Directory Structure:**
```
/data/adb/zeromount/logs/
├── kernel/         # dmesg filtered
├── frontend/       # Script logs
│   ├── service.log
│   ├── metamount.log
│   └── monitor.log
├── susfs/          # SUSFS integration
└── archive/        # Rotated logs
```

**Log Levels:**
| Level | Value | Function |
|-------|-------|----------|
| OFF | 0 | - |
| ERROR | 1 | `log_err` |
| WARN | 2 | `log_warn` |
| INFO | 3 | `log_info` |
| DEBUG | 4 | `log_debug` |
| TRACE | 5 | `log_trace` |

### susfs_integration.sh (SUSFS Coupling)

```
module/susfs_integration.sh:1-956
```

Automatic SUSFS configuration when ZeroMount rules are added.

**Main API:**
```bash
zm_register_rule_with_susfs <vpath> <rpath>
```

**Capabilities Detection:**
```
module/susfs_integration.sh:89-96
```

| Capability | Flag |
|------------|------|
| `add_sus_path` | `HAS_SUS_PATH` |
| `add_sus_path_loop` | `HAS_SUS_PATH_LOOP` |
| `add_sus_mount` | `HAS_SUS_MOUNT` |
| `add_sus_kstat_statically` | `HAS_SUS_KSTAT` |
| `add_sus_kstat_redirect` | `HAS_SUS_KSTAT_REDIRECT` |
| `add_sus_map` | `HAS_SUS_MAPS` |
| `add_open_redirect` | `HAS_OPEN_REDIRECT` |

**Path Classification:**
```
module/susfs_integration.sh:118-153
```

| Path Pattern | Actions |
|--------------|---------|
| `*.so, *.jar, *.dex` | sus_path, sus_maps, sus_kstat |
| `/system/bin/*` | sus_path, sus_kstat |
| `/system/fonts/*` | sus_kstat, sus_maps |
| `/system/app/*` | sus_path, sus_kstat, sus_mount_check |
| `/data/adb/*` | sus_path_loop, sus_kstat |

---

## WebUI Architecture

### Component Hierarchy

```
webui-v2-beta/src/
├── App.tsx                 # Root with TabBar
├── routes/
│   ├── StatusTab.tsx       # Engine status, system info
│   ├── ModulesTab.tsx      # KSU modules management
│   ├── ConfigTab.tsx       # App exclusions
│   └── SettingsTab.tsx     # Theme, accent color
├── components/
│   ├── core/               # Button, Badge, Card
│   └── layout/             # Header, TabBar
└── lib/
    ├── api.ts              # Shell commands via KSU exec
    ├── store.ts            # SolidJS state management
    ├── theme.ts            # Accent colors, themes
    ├── ksuApi.ts           # KSU native API wrappers
    └── constants.ts        # Paths, version
```

### State Management (store.ts)

```
webui-v2-beta/src/lib/store.ts:8-632
```

SolidJS reactive store with granular loading states:

```typescript
const [loading, setLoading] = createStore({
  status: false,   // Engine, stats, systemInfo
  modules: false,  // KSU modules scan
  apps: false,     // Installed apps list
  rules: false,    // VFS rules CRUD
  activity: false, // Activity log
  engine: false,   // Engine toggle
});
```

**Key Actions:**
- `loadInitialData()` - Fast path with cache, background refresh
- `toggleEngine()` - Enable/disable VFS hooks
- `excludeUid()` / `includeUid()` - UID management
- `scanKsuModules()` - Detect modules with system/vendor/product

### API Layer (api.ts)

```
webui-v2-beta/src/lib/api.ts:1-813
```

**KSU Exec Pattern (Callback, not Promise):**
```typescript
async function execCommand(cmd: string, timeoutMs = 30000): Promise<KsuExecResult> {
  const callbackName = `exec_cb_${Date.now()}_${execCounter++}`;

  (window as any)[callbackName] = (errno: number, stdout: string, stderr: string) => {
    clearTimeout(timeoutId);
    delete (window as any)[callbackName];
    resolve({ errno, stdout, stderr });
  };

  ksu.exec(cmd, '{}', callbackName);
}
```

**Status Cache Fast Path:**
```typescript
async getStatusCache(): Promise<StatusCache | null> {
  const { errno, stdout } = await execCommand(`cat "${PATHS.STATUS_CACHE}"`, 2000);
  if (errno === 0 && stdout.trim()) {
    const cache = JSON.parse(stdout.trim());
    if (Date.now() - cache.timestamp < 30000) {
      return cache;  // Valid cache hit
    }
  }
  return null;  // Cache miss, do full load
}
```

### Theme System (theme.ts)

```
webui-v2-beta/src/lib/theme.ts:1-197
```

**Accent Presets:**
| Color | Hex | Text on Gradient |
|-------|-----|------------------|
| Orange | `#FF8E53` | Dark |
| Emerald | `#00D68F` | Dark |
| Azure | `#00B4D8` | Dark |
| Slate | `#64748B` | Dark |
| Indigo | `#6366F1` | White |
| Coral | `#FF6B6B` | Dark |

**Dynamic Text Contrast:**
```typescript
export function getContrastText(bgHex: string): string {
  return getLuminance(bgHex) > 0.5 ? '#1A1A2E' : '#FFFFFF';
}
```

**Random Accent on Visibility:**
```typescript
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible' && settings.autoAccentColor) {
    const newRandom = colors[Math.floor(Math.random() * colors.length)];
    setSettings({ accentColor: newRandom });
  }
});
```

### Constants (constants.ts)

```
webui-v2-beta/src/lib/constants.ts:1-16
```

```typescript
export const PATHS = {
  BINARY: '/data/adb/modules/zeromount/bin/zm',
  DEVICE: '/dev/zeromount',
  DATA_DIR: '/data/adb/zeromount/',
  MODULE_PATHS: '/data/adb/zeromount/module_paths',
  EXCLUSION_FILE: '/data/adb/zeromount/.exclusion_list',
  EXCLUSION_META: '/data/adb/zeromount/.exclusion_meta.json',
  ACTIVITY_LOG: '/data/adb/zeromount/activity.log',
  VERBOSE_FLAG: '/data/adb/zeromount/.verbose',
  STATUS_CACHE: '/data/adb/zeromount/.status_cache.json',
};
```

---

## Integration Points

### SUSFS Integration (Kernel Level)

```
/home/claudetest/gki-build/susfs4ksu-new/kernel_patches/include/linux/susfs_def.h:91-98
```

```c
#ifdef CONFIG_ZEROMOUNT
extern bool zeromount_is_uid_blocked(uid_t uid);
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) {
    return zeromount_is_uid_blocked(uid);
}
#else
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) { return false; }
#endif
```

**Modified SUSFS Functions:**
- `is_i_uid_not_allowed()` - Checks ZeroMount exclusion
- `is_i_uid_in_android_data_not_allowed()` - Checks ZeroMount exclusion
- `show_vfsmnt()` - Mount hiding with ZeroMount check
- `show_mountinfo()` - Mount hiding with ZeroMount check

**Integration Flow:**
```
WebUI: User clicks "Exclude" on detector app
  │
  ▼
api.ts: zm blk <uid>
  │
  ▼
ZeroMount Kernel: hash_add_rcu(zeromount_uid_ht, uid)
  │
  ├──► ZeroMount: zeromount_is_uid_blocked() → bypass VFS redirection
  │
  └──► SUSFS: susfs_is_uid_zeromount_excluded() → bypass ALL hiding
```

### KernelSU Hooks

**Metamodule Hook:**
```bash
/data/adb/ksud kernel notify-module-mounted
```

Called after successful boot to notify KernelSU that module mounting is complete.

---

## Build & Deploy

### Kernel Patch Application

```bash
# From kernel source root
patch -p1 < /path/to/patches/zeromount-core.patch

# Enable in config
echo "CONFIG_ZEROMOUNT=y" >> .config
make olddefconfig
```

### WebUI Build

```bash
cd /home/claudetest/zero-mount/nomount/webui-v2-beta
pnpm install
pnpm build  # Output to ../module/webroot-beta/
```

### Device Push Commands

**WebUI:**
```bash
adb push module/webroot-beta/index.html /data/local/tmp/
adb push module/webroot-beta/assets/. /data/local/tmp/assets/
adb shell "su -c 'cp /data/local/tmp/index.html /data/adb/modules/zeromount/webroot/ && rm -rf /data/adb/modules/zeromount/webroot/assets && cp -r /data/local/tmp/assets /data/adb/modules/zeromount/webroot/'"
```

**Shell Scripts:**
```bash
adb push module/*.sh /data/local/tmp/
adb shell "su -c 'cp /data/local/tmp/*.sh /data/adb/modules/zeromount/ && chmod 755 /data/adb/modules/zeromount/*.sh'"
```

---

## Quick Reference

### Common Commands

```bash
# Check status
adb shell "su -c 'zm ver && zm list | wc -l'"

# View active rules
adb shell "su -c 'zm list'"

# Regenerate app list
adb shell "su -c '/data/adb/modules/zeromount/refresh_apps.sh'"

# View logs
adb shell "su -c 'cat /data/adb/zeromount/logs/frontend/metamount.log'"

# Diagnostic CLI
adb shell "su -c '/data/adb/modules/zeromount/zm-diag.sh status'"
```

### File Locations

| File | Purpose |
|------|---------|
| `/dev/zeromount` | Kernel device node |
| `/sys/kernel/zeromount/debug` | Debug level (0-2) |
| `/data/adb/modules/zeromount/bin/zm` | Userspace CLI |
| `/data/adb/zeromount/module_paths/<name>` | Per-module tracking |
| `/data/adb/zeromount/.exclusion_list` | Excluded UIDs |
| `/data/adb/zeromount/.exclusion_meta.json` | Exclusion metadata |
| `/data/adb/zeromount/.status_cache.json` | Daemon cache |
| `/data/adb/zeromount/logs/` | Log directory |

### Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `/dev/zeromount` missing | Kernel not patched | Rebuild kernel with patch |
| Rules not applied | Engine disabled | `zm enable` |
| App sees module files | UID not excluded | `zm blk <uid>` |
| Root sees module files | By design | Cannot test VFS as root |
| WebUI slow load | Cache miss | Monitor daemon generates cache |
| Bootloop | Config corruption | 3-strike protection auto-recovers |

### Diagnostic CLI

```
module/zm-diag.sh:1-130
```

```bash
zm-diag.sh status     # Driver, rules, SUSFS, monitor status
zm-diag.sh modules    # List tracked modules with rule counts
zm-diag.sh conflicts  # Detect file conflicts between modules
zm-diag.sh rules      # List active VFS redirection rules
```

---

## Design Decisions

1. **Root bypasses hooks** - Security requirement, cannot test VFS as root
2. **Exclusion is inverted** - "Excluded" UIDs see REAL files (for detectors)
3. **RCU for hot paths** - All hash table lookups are RCU-protected
4. **Per-CPU recursion guard** - Prevents re-entry during `getname_kernel()`
5. **Deferred SUSFS paths** - Applied after overlays unmount to avoid EINVAL
6. **Single aapt call** - Badging output cached, parsed for multiple fields
7. **Module-scope caches** - Icon cache outside component to persist
