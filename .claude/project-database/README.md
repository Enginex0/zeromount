# Function Index Database - Zero-Mount

## Overview

Complete function index for the Zero-Mount system, documenting 129+ functions across kernel, shell, and WebUI layers.

## Files in This Database

### function-index.json (PRIMARY)
- **Size:** 32 KB
- **Entries:** 129 functions
- **Structure:** Hierarchical JSON organized by subsystem
- **Format:** Each function includes:
  - File path
  - Line number
  - Function signature
  - Description
  - Additional metadata (integration points, notes)

### Organization

```
function-index.json
├── metadata (version, project info)
├── susfs_kernel (20 functions)
│   ├── susfs_init, susfs_add_sus_path, susfs_add_sus_kstat, ...
│   ├── SUSFS path hiding, stat spoofing, mount hiding
│   └── Integration with ZeroMount via susfs_is_uid_zeromount_excluded()
├── zeromount_kernel (13 functions)
│   ├── zeromount_is_uid_blocked (EXPORTED)
│   ├── VFS rule management (add/del/clear)
│   └── UID exclusion (blk/unb)
├── shell_functions (36 functions across 4 scripts)
│   ├── logging.sh (13 functions)
│   ├── susfs_integration.sh (18 functions)
│   ├── metamount.sh (5 functions)
│   └── monitor.sh (14 functions)
├── webui_typescript (44 functions across 3 files)
│   ├── api.ts (28 functions)
│   ├── ksuApi.ts (4 functions)
│   └── store.ts (16 functions)
├── component_architecture (kernel hooks, daemon services)
├── cross_component_flows (data flow diagrams)
└── index_notes (critical semantic information)
```

## Key Sections

### Kernel Functions

**SUSFS (fs/susfs.c)**
- Path hiding: `susfs_add_sus_path`, `susfs_add_sus_path_loop`
- Stat spoofing: `susfs_add_sus_kstat_statically`, `susfs_add_sus_kstat_redirect`
- Mount hiding: `susfs_add_sus_mount`
- Map hiding: `susfs_add_sus_map`
- Open redirect: `susfs_add_open_redirect`, `susfs_add_open_redirect_all`

**ZeroMount (fs/zeromount.c)**
- Rule management: `zeromount_ioctl_add_rule`, `zeromount_ioctl_del_rule`, `zeromount_ioctl_clear_rules`
- UID exclusion: `zeromount_ioctl_add_uid`, `zeromount_ioctl_del_uid`
- Path resolution: `zeromount_resolve_path`, `zeromount_match_path`
- CRITICAL: `zeromount_is_uid_blocked()` (EXPORTED for SUSFS integration)

### Shell Functions

**logging.sh (13 functions)**
- Structured logging with levels (ERROR, WARN, INFO, DEBUG, TRACE)
- Automatic log rotation and archival

**susfs_integration.sh (18 functions)**
- SUSFS detection and capability scanning
- Path classification and metadata capture
- Deferred sus_path application (overlays unmounted first)
- Module cleanup on uninstall

**metamount.sh (5 functions)**
- Conflict detection between multiple modules
- Bootloop protection with config backup/restore
- Module registration at boot

**monitor.sh (14 functions)**
- Module polling loop (5 second interval)
- Status cache generation (instant WebUI load)
- App install detection (inotifywait/logcat)
- Dynamic module handling

### WebUI Functions

**api.ts (28 functions)**
- ZeroMount control: `toggleEngine`, `getRules`, `addRule`, `deleteRule`
- UID exclusion: `excludeUid`, `includeUid`, `getExcludedUids`
- System info: `getSystemInfo`, `getVersion`, `getStatusCache`
- Module management: `getModules`, `loadKsuModule`, `unloadKsuModule`
- Performance: Daemon cache (30s validity), parallel APIs

**ksuApi.ts (4 functions)**
- Package listing: `listPackages` (all, user, system)
- Package info: `getPackagesInfo`, `getAppLabelViaAapt`
- Icons: `getPackagesIcons`

**store.ts (16 functions)**
- Solid.js state management
- Data loading with cache fast path
- App polling with trigger file detection
- UI state synchronization

## Critical Semantics

### UID Exclusion (INVERTED LOGIC)
```
zm blk <uid>  → UID is "excluded" (sees REAL files, not redirected)
No zm blk     → UID is "included" (sees MODULE files, redirected)
Root          → Always sees REAL files (by design)
```

### Integration Point
- SUSFS checks: `susfs_is_uid_zeromount_excluded(uid)`
- Calls: `zeromount_is_uid_blocked(uid)` (EXPORTED function)
- Effect: Excluded UID bypasses BOTH VFS redirection AND path hiding

## Performance Optimizations

| Level | Source | Latency | Content |
|-------|--------|---------|---------|
| L1 Cache | monitor.sh (5s) | ~50ms | Full status JSON |
| L2 Cache | api.ts (30s) | ~100ms | Cached system info |
| L3 Full | Parallel APIs | ~1s | Rules, UIDs, modules |

## File Paths

- **Kernel patches:** `/home/claudetest/zero-mount/nomount/patches/`
- **SUSFS source:** `/home/claudetest/gki-build/susfs4ksu-new/kernel_patches/`
- **Shell scripts:** `/home/claudetest/zero-mount/nomount/module/`
- **WebUI source:** `/home/claudetest/zero-mount/nomount/webui-v2-beta/src/`

## Usage

### Quick Lookup
```bash
jq '.susfs_kernel.susfs_init' function-index.json
jq '.webui_typescript.api.ts.getRules' function-index.json
jq '.shell_functions."susfs_integration.sh"' function-index.json
```

### Search
```bash
grep -o '"[^"]*": {' function-index.json | grep -i "path"
```

### Count Functions
```bash
grep -c '"signature"' function-index.json
```

## Navigation

For detailed reference docs, see:
- `zeromount-reference.md` - Architecture and operations
- `susfs-reference.md` - SUSFS kernel integration
- `integration-map.md` - Cross-component flows

---

**Generated:** 2026-01-31  
**Total Functions:** 129  
**File Size:** 32 KB
