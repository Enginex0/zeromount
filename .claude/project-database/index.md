# Project Database Index

Master reference for SUSFS and Zero-Mount kernel security projects with integrated architecture analysis and common task workflows.

**Last Updated:** 2026-01-31
**Database Version:** 1.0
**Projects Covered:** SUSFS v2.0.0 GKI, Zero-Mount v3.2.0

---

## Quick Navigation

| Project | Purpose | Reference Document | Source Path |
|---------|---------|-------------------|-------------|
| **SUSFS** | Kernel-level path hiding framework | [susfs-reference.md](./susfs-reference.md) | `/home/claudetest/gki-build/susfs4ksu-new/` |
| **Zero-Mount** | VFS path redirection without mounts | [zeromount-reference.md](./zeromount-reference.md) | `/home/claudetest/zero-mount/nomount/` |
| **Integration** | Unified SUSFS+ZeroMount architecture | [integration-map.md](./integration-map.md) | Both projects |
| **Functions** | Cross-project function index | [function-index.json](./function-index.json) | All files |

---

## Project Comparison Matrix

| Feature | SUSFS | Zero-Mount |
|---------|-------|-----------|
| **Purpose** | Path HIDING (ENOENT) | Path REDIRECTION (different content) |
| **Hook Point** | `__lookup_hash()`, `lookup_fast()`, `generic_fillattr()`, etc. | `getname_flags()` in `fs/namei.c` |
| **Primary Mechanism** | Inode marking + VFS interception | Hash table lookup + filename replacement |
| **Scope** | Zygote-spawned apps (sus_path), all processes (sus_mount) | All processes respecting UID exclusion |
| **Data Structure** | Linked lists (sus_path), hash tables (sus_kstat, open_redirect) | Hash tables (rules, UIDs, dirs, inodes) |
| **Kernel Config** | `CONFIG_KSU_SUSFS` | `CONFIG_ZEROMOUNT` |
| **State** | Thread-local flags (`TIF_PROC_UMOUNTED`), inode flags | Atomic enable flag, per-CPU recursion guard |
| **Capabilities** | 6 mechanisms (path, mount, kstat, map, redirect, uname) | Path redirection + UID exclusion + deferred hooks |
| **Dependencies** | `CONFIG_KSU`, `CONFIG_THREAD_INFO_IN_TASK` | None (standalone kernel module) |

---

## Common Tasks Quick-Reference

### "I want to hide a file so apps can't find it"
**→ Use SUSFS `sus_path`**
- File location: `/home/claudetest/gki-build/susfs4ksu-new/fs/susfs.c:178-268`
- Function: `susfs_add_sus_path()` (line 178)
- Related: `susfs_add_sus_path_loop()` for re-flagging on zygote spawns
- CLI: `ksu_susfs add_sus_path /path/to/hide`
- See also: susfs-reference.md § Path Hiding

### "I want to redirect file content (app opens /system/foo → gets /data/.../foo)"
**→ Use Zero-Mount VFS redirection**
- File location: `/home/claudetest/zero-mount/nomount/patches/zeromount-core.patch:269-302`
- Function: `zeromount_getname_hook()` (lines 269-302)
- Hash table: `zeromount_rules_ht` (rules lookup)
- CLI: `zm add /virtual/path /real/module/path`
- See also: zeromount-reference.md § VFS Hook Location

### "I want an excluded app to see the REAL filesystem (bypass redirection + hiding)"
**→ Use `zm blk <uid>` for unified exclusion**
- Zero-Mount side: `zeromount_is_uid_blocked()` (patch:97) - bypass VFS hooks
- SUSFS side: `susfs_is_uid_zeromount_excluded()` (susfs_def.h:94-97) - bypass all hiding
- Flow diagram: zeromount-reference.md § Integration Points
- Semantics: "excluded" means sees REAL files (inverted logic)

### "I need to spoof file metadata (stat, size, timestamps, permissions)"
**→ Use SUSFS `sus_kstat`**
- Function: `susfs_add_sus_kstat()` (susfs.c:507)
- Hash table: `SUS_KSTAT_HLIST` (line 196)
- Hook point: `generic_fillattr()` in `fs/stat.c`
- CLI: `ksu_susfs add_sus_kstat /path` (dynamic) or `add_sus_kstat_statically` (static)
- See also: susfs-reference.md § Stat Spoofing

### "I want to hide mount entries from /proc/mounts"
**→ Use SUSFS `sus_mount`**
- Function: `susfs_set_hide_sus_mnts_for_non_su_procs()` (susfs.c:456)
- Hook points: `show_vfsmnt()`, `show_mountinfo()`, `show_vfsstat()`
- Global flag: Controlled at boot time
- INTEGRATION: ZeroMount-aware mount hiding for excluded UIDs
- See also: susfs-reference.md § Mount Hiding, integration-map.md

### "I want to hide /dev/zeromount artifact from detector apps"
**→ Use SUSFS `sus_path_loop` + ZeroMount integration**
- Zero-Mount side: (automatic from `zm add` via `susfs_integration.sh`)
- SUSFS side: `ksu_susfs add_sus_path_loop /dev/zeromount` (re-flags per zygote)
- Script automation: `/home/claudetest/zero-mount/nomount/module/susfs_integration.sh:89-96`
- CI/CD: Applied automatically in `metamount.sh` (line 214)
- See also: zeromount-reference.md § susfs_integration.sh

### "I need to detect and resolve module conflicts"
**→ Use Zero-Mount conflict detection**
- Script: `/home/claudetest/zero-mount/nomount/module/zm-diag.sh`
- Command: `zm-diag.sh conflicts`
- Checks: File overlaps between modules with collision resolution
- See also: zeromount-reference.md § Diagnostic CLI

### "WebUI loads slowly / I want instant page load"
**→ Use daemon-generated status cache**
- Cache file: `/data/adb/zeromount/.status_cache.json` (30s TTL)
- Generator: `monitor.sh:65-88` (every 5 seconds)
- Consumer: `api.ts::getStatusCache()` (fast path on load)
- Fields: engineActive, rulesCount, excludedCount, driverVersion, etc.
- See also: zeromount-reference.md § Monitor.sh, integration-map.md § Performance

### "I need to trace a function call across both projects"
**→ Use function-index.json with jq**
- Command: `jq '.[] | select(.name == "zeromount_is_uid_blocked")' function-index.json`
- Includes: All functions, declarations, cross-references
- See also: function-index.json

---

## System Architecture Overview

### Unified Exclusion Pipeline

```
┌────────────────────────────────────────────────────────────────────┐
│                   WebUI (React/SolidJS)                            │
│                   ConfigTab.tsx - Exclusion List                  │
└──────────────────────────┬─────────────────────────────────────────┘
                           │ User clicks "Exclude App"
                           ▼
┌────────────────────────────────────────────────────────────────────┐
│                   KSU API Layer (api.ts)                           │
│                   execCommand("zm blk <uid>")                      │
└──────────────────────────┬─────────────────────────────────────────┘
                           │ Callback-based KSU.exec()
                           ▼
┌────────────────────────────────────────────────────────────────────┐
│              Zero-Mount Kernel (zeromount-core.patch)              │
│    zeromount_ioctl_add_uid() → hash_add_rcu(zeromount_uid_ht)     │
└─────┬──────────────────────────────────────────────────────────────┘
      │
      ├─► ZeroMount VFS Hook (getname_flags)
      │   └─► zeromount_is_uid_blocked(uid)
      │       └─► TRUE → bypass VFS redirection (return original path)
      │
      └─► SUSFS Integration (susfs_def.h:94-97)
          └─► susfs_is_uid_zeromount_excluded(uid)
              └─► Calls zeromount_is_uid_blocked(uid)
                  └─► TRUE → bypass ALL hiding:
                      ├─ sus_path (path hiding)
                      ├─ sus_mount (mount hiding)
                      ├─ sus_kstat (stat spoofing)
                      └─ sus_map (map hiding)

Result: Excluded app sees REAL/STOCK filesystem + detects all artifacts
```

### Boot Sequence (Zero-Mount Module)

```
Android Boot
  │
  ├─► post-fs-data (KernelSU hook)
  │   └─► metamount.sh (module/metamount.sh:1-275)
  │       ├─ Bootloop protection (3-strike counter)
  │       ├─ zm clear (wipe stale rules)
  │       ├─ Scan /data/adb/modules/*/system|vendor|product/*
  │       ├─ For each file: zm add /virtual/path /real/module/path
  │       ├─ SUSFS integration: zm_register_rule_with_susfs()
  │       ├─ zm enable (activate VFS hooks)
  │       └─ Start monitor.sh daemon
  │
  └─► late_start service (KernelSU hook)
      └─► service.sh (module/service.sh:1-56)
          ├─ SUSFS hide /dev/zeromount
          ├─ SUSFS hide /sys/kernel/zeromount
          ├─ Apply UID exclusions from .exclusion_list
          └─ Create WebUI symlink
```

### WebUI Component Architecture

```
webui-v2-beta/src/
├─ App.tsx (Root + Tab Router)
│   ├─ StatusTab.tsx (Engine status, system info, performance)
│   ├─ ModulesTab.tsx (KSU modules, scan with 1 shell script)
│   ├─ ConfigTab.tsx (App exclusions, collapsible list)
│   └─ SettingsTab.tsx (Theme, accent color)
│
├─ components/
│   ├─ core/ (Button.tsx, Badge.tsx, Card.tsx)
│   └─ layout/ (Header.tsx, TabBar.tsx)
│
└─ lib/
    ├─ store.ts (SolidJS reactive state with granular loading)
    ├─ api.ts (KSU exec callbacks, shell command execution)
    ├─ ksuApi.ts (KSU native APIs: getPackagesIcons, etc.)
    ├─ theme.ts (6 accent colors, dynamic text contrast)
    └─ constants.ts (File paths, versions)
```

---

## Integration Architecture (SUSFS + Zero-Mount)

### Problem Statement

Previously, `zm blk <uid>` only disabled ZeroMount VFS redirection. SUSFS continued hiding artifacts from ALL non-root apps:
- `/dev/zeromount` (ZeroMount device node)
- `/sys/kernel/zeromount` (debug sysfs)
- Module paths and mounts in `/proc/mounts`
- File stat info via `sus_kstat`

Result: Detector apps couldn't see artifacts despite being "excluded" from redirection.

### Solution (Kernel-Level Integration)

**Cross-patch exported function:**
```c
// zeromount-core.patch:97
bool zeromount_is_uid_blocked(uid_t uid);  // EXPORT_SYMBOL
```

**SUSFS helper (susfs_def.h:91-101):**
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

**3 Modified Locations in susfs.c:**
1. `is_i_uid_in_android_data_not_allowed()` (line 349-354)
2. `is_i_uid_in_sdcard_not_allowed()` (line 356-360)
3. `is_i_uid_not_allowed()` (line 362-367)

Each checks: `if (susfs_is_uid_zeromount_excluded(current_uid().val)) return false;`

**Mount Hiding (50_add_susfs_in_gki-android12-5.10.patch):**
- `show_vfsmnt()` - Hide mounts unless UID excluded
- `show_mountinfo()` - Hide mount info unless UID excluded
- `show_vfsstat()` - Hide mount stats unless UID excluded

### Build Requirements

Both kernel patches must be applied and built together:
```bash
# 1. Apply Zero-Mount patch
patch -p1 < zeromount-core.patch

# 2. Apply SUSFS patches
patch -p1 < 10_enable_susfs_for_ksu.patch
patch -p1 < 50_add_susfs_in_gki-android12-5.10.patch

# 3. Enable both in .config
CONFIG_ZEROMOUNT=y
CONFIG_KSU_SUSFS=y
```

### Module-Level Automation

**susfs_integration.sh** automatically registers SUSFS rules when ZeroMount rules are added:

```bash
# Called from metamount.sh:214
zm_register_rule_with_susfs <vpath> <rpath>
```

**Path Classification Rules:**
| Path Pattern | SUSFS Actions |
|--------------|---------------|
| `*.so, *.jar, *.dex` | sus_path, sus_maps, sus_kstat |
| `/system/bin/*` | sus_path, sus_kstat |
| `/system/fonts/*` | sus_kstat, sus_maps |
| `/system/app/*` | sus_path, sus_kstat, sus_mount_check |
| `/data/adb/*` | sus_path_loop, sus_kstat |

---

## Development Workflow

### Searching This Database

#### By Function Name
```bash
# Find all locations of a function
jq '.[] | select(.name == "function_name")' function-index.json

# Example: Find zeromount_is_uid_blocked
jq '.[] | select(.name | contains("zeromount_is_uid_blocked"))' function-index.json
```

#### By File
```bash
# All functions in a specific file
jq '.[] | select(.file == "fs/susfs.c")' function-index.json

# Example: All Zero-Mount functions
jq '.[] | select(.project == "zeromount")' function-index.json
```

#### By Mechanism/Feature
```bash
# All SUSFS path hiding functions
grep -r "sus_path" susfs-reference.md

# All Zero-Mount redirection code
grep -r "zeromount_getname_hook" zeromount-reference.md
```

### Analyzing Integration Points
```bash
# See all cross-references
grep "CONFIG_ZEROMOUNT\|zeromount_is_uid" integration-map.md

# View kernel patches
grep -A 10 "susfs_is_uid_zeromount_excluded" /home/claudetest/gki-build/susfs4ksu-new/kernel_patches/include/linux/susfs_def.h
```

### Tracing Boot Flow
```bash
# Verify metamount.sh execution
adb shell "su -c 'cat /data/adb/modules/zeromount/logs/frontend/metamount.log'"

# Check service.sh output
adb shell "su -c 'cat /data/adb/modules/zeromount/logs/frontend/service.log'"

# Monitor daemon logs
adb shell "su -c 'tail -f /data/adb/modules/zeromount/logs/frontend/monitor.log'"
```

---

## File Structure Map

```
Project Database Structure
├─ index.md (this file)
│  └─ Quick navigation, common tasks, integration overview
│
├─ susfs-reference.md (507 lines)
│  ├─ Overview (mechanisms, architecture)
│  ├─ Data structures (sus_path, sus_kstat, open_redirect)
│  ├─ IOCTL reference (CMD_SUSFS_*)
│  ├─ Key functions (organized by mechanism)
│  ├─ Integration points (ZeroMount, KernelSU)
│  └─ Build guide + quick reference
│
├─ zeromount-reference.md (646 lines)
│  ├─ Overview (architecture, hash tables, atomic state)
│  ├─ Kernel implementation (data structures, IOCTL handlers)
│  ├─ zm CLI reference (add, del, clear, blk, unb, list, ver, enable/disable)
│  ├─ Module scripts (metamount.sh, service.sh, monitor.sh, logging.sh)
│  ├─ WebUI architecture (components, state management, API layer)
│  ├─ Integration points (SUSFS, KernelSU)
│  └─ Build & deployment guide
│
├─ integration-map.md (empty, awaiting completion)
│  └─ To be filled with detailed integration flow diagrams
│
└─ function-index.json (empty, awaiting population)
   └─ To be populated with cross-project function index
```

---

## Key Concepts

### UID Exclusion Semantics (INVERTED LOGIC)

```
zm blk <uid>           → UID BLOCKED (sees REAL files)
No zm blk              → UID NORMAL (sees MODULE files)
Root (UID 0)           → ALWAYS sees REAL files (by design)

Term            Usage              Effect
──────────────────────────────────────────────────────────
"Excluded"      WebUI, user-facing Sees REAL filesystem (inverted)
"Blocked"       Kernel code        Bypasses VFS hooks
```

**Important:** The WebUI terminology ("Excluded") is inverse to kernel terminology ("Blocked").

### Hash Tables vs Linked Lists

**SUSFS:**
- Linked lists: `LH_SUS_PATH_LOOP`, `LH_SUS_PATH_ANDROID_DATA`, `LH_SUS_PATH_SDCARD` (simple iteration)
- Hash tables: `SUS_KSTAT_HLIST`, `OPEN_REDIRECT_HLIST`, `OPEN_REDIRECT_ALL_HLIST` (fast O(1) lookup)

**Zero-Mount:**
- All hash tables: `zeromount_rules_ht`, `zeromount_uid_ht`, `zeromount_dirs_ht`, `zeromount_ino_ht`
- RCU-protected for hot paths

### Thread-Local vs Global State

**SUSFS:** Thread-local via `TIF_PROC_UMOUNTED` flag in `task_struct->thread_info.flags`
- Set by KernelSU's `ksu_handle_setresuid()` when app process spawned
- Checked in every VFS hook to determine if hiding is active

**Zero-Mount:** Global atomic flag `zeromount_enabled` + per-CPU recursion guard
- Atomic for lock-free operations
- Per-CPU prevents re-entry during `getname_kernel()`

---

## Troubleshooting Guide

| Symptom | Root Cause | Investigation Path |
|---------|-----------|-------------------|
| `/dev/zeromount` missing | Kernel not patched OR driver not loaded | Check: `adb shell "su -c 'ls -l /dev/zeromount'"` → See zeromount-reference.md § Troubleshooting |
| Rules not applying | Engine disabled | Check: `adb shell "su -c 'zm ver'"` → See zeromount-reference.md § Quick Reference |
| App still sees module files | UID not excluded OR rule missing | Check: `adb shell "su -c 'zm list \| grep <app_path>'"` → Verify in WebUI |
| WebUI loads slowly | Cache miss on startup | Check: `/data/adb/zeromount/.status_cache.json` age → Monitor daemon running? |
| Bootloop detected | Config corruption | Auto-recovery: metamount.sh 3-strike counter → Manual fix: Clear `/data/adb/zeromount/.config` |
| SUSFS paths not hiding | Integration not applied | Check kernel config: `grep CONFIG_ZEROMOUNT` → Build status in integration-map.md |

---

## Version Information

| Component | Version | Path | Notes |
|-----------|---------|------|-------|
| SUSFS | v2.0.0 GKI | `/home/claudetest/gki-build/susfs4ksu-new/` | Supports Android 12+ with kernel >= 5.0.0 |
| Zero-Mount | v3.2.0 | `/home/claudetest/zero-mount/nomount/` | Latest WebUI v2-beta with optimizations |
| WebUI | v2-beta | `webui-v2-beta/src/` | SolidJS, instant load via cache, collapsible lists |
| Android Support | 12-14 | GKI builds | Tested on android12/5.10, android13/5.15 |

---

## Quick Command Reference

### Device Status
```bash
# Check ZeroMount driver and version
adb shell "su -c 'zm ver'"

# Count active rules
adb shell "su -c 'zm list | wc -l'"

# Check if engine is active
adb shell "su -c 'zm list' | wc -l" && echo "Engine active" || echo "Engine inactive"
```

### WebUI
```bash
# Build and deploy
cd /home/claudetest/zero-mount/nomount/webui-v2-beta && pnpm build
adb push module/webroot-beta/* /data/local/tmp/

# Quick development cycle
adb shell "su -c 'rm -rf /data/adb/modules/zeromount/webroot/assets && cp -r /data/local/tmp/assets /data/adb/modules/zeromount/webroot/'"
```

### Debugging
```bash
# View daemon cache
adb shell "su -c 'cat /data/adb/zeromount/.status_cache.json | jq .'"

# Regenerate app list
adb shell "su -c '/data/adb/modules/zeromount/refresh_apps.sh'"

# Diagnostic CLI
adb shell "su -c '/data/adb/modules/zeromount/zm-diag.sh status'"
```

---

## Related Documentation

- **Project CLAUDE.md:** `/home/claudetest/zero-mount/nomount/.claude/CLAUDE.md` (architecture, boot sequence, WebUI patterns)
- **Session Roundups:** `/home/claudetest/zero-mount/nomount/.claude/session-roundup.md` (work history)
- **Progress Tracking:** `/home/claudetest/zero-mount/nomount/.claude/progress.json` (feature status)

---

## Contact & Attribution

**Projects:**
- **SUSFS:** Developed by SUSFS team, integrated for KernelSU
- **Zero-Mount:** Educational research project (MIT Graduate Independent Study)
- **Integration:** Custom kernel patches for unified exclusion control

**Documentation Generated:** 2026-01-31
**Database Purpose:** Educational reference for Android security research

---

*This database is a living reference. Update relevant sections when architecture changes or new features are added.*
