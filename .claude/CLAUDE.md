## Project Documentation Protocol

**This project uses structured documentation. Follow these instructions.**

---

### Session Start — READ THESE FIRST

```
1. .claude/session-roundup.md       → Session summaries
2. .claude/progress.json            → Phase: ship
```

**Current state:** v3.2.0 with performance optimizations. WebUI loads instantly via daemon cache. Modules tab uses single shell script. ConfigTab exclusion list collapsible.

---

## Project Context (Zero-Mount)

KernelSU metamodule providing **VFS-level path redirection without mounts**.

**Identity:**
- Name: `Zero-Mount` (with hyphen)
- Binary: `zm`
- Device: `/dev/zeromount`
- Kernel config: `CONFIG_ZEROMOUNT`

---

## CRITICAL: Architecture Understanding

### ZeroMount vs SUSFS — DIFFERENT SYSTEMS

```
┌─────────────────────────────────────────────────────────────┐
│                        ZeroMount                             │
│           (VFS Path REDIRECTION - returns different file)    │
│                                                              │
│  Hook: getname_flags() in fs/namei.c                        │
│  Effect: App opens /system/foo → kernel serves /data/.../foo │
│  Storage: Kernel memory hash tables (NOT persistent)        │
│  Commands: zm add/del/blk/unb/enable/disable/list/ver       │
└─────────────────────────────────────────────────────────────┘
                              +
┌─────────────────────────────────────────────────────────────┐
│                         SUSFS                                │
│           (Path HIDING - makes files INVISIBLE)              │
│                                                              │
│  Effect: App tries to access path → returns ENOENT           │
│  NOT for content redirection - purely for stealth            │
│  Commands: ksu_susfs add_sus_path, add_sus_kstat, etc.      │
└─────────────────────────────────────────────────────────────┘
```

**KEY INSIGHT:** SUSFS hides paths (file not found). ZeroMount redirects paths (different content).

---

### SUSFS + ZeroMount Integration (2026-01-30)

**Problem solved:** Previously, `zm blk <uid>` only affected ZeroMount VFS redirection. SUSFS continued hiding `/dev/zeromount`, module paths, and mounts from ALL non-root apps regardless of exclusion.

**Solution:** Cross-patch kernel integration via exported function.

```
┌─────────────────────────────────────────────────────────────┐
│                   UNIFIED EXCLUSION FLOW                     │
│                                                              │
│  WebUI: User clicks "Exclude" on detector app               │
│           │                                                  │
│           ▼                                                  │
│  api.ts: zm blk <uid>                                       │
│           │                                                  │
│           ▼                                                  │
│  ZeroMount Kernel: Adds UID to zeromount_uid_ht hash table  │
│           │                                                  │
│           ├──► ZeroMount checks: zeromount_is_uid_blocked() │
│           │    └── TRUE → bypass VFS redirection            │
│           │                                                  │
│           └──► SUSFS checks: susfs_is_uid_zeromount_excluded│
│                └── Calls zeromount_is_uid_blocked()         │
│                    └── TRUE → bypass ALL hiding:            │
│                        ├─ Path hiding (sus_path)            │
│                        ├─ Mount hiding (/proc/mounts)       │
│                        └─ Stat spoofing (sus_kstat)         │
│                                                              │
│  Result: Excluded app sees EVERYTHING real                  │
└─────────────────────────────────────────────────────────────┘
```

**Files modified for integration:**
- `zeromount-core.patch` - Exported `zeromount_is_uid_blocked()`
- `susfs_def.h` - Added `susfs_is_uid_zeromount_excluded()` helper
- `susfs.c` - Modified `is_i_uid_not_allowed()` and `is_i_uid_in_android_data_not_allowed()`
- `50_add_susfs_in_gki-android12-5.10.patch` - Modified mount hiding hooks

**Requires kernel rebuild** with both patches.

---

### BUILD SUCCESS (2026-01-30)

**Status:** GKI build `21525555614` completed successfully.

**Integration Pattern (susfs_def.h):**
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

**Key:** No `zeromount_is_uid_blocked` declaration in `#else` block to avoid conflicts.

**Build command:**
```bash
gh workflow run build.yml --repo Enginex0/kernelsu-next-vanilla \
  -r main -f android_version=android12 -f kernel_version=5.10 \
  -f sub_level=209 -f os_patch_level=2024-05 -f device_codename=lake
```

---

### UID Exclusion Semantics (INVERTED FROM INTUITION)

```
zm blk <uid>     → UID sees REAL/STOCK files (bypass redirection + bypass hiding)
No zm blk        → UID sees MODULE files (redirected) + artifacts hidden
Root (UID 0)     → ALWAYS sees REAL files (by design, security)
```

**"Excluded" in WebUI = blocked = bypasses BOTH ZeroMount AND SUSFS**

| UID State | ZeroMount VFS | SUSFS Hiding | What App Sees |
|-----------|---------------|--------------|---------------|
| Excluded (`zm blk`) | Bypassed | Bypassed | REAL filesystem + all artifacts |
| Normal (not blocked) | Active | Active | MODULE files + hidden artifacts |
| Root | Bypassed | Bypassed | REAL filesystem (by design) |

This is intentional: detection apps are "excluded" so they see real files AND can detect artifacts like `/dev/zeromount`.

---

### VFS Hook Flow (When App Opens File)

```
App calls open("/system/framework/services.jar")
        │
        ▼
    Kernel VFS
        │
        ▼
    getname_flags() ←── ZeroMount hook here
        │
        ├── Is ZeroMount enabled? (atomic flag)
        │   └── NO → return original path
        │
        ├── Is caller UID blocked? (zm blk was called)
        │   └── YES → return original path (sees REAL file)
        │
        ├── Is caller root/critical process?
        │   └── YES → return original path (sees REAL file)
        │
        └── Look up path in rules hash table
            ├── NO MATCH → return original path
            └── MATCH → replace with module path, return redirected
```

---

### Boot Sequence

```
1. [KSU metamodule hook] metamount.sh
   ├── Clear stale rules: zm clear
   ├── Scan /data/adb/modules/*/system|vendor|product/*
   ├── For each file: zm add /virtual/path /real/module/path
   ├── Enable engine: zm enable
   └── Start monitor daemon

2. [Late service] service.sh
   ├── Hide artifacts: susfs add_sus_path_loop /dev/zeromount
   ├── Apply exclusions: for uid in .exclusion_list: zm blk $uid
   ├── Create symlink: webroot/link → /data/adb/zeromount
   └── Generate app list (background)

3. [Monitor daemon] monitor.sh (continuous)
   ├── Poll module changes every 5s
   ├── Generate .status_cache.json every 5s (for instant WebUI load)
   ├── Watch app installs/uninstalls
   └── Update .refresh_trigger on changes
```

---

### zm CLI Reference

```bash
zm add <virtual_path> <real_path>  # Add redirection rule
zm del <virtual_path>               # Delete rule
zm clear                            # Clear all rules and UIDs
zm blk <uid>                        # Block UID (sees real files)
zm unb <uid>                        # Unblock UID
zm list                             # List rules (format: real->virtual)
zm ver                              # Get version (returns 1)
zm enable                           # Enable VFS hooks
zm disable                          # Disable VFS hooks
```

**IOCTL codes:** Magic byte `0x5A` ('Z')

---

### Testing VFS Redirection

**IMPORTANT:** Root and `su <uid>` see REAL files by design!

To test redirection, you need:
1. A real app process (not root shell)
2. App UID NOT in exclusion list
3. Engine enabled (`zm enable`)
4. Valid rule exists (`zm list | grep <path>`)

**Cannot test via:**
- `su -c 'cat /system/...'` (root bypasses hooks)
- `su <uid> -c '...'` (still has root capabilities)

---

## WebUI Architecture

### KSU API Patterns

**EXEC - Callback Pattern (NOT Promises):**
```javascript
const callbackName = `exec_cb_${Date.now()}`;
window[callbackName] = (errno, stdout, stderr) => {
    delete window[callbackName];
    resolve({ errno, stdout, stderr });
};
ksu.exec(cmd, '{}', callbackName);
```

**ICON API:**
```javascript
const ksu = globalThis.ksu;  // NOT window.ksu
const result = ksu.getPackagesIcons(JSON.stringify([packageName]), 100);
const parsed = JSON.parse(result);
imgEl.src = parsed[0].icon;  // base64 PNG
```

---

### App List Architecture

```
Boot (service.sh)                    Runtime (WebUI)
─────────────────────────────────────────────────────────────
refresh_apps.sh                      ConfigTab.tsx
├─ pm list packages -U (ALL apps)    ├─ fetch installed_apps.json
├─ aapt dump badging (single call)   ├─ poll .refresh_trigger (5s)
├─ detect isSystemApp by path        ├─ merge new/removed apps
└─ save to installed_apps.json       └─ icons via ksu.getPackagesIcons()

app_monitor.sh (daemon)              store.ts
├─ poll every 5s                     ├─ trigger comparison (null-safe)
├─ diff package list                 └─ existingPkgs inside updater
├─ append/remove from JSON
└─ write .refresh_trigger
```

---

### WebUI Fixes Implemented (2026-01-30)

| Fix | File | Description |
|-----|------|-------------|
| 1.1 | service.sh, refresh_apps.sh | Single aapt call (cached badging) |
| 1.2 | api.ts | Removed 30-retry blocking loop |
| 1.4 | refresh_apps.sh | Architecture detection (arm64/arm) |
| 3.4 | service.sh | Symlink before background tasks |
| 3.3 | refresh_apps.sh | Trigger file on completion |
| 2.1 | ConfigTab.tsx | iconCache at module scope |
| 2.2 | store.ts | Null-safe trigger comparison |
| 2.3 | store.ts | existingPkgs inside updater |
| 2.4 | app_monitor.sh | Initial trigger on start |
| 3.1 | api.ts | 30s timeout on KSU exec |
| 3.2 | app_monitor.sh | Removed dead inotifywait code |

**Also fixed:**
- System apps included in list (pm list packages -U, not -3)
- isSystemApp detection by APK path (/system/, /vendor/, etc.)
- Exclusion metadata persistence (.exclusion_meta.json)
- Dynamic "User Apps" / "All Apps" title based on toggle

---

### WebUI Performance Optimizations

**Daemon Cache (Instant Load):**
```
┌─────────────────────────────────────────────────────────────┐
│  monitor.sh (every 5 seconds)                                │
│  └─ Generates: /data/adb/zeromount/.status_cache.json       │
│     Contains: engineActive, rulesCount, excludedCount,       │
│     driverVersion, kernelVersion, deviceModel, androidVersion│
│     uptime, selinuxStatus, susfsVersion, loadedModules       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  WebUI loadInitialData()                                     │
│  1. Try cache first (single file read, ~50ms)               │
│     ├─ CACHE HIT: Show UI instantly, refresh in background  │
│     └─ CACHE MISS: Fall back to full parallel load          │
│  Cache valid for 30 seconds                                  │
└─────────────────────────────────────────────────────────────┘
```

**API Call Optimizations:**
| Optimization | Impact |
|--------------|--------|
| Removed redundant `getStats()` | Saved 3 shell calls (was re-calling getRules/getExcludedUids) |
| Stats computed locally | `rulesData.length`, `uidsData.length` instead of API call |
| `fetchSystemColor` parallelized | Moved into Promise.allSettled, no longer blocks |
| SUSFS version parallelized | Moved into getSystemInfo Promise.all |

**Modules Tab Optimizations:**
| Optimization | Impact |
|--------------|--------|
| Single shell script for scan | Replaced ~22 separate shell calls with 1 script outputting JSON |
| `awk` for loaded detection | Android-compatible (toybox sed fails with `->` pattern) |
| Chevron expand indicator | Visual affordance for collapsible module details |

**ConfigTab UX:**
- Exclusion list collapsible (collapsed by default)
- Clickable header with rotating chevron
- Badge count visible even when collapsed

---

### Theme & UI Configuration

**Accent Colors (6 total):**
| Color | Hex | Text on Gradient |
|-------|-----|------------------|
| Orange | `#FF8E53` | Dark |
| Emerald | `#00D68F` | Dark |
| Azure | `#00B4D8` | Dark |
| Slate | `#64748B` | Dark |
| Indigo | `#6366F1` | White |
| Coral | `#FF6B6B` | Dark |

**Dynamic Text Contrast:**
- `textOnAccent` CSS variable adapts text color based on gradient luminance
- Shield, "Engine Active" text, buttons all use `--text-on-accent`
- Prevents invisible text on bright/dark accent colors

**Random Accent:**
- Default: ON (randomizes each session)
- Triggers on `visibilitychange` event (handles cached WebViews)
- Toggle OFF: uses saved accent color (static)

**Status Tab Data Sources:**
| Field | Source |
|-------|--------|
| Driver | `zm ver` |
| SUSFS | `ksu_susfs show version` |
| Modules Active | `ksuModules.filter(m => m.isLoaded).length` |
| Uptime | `/proc/uptime` |
| misc | `/dev/zeromount` (static) |

**Header:** "ZEROMOUNT" title with "Enginex0" subtitle.

---

## Key Paths

```
Module scripts:  /home/claudetest/zero-mount/nomount/module/
WebUI source:    /home/claudetest/zero-mount/nomount/webui-v2-beta/
Kernel patches:  /home/claudetest/zero-mount/nomount/patches/
SUSFS source:    /home/claudetest/gki-build/susfs4ksu-new/
```

### Kernel Patch Files (Modified for Integration)

```
ZeroMount:
  patches/zeromount-core.patch
    └─ zeromount_is_uid_blocked() now EXPORTED (was static)

SUSFS (at /home/claudetest/gki-build/susfs4ksu-new/kernel_patches/):
  include/linux/susfs_def.h
    └─ Added susfs_is_uid_zeromount_excluded() helper (line 91-98)

  fs/susfs.c
    └─ is_i_uid_in_android_data_not_allowed() - checks ZeroMount (line 349-354)
    └─ is_i_uid_not_allowed() - checks ZeroMount (line 360-365)

  50_add_susfs_in_gki-android12-5.10.patch
    └─ show_vfsmnt() - mount hiding with ZeroMount check
    └─ show_mountinfo() - mount hiding with ZeroMount check
    └─ show_vfsstat() - mount hiding with ZeroMount check
```

---

## Quick Commands

```bash
# Build WebUI
cd webui-v2-beta && pnpm build

# Quick push to device
adb push module/webroot-beta/index.html /data/local/tmp/
adb push module/webroot-beta/assets/. /data/local/tmp/assets/
adb shell "su -c 'cp /data/local/tmp/index.html /data/adb/modules/zeromount/webroot/ && rm -rf /data/adb/modules/zeromount/webroot/assets && cp -r /data/local/tmp/assets /data/adb/modules/zeromount/webroot/'"

# Push shell scripts
adb push module/service.sh /data/local/tmp/
adb push module/refresh_apps.sh /data/local/tmp/
adb push module/app_monitor.sh /data/local/tmp/
adb shell "su -c 'cp /data/local/tmp/*.sh /data/adb/modules/zeromount/ && chmod 755 /data/adb/modules/zeromount/*.sh'"

# Regenerate app list
adb shell "su -c '/data/adb/modules/zeromount/refresh_apps.sh'"

# Check ZeroMount status
adb shell "su -c 'zm ver && zm list | wc -l'"
```

---

## Rules

1. **Root sees REAL files** — By design, cannot test VFS redirection as root
2. **SUSFS ≠ ZeroMount** — Hiding vs redirection, different mechanisms
3. **Exclusion is inverted** — "Excluded" apps see REAL files, not module files
4. **Test on REAL DEVICE** — Mock tests prove nothing about kernel integration
5. **Single aapt call** — Cache badging output, parse multiple fields
6. **Module scope for caches** — iconCache outside component to persist

---

## Reference Projects

**SUSFS Source:**
- Path: `/home/claudetest/gki-build/susfs4ksu-new/`
- Key: sus_path (hiding), sus_kstat (stat spoofing), open_redirect (root only)

**TrickyAddon:**
- Path: `/home/claudetest/repo-analysis/TA-official-extracted`
- Key: KSU exec callbacks, getPackagesIcons, IntersectionObserver
