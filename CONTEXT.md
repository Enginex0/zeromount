# ZeroMount Metamodule - Complete Context Document

> **Purpose:** Single source of truth for the ZeroMount refactoring project
> **Last Updated:** 2026-02-08
> **Version Analyzed:** v3.4.0 (module.prop:4)
> **Source Analysis:** 6 domain analysts, 2 synthesizers, 12 consensus validators (294 claims checked, 90.5% verified)

---

## 1. Problem Statement

**Symptom:** ZeroMount metamodule causes instability with LSPosed and interferes with other modules.

**Root Cause:** `susfs_apply_mount_hiding()` in `susfs_integration.sh` scans `/proc/mounts` for ALL overlay/tmpfs mounts and hides them via SUSFS. ZeroMount is mountless — it doesn't create mounts. This function catches and hides mounts from OTHER systems (LSPosed, stock Android overlays, modules with `skip_mount`).

**Broader Issues Discovered:** Beyond the mount-hiding bug, analysis revealed kernel-level bugs (ghost directory entries, missing binary commands), script inconsistencies (partition list mismatches), WebUI dead code, and stale documentation across the project.

---

## 2. Architecture Overview

### 2.1 What ZeroMount IS

A compiled-in Linux kernel subsystem (not a loadable module) that intercepts VFS operations to redirect file paths and inject virtual directory entries — all without `mount()` syscalls. It operates as a KernelSU metamodule (`metamodule=1` in `module.prop:2`), meaning KernelSU delegates module mounting to ZeroMount's orchestration scripts.

**Binary:** `zm` (evolved from legacy `nm` / "NoMount")
**Kernel magic:** `0x5A` (ASCII `'Z'`)
**Device:** `/dev/zeromount` (miscdevice, mode 0600, root-only)
**Debug:** `/sys/kernel/zeromount/debug` (sysfs, levels 0/1/2)

### 2.2 Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  LAYER 3: WebUI (SolidJS SPA in KernelSU WebView)          │
│    StatusTab | ModulesTab | ConfigTab | SettingsTab          │
│         │  ksu.exec() -> shell commands                      │
├─────────────────────────────────────────────────────────────┤
│  LAYER 2: Userspace Orchestration                            │
│  ┌────────┐ ┌────────────┐ ┌──────────┐ ┌───────────┐      │
│  │zm binary│ │metamount.sh│ │service.sh│ │monitor.sh │      │
│  │(ioctl  │ │(boot-time  │ │(post-boot│ │(runtime   │      │
│  │ CLI)   │ │ orchestrate)│ │UID block)│ │ watch)    │      │
│  └────┬───┘ └─────┬──────┘ └────┬─────┘ └─────┬─────┘      │
│       │     ┌─────┴──────────┐  │              │             │
│       │     │susfs_integration│  │              │             │
│       │     │.sh (coupling)  │  │              │             │
│       │     └────────────────┘  │              │             │
│       │  ioctl(/dev/zeromount)  │              │             │
├─────────────────────────────────────────────────────────────┤
│  LAYER 1: Kernel (compiled-in)                               │
│  fs/zeromount.c + include/linux/zeromount.h                  │
│  4 Hash Tables (1024 buckets, RCU-protected)                 │
│  6 VFS Hook Points (namei, readdir, d_path, stat, statfs,   │
│                      xattr)                                  │
│  Optional: SUSFS read-only coupling                          │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 What ZeroMount is NOT

- NOT an overlayfs-based system (like Magisk/KernelSU magic mount)
- NOT visible in `/proc/mounts`
- NOT creating any mount points
- Detection apps enumerating mount tables find nothing

### 2.4 Unique Capabilities

| Capability | ZeroMount | Hybrid Mount | Why |
|------------|-----------|--------------|-----|
| Font/Emoji modules | Yes | No | Needs kstat spoofing for inode match |
| Debloat (remove files) | Yes | No | VFS returns ENOENT, no whiteout needed |
| App Systemizer | Yes | No | Injects into /system/app listings |
| Invisible operation | Yes | No | No /proc/mounts entries |
| Statfs spoofing | Yes | No | Reports EROFS for redirected paths |
| SELinux context spoofing | Yes | No | Returns correct xattr contexts |

---

## 3. Kernel Implementation

### 3.1 Core Data Structures

```c
struct zeromount_rule {
    struct hlist_node node;      // Hash by virtual_path
    struct hlist_node ino_node;  // Hash by inode
    struct list_head list;       // Linear iteration
    char *virtual_path;          // e.g., "/system/lib/libc.so"
    char *real_path;             // e.g., "/data/adb/modules/xyz/system/lib/libc.so"
    unsigned long real_ino;      // Cached inode of real file
    dev_t real_dev;              // Device ID of real file
    bool is_new;                 // Newly injected file?
    u32 flags;                   // ZM_FLAG_ACTIVE, ZM_FLAG_IS_DIR
    struct rcu_head rcu;         // RCU for safe deletion
};
```

**Hash Tables (4, all 1024 buckets):**
- `zeromount_rules_ht` — virtual_path → rule
- `zeromount_dirs_ht` — dir_path → children list (for readdir injection)
- `zeromount_uid_ht` — uid → blocklist entry
- `zeromount_ino_ht` — inode^dev → rule (reverse lookup for d_path/xattr)

**Concurrency:** Single spinlock (`zeromount_lock`) for writes; RCU for all hot-path reads. Rules freed via `call_rcu()`.

### 3.2 VFS Hook Chain

| Hook Location | Function | Purpose |
|---------------|----------|---------|
| `fs/namei.c` getname_flags() | `zeromount_getname_hook()` | Primary path redirection |
| `fs/namei.c` generic_permission() | Permission bypass | Allow traversal through /data/adb |
| `fs/readdir.c` getdents/getdents64/compat | `zeromount_inject_dents64()` | Inject virtual directory entries |
| `fs/d_path.c` d_path() | Inode-to-path reverse | Return virtual path for redirected files |
| `fs/stat.c` vfs_statx() | Relative path handling | Handle dirfd-relative paths |
| `fs/statfs.c` user_statfs() | `zeromount_spoof_statfs()` | Spoof fs type as EROFS |
| `fs/xattr.c` vfs_getxattr() | `zeromount_spoof_xattr()` | Spoof SELinux contexts |

### 3.3 Central Safety Gate (`zeromount_should_skip()`)

Skips all hooks when:
- Subsystem disabled (`zeromount_enabled == 0`)
- Recursion detected (prevents infinite loops from internal `kern_path()`)
- Interrupt/NMI/oops context
- Kernel threads, exiting tasks, PID 1/2
- No mm_struct or nsproxy
- `PF_MEMALLOC_NOFS` (memory reclaim path)
- SUSFS marks process as "umounted" (`susfs_is_current_proc_umounted()`)

**Recursion Guard:** Preferred implementation uses bit 0 of `current->android_oem_data1`; fallback uses `current->journal_info` with marker `0x5A4D`.

### 3.4 Ioctl Commands (10 total)

| Command | Code | Purpose |
|---------|------|---------|
| ADD_RULE | 0x40185A01 | Add path redirection rule |
| DEL_RULE | 0x40185A02 | Delete rule by virtual_path |
| CLEAR_ALL | 0x5A03 | Clear all rules and UIDs |
| GET_VERSION | 0x80045A04 | Query version (no CAP_SYS_ADMIN needed) |
| ADD_UID | 0x40045A05 | Exclude UID from redirection |
| DEL_UID | 0x40045A06 | Include UID in redirection |
| GET_LIST | 0x80045A07 | List all rules (max 64KB) |
| ENABLE | 0x5A08 | Activate redirection engine |
| DISABLE | 0x5A09 | Deactivate engine |
| REFRESH | 0x5A0A | Flush dcache for all paths |

**Engine starts DISABLED** (`ATOMIC_INIT(0)`) — must be explicitly enabled via ioctl after rules load.

### 3.5 Kernel Patches

**Stage 1 — Core patch** (`zeromount-core.patch`):
- Creates `fs/zeromount.c` (1158 lines) and `include/linux/zeromount.h` (160 lines)
- Modifies `fs/Kconfig` (adds `CONFIG_ZEROMOUNT`, default `y`)
- Modifies `fs/Makefile` (adds `zeromount.o`)

**Stage 2 — Hook injection** (6 idempotent scripts, all `#ifdef CONFIG_ZEROMOUNT` guarded):

| Script | Target | Injection Points |
|--------|--------|-----------------|
| inject-zeromount-namei.sh | fs/namei.c | 4 |
| inject-zeromount-readdir.sh | fs/readdir.c | 12 (3 syscalls x 4 blocks) |
| inject-zeromount-dpath.sh | fs/d_path.c | 1 |
| inject-zeromount-stat.sh | fs/stat.c | 2 |
| inject-zeromount-statfs.sh | fs/statfs.c | 1 |
| inject-zeromount-xattr.sh | fs/xattr.c | 1 |

**Stage 3 — SUSFS bypass fix** (`fix-zeromount-susfs-bypass.sh`):
Adds `susfs_is_current_proc_umounted()` guards to 10 exported functions. Defense-in-depth — the central `zeromount_should_skip()` already includes this check, but per-function guards cover early-return paths.

**Kernel 5.10 variant** (`zeromount-kernel-5.10.patch`):
Older version. Statfs spoofing (`zeromount_spoof_statfs()` at line 567) and xattr spoofing (`zeromount_spoof_xattr()` at line 638) ARE present. Missing 3 features: relative-path stat (`zeromount_build_absolute_path()`), directory-already-redirected check in readdir, recursive auto-parent injection. Possibly unmaintained.

### 3.6 SUSFS Kernel Coupling

One-way, read-only at kernel level:
1. SUSFS marks processes as "umounted"
2. ZeroMount reads those marks via `susfs_is_current_proc_umounted()`
3. ZeroMount disables its hooks for marked processes
4. ZeroMount never calls into SUSFS to register anything

Userspace SUSFS integration is separate — scripts call SUSFS binary (`add_sus_path`, `add_sus_kstat`) after each `zm add` to hide real files.

### 3.7 Kernel Export for SUSFS

```c
// ZeroMount exports:
EXPORT_SYMBOL(zeromount_is_uid_blocked);

// SUSFS consumes via extern:
#ifdef CONFIG_ZEROMOUNT
extern bool zeromount_is_uid_blocked(uid_t uid);
#endif
```

Called at 6 SUSFS check points (3 in sus_path visibility + 3 in mount display) for per-UID visibility decisions.

---

## 4. Binary (`zm`)

**Source:** `src/zm.c` (304 lines). Freestanding C, zero libc dependency, raw syscalls. Cross-compiled with Zig (`zig cc`).

**Targets:** aarch64-linux (ARM64), arm-linux (ARM32)

**Build flags:** `-Oz -static -nostdlib -ffreestanding -flto` + aggressive stripping. Optional `sstrip` post-processing.

**Commands (9 of 10 kernel ioctls implemented):**

| Command | Dispatched by | Purpose |
|---------|--------------|---------|
| `zm add <vp> <rp>` | `a` | Add rule (auto-detects directory via fstatat) |
| `zm del <vp>` | `de` | Delete rule |
| `zm clear` | `c` | Clear all rules + UIDs |
| `zm ver` | `v` | Print kernel version (bare integer) |
| `zm blk <uid>` | `b` | Block UID |
| `zm unb <uid>` | `u` | Unblock UID |
| `zm list` | `l` | Print rules |
| `zm enable` | `e` | Enable engine |
| `zm disable` | `di` | Disable engine |

**Missing:** `zm refresh` — kernel defines `ZEROMOUNT_IOC_REFRESH` (0x5A0A) but binary has no handler.

---

## 5. Userspace Scripts

### 5.1 File Inventory

| File | Lines | Purpose |
|------|-------|---------|
| `metamount.sh` | 427 | Boot-time orchestrator: 8-phase rule injection pipeline |
| `susfs_integration.sh` | 978 | SUSFS coupling: path hiding, kstat spoofing, font redirect |
| `monitor.sh` | 327 | Runtime: module change detection, status cache for WebUI |
| `logging.sh` | 393 | Structured logging with rotation (sourced, not executed) |
| `sync.sh` | 130 | On-demand rule synchronization |
| `service.sh` | 79 | Post-boot: UID blocking, SUSFS artifact hiding, WebUI link |
| `zm-diag.sh` | 129 | CLI diagnostic tool |
| `customize.sh` | 45 | KernelSU installation hook |
| `metainstall.sh` | 3 | Module install hook |
| `metauninstall.sh` | 17 | Cleanup: clear rules and data |

### 5.2 Boot Sequence

```
1. Kernel init → fs_initcall(zeromount_init) → engine DISABLED
2. KernelSU detects metamodule=1 → calls metamount.sh
3. metamount.sh 8-phase orchestration:
   Phase 1: Init (source logging.sh, susfs_integration.sh)
   Phase 2: Bootloop protection (3-strike counter)
   Phase 3: Kernel check ([ -e /dev/zeromount ])
   Phase 4: Clear + SUSFS init (zm clear, susfs_init)
   Phase 5: Conflict detection (cross-module overlap scan)
   Phase 6: Whiteout handling (overlay artifacts → SUSFS)
   Phase 7: Module iteration (zm add for each file)
   Phase 8: Finalization
     ├── zm enable
     ├── zm refresh (BUG: no-op, command missing from binary)
     ├── apply_deferred_sus_paths
     ├── late_kstat_pass
     ├── Launch monitor.sh
     ├── Notify KernelSU module-mounted
     └── Reset boot counter, save config backup
4. service.sh (post-boot stage)
   ├── Hide /dev/zeromount and /sys/kernel/zeromount via SUSFS
   ├── Process .exclusion_list → zm blk <uid> for each
   └── Create webroot/link → /data/adb/zeromount symlink
5. monitor.sh (steady state)
   ├── Camouflage as kworker/u<N>:zm
   └── Every 5s: check modules, regenerate .status_cache.json
```

### 5.3 Dependency Chain

```
metamount.sh
  ├── sources logging.sh
  ├── sources susfs_integration.sh
  ├── spawns monitor.sh (background)
  └── calls zm (clear, add, enable, refresh[no-op], list)

monitor.sh
  ├── sources logging.sh
  ├── sources susfs_integration.sh
  └── calls zm (add, del, list, ver)

service.sh → calls SUSFS binary directly + zm (blk)
sync.sh → sources logging.sh + calls zm (add, del)
zm-diag.sh → calls zm (list, ver)
metauninstall.sh → calls zm (clear, disable)
```

### 5.4 `susfs_integration.sh` Functions (20)

| Function | Purpose | Status |
|----------|---------|--------|
| `susfs_init()` | Binary discovery, capability detection | KEEP |
| `susfs_classify_path()` | Determines SUSFS actions per file type | REVIEW — contains `sus_mount_check` |
| `susfs_capture_metadata()` | Captures original stat before overlay | KEEP |
| `susfs_apply_path()` | Hides paths via `add_sus_path` (deferred) | KEEP |
| `apply_deferred_sus_paths()` | Processes deferred paths post-overlay | KEEP |
| `susfs_apply_mount_hiding()` | Scans /proc/mounts, hides overlay/tmpfs | **REMOVE — root cause of LSPosed issue** |
| `susfs_apply_kstat()` | Spoofs file metadata to match stock | KEEP — critical for fonts |
| `apply_font_redirect()` | Font-specific open_redirect + kstat | KEEP — 115 lines, specialized |
| `late_kstat_pass()` | Re-processes deferred kstat entries | KEEP |
| `susfs_update_config()` | Writes SUSFS config files | REVIEW — consumed by upstream SUSFS module at boot; unnecessary when ZeroMount replaces SUSFS module in v2 |
| `susfs_clean_zeromount_entries()` | Removes [ZeroMount]-tagged SUSFS entries | KEEP |
| `susfs_clean_module_entries()` | Per-module SUSFS cleanup | KEEP |
| `susfs_clean_module_metadata_cache()` | Metadata cache cleanup | KEEP |
| `zm_register_rule_with_susfs()` | Main entry after each `zm add` | KEEP |
| `susfs_get_cached_metadata()` | Cache lookup for pre-overlay stat metadata | KEEP (in-memory in Rust) |
| `susfs_hide_path()` | Simplified path hiding wrapper | KEEP |
| `susfs_apply_maps()` | Maps hiding via `add_sus_map` | KEEP |
| `susfs_capture_module_metadata()` | Batch stat capture for module files | KEEP |
| `susfs_status()` | Status JSON output | KEEP — replaced by Rust binary status |
| `susfs_reset_stats()` | Stats tracking reset | KEEP |

### 5.5 Data Directory (`/data/adb/zeromount/`)

| Path | Writer | Reader | Purpose |
|------|--------|--------|---------|
| `.status_cache.json` | monitor.sh (5s) | WebUI | Dashboard data |
| `.exclusion_list` | WebUI | service.sh | UID-per-line blocklist |
| `.exclusion_meta.json` | WebUI | WebUI | UID metadata (app name, package) |
| `activity.log` | WebUI | WebUI | Activity log entries |
| `.verbose` | WebUI (touch/rm) | metamount.sh | Verbose logging flag |
| `boot_counter` | metamount.sh | metamount.sh | Bootloop protection |
| `module_paths/<name>` | metamount.sh, monitor.sh | WebUI, sync.sh | Per-module tracking |
| `metadata_cache/` | susfs_integration.sh | susfs_integration.sh | Stat metadata (MD5-keyed) |
| `.monitor.pid` | monitor.sh | monitor.sh | Single-instance PID lock |
| `.refresh_trigger` | monitor.sh | WebUI (polled 2s) | App list refresh signal |
| `logs/` | logging.sh | Debugging | Structured logs |
| `config.sh` | Manual | monitor.sh | Shell config (excluded_modules) |
| `.deferred_kstat` | susfs_integration.sh | susfs_integration.sh | Deferred kstat entries |

---

## 6. WebUI

### 6.1 Architecture

SolidJS + Vite SPA running inside KernelSU WebView. No backend server. All device communication via `ksu.exec()` → shell commands. Mock mode activates when `globalThis.ksu` is undefined (browser dev).

**Build:** `vite.config.ts` outputs to `../module/webroot-beta` (but deployed dir is `webroot` — mismatch).
**Dependencies:** solid-js ^1.9.10, @material/web ^2.4.1, kernelsu ^3.0.0 (external)
**Entry:** `index.html` → `src/index.tsx` → `<App />`
**Navigation:** State-driven via `store.activeTab()`, no URL routing.

### 6.2 Pages

| Page | Lines | Purpose |
|------|-------|---------|
| StatusTab | 459 | Dashboard: engine toggle, stats, system info, activity feed |
| ConfigTab | 331 | App exclusion management with search and lazy-loaded icons |
| ModulesTab | 244 | KSU module scan, hot load/unload |
| SettingsTab | 334 | Theme, accent color, engine prefs, debug export |

### 6.3 Shell Commands Called

| Command | Purpose |
|---------|---------|
| `zm ver/list/add/del/clear/blk/unb/enable/disable` | All zm binary operations |
| `uname -r` | Kernel version |
| `cat /proc/uptime` | System uptime |
| `getprop ro.product.model` / `ro.build.version.release` | Device/Android info |
| `getenforce` | SELinux status |
| `ksu_susfs show version` | SUSFS version |
| `pm list packages` | App listing (fallback) |
| `aapt dump badging` | App label resolution (fallback) |

### 6.4 KSU Native API (optional)

`ksu.listUserPackages()`, `ksu.listSystemPackages()`, `ksu.getPackagesInfo()`, `ksu.getPackagesIcons()` — falls back to `pm`/`aapt` if unavailable.

---

## 7. Confirmed Bugs

### 7.1 HIGH Severity

**BUG-H1: Ghost directory entries (`dirs_ht` not cleaned)**
- Location: `zeromount.c` — `del_rule` removes from `rules_ht`/`ino_ht` only; `clear_all` clears `rules_ht`/`uid_ht` only. `dirs_ht` is never cleaned.
- Effect: Deleted files still appear in `ls`/`readdir` but can't be opened. Accumulates over runtime hot-unload cycles.
- Boot-time masked because `zm clear` + full re-injection covers stale entries.

**BUG-H2: ARM32 native ioctl mismatch**
- Location: `zm.c:120-121` hardcodes arm64 struct size (24 bytes) in ioctl numbers. Native arm32 kernel produces different values (sizeof=12 → `0x400C5A01` vs `0x40185A01`).
- Effect: ADD_RULE and DEL_RULE broken on native arm32 kernels.
- Real-world impact: Low — pure arm32 Android kernels are extremely rare.

### 7.2 MEDIUM Severity

**BUG-M1: Missing `refresh` command in zm binary**
- Location: `zm.c` has 9 ioctl constants, no REFRESH. `metamount.sh:388` calls `zm refresh` in background, silently fails.
- Effect: Post-enable dcache flush never happens. System relies on per-rule flush at add time and natural dcache invalidation.

**BUG-M2: Target partition list mismatch**
- Locations: metamount.sh:14 (20 partitions), monitor.sh:15 (10), sync.sh:14 (13), zm-diag.sh:10 (6).
- Effect: OEM partition files (my_bigball, my_carrier, etc.) injected at boot but invisible to runtime monitoring/sync. Also sync.sh has 3 partitions (oem_dlkm, system_dlkm, vendor_dlkm) NOT in metamount.sh.

**BUG-M3: Enable-before-SUSFS race**
- Location: `metamount.sh:386` enables engine; `line 399` applies deferred SUSFS paths. Brief visibility window.
- Mitigation: Kernel-side `susfs_is_current_proc_umounted()` covers SUSFS-marked processes. Non-marked processes could observe unprotected paths during the gap.

**BUG-M4: `isEngineActive()` checks wrong thing**
- Location: `api.ts:574-588` — checks `[ -e /dev/zeromount ]` which tests kernel patch presence, not engine enabled state. Device always exists on patched kernel.
- Root cause: No kernel ioctl exists to query `zeromount_enabled` state.

**BUG-M5: `installed_apps.json` never generated**
- Location: `api.ts:597` fetches it; no shell script generates it. Dead code path.

**BUG-M6: UID unblock persistence risk**
- Location: `api.ts:437-470` — `includeUid()` runs `zm unb <uid>` (runtime) + `sed` (persistence). The `sed` is in `.catch()` that swallows errors. Silent failure → UID re-blocked on reboot.

**BUG-M7: Build output path mismatch**
- Location: `vite.config.ts:9` targets `webroot-beta`; deployed directory is `webroot`. Manual copy/rename step not captured.

**BUG-M8: Stale ARCHITECTURE.md**
- Claims 7 ioctls (actually 10), binary named `nm` (actually `zm`), engine starts ENABLED (actually DISABLED).

### 7.3 LOW Severity

**BUG-L1: Version string inconsistency**
- `module.prop:4` = v3.4.0, `constants.ts:15` = 3.0.0, `package.json:4` = 0.0.0

**BUG-L2: Activity type parser mismatch**
- `logActivity()` writes 8+ types; parser recognizes 6. `RULES_CLEARED`, `MODULE_LOADED`, `MODULE_UNLOADED` fall back to `engine_enabled` type.

**BUG-L3: Process camouflage incomplete**
- `monitor.sh:52-57` sets `/proc/self/comm` to `kworker/u<N>:zm` but `/proc/<pid>/cmdline` still shows `sh monitor.sh`.

**BUG-L4: VfsRule naming inversion**
- `types.ts:1-9` — `source` = real path, `target` = virtual path. Backwards from intuitive naming. `addRule()` and `deleteRule()` compound the confusion. The same field names mean different things in read vs write paths: parsing kernel output (`GET_LIST`) uses `source` = text before `->` = real_path, but `addRule()` passes `source` as the first arg to `zm add` which expects virtual_path. Read/write semantic flip on the same field.

**BUG-L5: Verbose logging toggle deferred**
- WebUI toggle appears instant but `.verbose` flag is only read at boot. No indication of reboot requirement.

**BUG-L6: `zm ver` format mismatch**
- Outputs bare integer (e.g., "1"). WebUI expects/displays "v3.0.0" format.

### 7.4 Newly Discovered (Verification Phase)

**NEW-1: monitor.sh hot-load misses whiteouts/symlinks/dirs**
- Location: `monitor.sh:145-153` — `register_module()` only scans `-type f` (regular files). Misses whiteout char devices (`-type c`), directories, symlinks, and AUFS whiteouts. Contrasts with `metamount.sh:249` which scans `-type f -o -type d -o -type l -o -type c`. Cascades to WebUI rule count inaccuracy (`api.ts:557` runs `wc -l` on tracking files populated by monitor.sh).

**NEW-2: sync.sh bypasses unified logging system**
- Location: `sync.sh:13` — writes to `$ZEROMOUNT_DATA/zeromount.log` and defines its own `log_err`, `log_info`, `log_debug` functions instead of sourcing `logging.sh`.

**NEW-3: Four different file-scan patterns across scripts and WebUI**
- `metamount.sh:249`: `-type f -o -type d -o -type l -o -type c`
- `monitor.sh:147`: `-type f` only
- `sync.sh:70`: `-type f -o -type c`
- `api.ts:682` (scanKsuModules): `-type f`, limited to 3 partitions
- Extends ARCH-1 beyond partition lists to file type filtering.

**NEW-4: service.sh hardcodes SUSFS binary path**
- Location: `service.sh:4` — hardcodes `SUSFS_BIN="/data/adb/ksu/bin/ksu_susfs"` with `command -v` fallback. Differs from `susfs_integration.sh:56-71` which checks multiple paths.

---

## 8. Architecture Issues

**ARCH-1: No centralized TARGET_PARTITIONS**
Four scripts independently define partition lists. No shared constant file. Root cause of BUG-M2.

**ARCH-2: No kernel-side "is enabled" query**
No ioctl or sysfs attribute exposes `zeromount_enabled` state. Root cause of BUG-M4.

**ARCH-3: `susfs_apply_mount_hiding()` — root cause of original problem**
Scans `/proc/mounts` for overlay/tmpfs and hides them. ZeroMount doesn't create mounts. Catches LSPosed, stock overlays, other modules.

**ARCH-4: `sus_mount_check` classification in `susfs_classify_path()`**
Triggers mount hiding for app paths. Unnecessary for mountless architecture.

**ARCH-5: `susfs_update_config()` — v1 persistence model**
Writes SUSFS config files (`sus_path.txt`, `sus_maps.txt`, etc.) consumed by upstream SUSFS flashable module at boot (`boot-completed.sh`, `post-mount.sh`, `service.sh`). Unnecessary when ZeroMount replaces the SUSFS module in v2.

**ARCH-6: WebUI dual styling strategy**
CSS-file components (Badge, Button, Card) vs inline-styled components (Toggle, Modal, Toast). Both work but inconsistent for debugging.

**ARCH-7: SettingsTab breaks store pattern**
Only page that directly imports `api` (for `fetchSystemColor()` and `setVerboseLogging()`). All others go through store.

---

## 9. Dead Code Inventory

### 9.1 WebUI Dead Code

| Item | Location |
|------|----------|
| `hitsToday` animated but never rendered | StatusTab.tsx:52 |
| `.header__sun` CSS class | Header.css:26-36 |
| Unused theme imports | StatusTab.tsx:5 |
| `autoStartOnBoot` toggle (no backend) | SettingsTab.tsx:196-205 |
| `animationsEnabled` toggle (no effect) | SettingsTab.tsx:164-173 |
| `Card variant="elevated"` | Never used |
| `Badge variant="warning"` | Never used |
| `Input label`/`error` props | Never used |
| `store.modules()` signal | Loaded but unused |
| `store.addRule()` / `store.deleteRule()` | Exported but never called |
| `api.getInstalledApps()` | Fetches nonexistent file |
| `api.refreshInstalledApps()` | Explicit no-op |
| `api.checkAppListStale()` | Never called |
| `VfsRule.hits` | Always 0, kernel doesn't track |
| `VfsRule.active` | Always true, no per-rule toggle |

### 9.2 Repository Artifacts

| Item | Location |
|------|----------|
| Accidental `"` file (1 byte) | Repository root |
| Legacy `nm.c` (246 lines) | src/legacy/nm.c |
| `zm unb` unused in scripts | Only reachable from WebUI |

---

## 10. Reference Implementations

### 10.1 Source Locations

- ZeroMount module: `/home/claudetest/zero-mount/zeromount/`
- ZeroMount patches: `/home/claudetest/zero-mount/zeromount/patches/` (inferred)
- Full analysis: `/home/claudetest/zero-mount/context-gathering/output/zeromount/`

### 10.2 Comparison

| Metric | ZeroMount | Hybrid Mount | Mountify |
|--------|-----------|--------------|----------|
| metamount.sh | 427 lines | 30 lines | 430 lines |
| SUSFS integration | 978 lines | 0 | 10 lines |
| Background monitor | 327 lines | 0 | 0 |
| Total shell | ~2500 lines | 158 lines | 1062 lines |

---

## 11. Open Questions

1. Is the kernel 5.10 patch still maintained? Has statfs/xattr but missing relative-path stat, readdir dir-check, and recursive auto-parent injection.
2. Are per-function SUSFS checks (`fix-zeromount-susfs-bypass.sh`) needed given centralized `zeromount_should_skip()` check?
3. ~~Does `susfs_update_config()` write files anything actually reads?~~ **RESOLVED:** Yes, upstream SUSFS flashable module reads them at boot (see S12). Moot in v2 — Rust binary replaces the SUSFS module entirely.
4. Should `monitor.sh` force-stop KSU app (`com.rifsxd.ksunext`) on app changes?
5. What is the intended behavior for `autoStartOnBoot` setting? Wire up or remove?

---

*Last updated: 2026-02-08*
*Source: Validated analysis (294 claims, 90.5% verified, all inaccuracies patched)*
