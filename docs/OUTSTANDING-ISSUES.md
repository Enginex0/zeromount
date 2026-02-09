# Outstanding Issues -- ZeroMount v2.0.0-dev

> **Created:** Session 20 (2026-02-09)
> **Purpose:** Complete handoff document for a new Claude session to investigate and fix remaining issues.
> **Last verified commit:** `9a3fd45 fix(webui): mount mode status display, reboot toast, and header glow`

---

## Table of Contents

1. [Boot Flow Architecture (Critical Context)](#1-boot-flow-architecture)
2. [CRITICAL: Boot Blocking on Overlay/Magic Mount](#2-critical-boot-blocking)
3. [CRITICAL: metamount.sh Is Never Called](#3-critical-metamountsh-orphan)
4. [MEDIUM: VFS Ioctl Bugs (A1, A2, A3)](#4-medium-vfs-ioctl-bugs)
5. [MEDIUM: probe_custom_cmd Mutation Risk (A4)](#5-medium-probe-mutation-risk)
6. [LOW: Shell Script Gaps (A8)](#6-low-shell-script-gaps)
7. [LOW: Dead Code -- monitor.sh (B1-B6)](#7-low-dead-code-monitorsh)
8. [LOW: package.sh ZIP Append Bug](#8-low-packagesh-zip-append)
9. [FIXED This Session](#9-fixed-this-session)
10. [Reference: File Map and Key Functions](#10-reference)

---

## 1. Boot Flow Architecture

Understanding the boot sequence is essential for all issues below.

### KSU Module Boot Sequence

```
BOOT STAGE 1: post-fs-data (blocking, 10s KSU timeout)
  └── post-fs-data.sh:17  →  zeromount detect
      └── handlers.rs:54  →  handle_detect()
          └── detect/mod.rs:74  →  detect_and_persist()
              ├── Probes VFS driver (/dev/zeromount)
              ├── Probes SUSFS (kernel supercall)
              ├── Determines scenario: Full|SusfsFrontend|KernelOnly|SusfsOnly|None
              └── Writes /data/adb/zeromount/.detection.json

BOOT STAGE 2: late_start service (non-blocking, runs in background)
  └── service.sh:15  →  zeromount mount --post-boot
      └── handlers.rs:8  →  handle_mount(post_boot=true)
          └── watcher.rs:260  →  start_module_watcher()
              └── BLOCKS FOREVER in infinite poll loop (watcher.rs:172)
              └── Callback runs full pipeline on inotify events
              └── NO initial scan -- only watches for CHANGES

NEVER CALLED DURING BOOT:
  └── metamount.sh:21  →  zeromount mount (without --post-boot)
      └── handlers.rs:29  →  handle_mount(post_boot=false)
          └── pipeline.rs:518  →  run_pipeline_with_bootloop_guard()
              └── Full pipeline: detect → scan → plan → execute → finalize
```

### Key Insight

The FULL mount pipeline (detect → scan → plan → execute → finalize) is only triggered by:
1. The watcher callback when a module changes (after boot)
2. `metamount.sh` -- but nothing calls it

KSU itself handles basic module mounting via its own magic mount. ZeroMount's pipeline adds VFS redirection ON TOP. So modules still work without ZeroMount's pipeline running -- but the VFS/overlay/magic mount strategy selection only takes effect when the pipeline runs.

---

## 2. CRITICAL: Boot Blocking on Overlay/Magic Mount

### Symptom

User reports: "overlayfs/magic mount mode causes very slow boot like blocking boot"

### Root Cause Analysis

When the pipeline DOES run (via watcher callback or if metamount.sh gets wired), overlay and magic mount modes perform heavy synchronous I/O:

#### 2a. Storage Initialization (storage.rs)

**EROFS image creation** -- `storage.rs:255-294`
```rust
// Line 258-268: Blocks 5-30s depending on module size
Command::new("mkfs.erofs")
    .args(["-z", "lz4hc", "-x", "256"])
    .arg(&image_path)
    .arg(base_path)
    .output()  // SYNCHRONOUS -- no timeout
```

**Ext4 image creation** -- `storage.rs:328-359`
```rust
// Line 332-341: dd creates sparse image (blocks)
Command::new("dd")
    .args(["if=/dev/zero", &format!("of={}", image_path.display()), "bs=1M", "count=0", "seek=2048"])
    .output()  // SYNCHRONOUS

// Line 349-353: mkfs.ext4 formats it (blocks again)
Command::new("mkfs.ext4")
    .args(["-O", "^has_journal"])
    .arg(&image_path)
    .output()  // SYNCHRONOUS
```

**Storage cascade fallback** -- `storage.rs:100-125`
If user selects "auto" and EROFS fails, system tries ext4 next, then tmpfs. Each failure adds its full creation time before falling back. Worst case: EROFS fail (30s) + ext4 fail (45s) before tmpfs succeeds.

#### 2b. Per-File Bind Mounts (magic.rs)

**magic.rs:29-102** -- For each file in each module:
```rust
for file in &module.files {
    match bind_mount_file(&source, &target) {  // Line 35-50
        // Each call does: libc::mount(..., MS_BIND, ...) -- one syscall per file
    }
}
```

A module with 500 files = 500 sequential mount syscalls. Modules like LSPosed can have 1000+ files.

#### 2c. Recursive Directory Copy (magic.rs)

**magic.rs:226-254** -- `copy_dir_recursive()` walks the entire module tree:
```rust
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)?.flatten() {
        let metadata = fs::symlink_metadata(&src_path)?;  // stat per file
        if metadata.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;    // recursive
        } else {
            fs::copy(&src_path, &dst_path)?;               // copy per file
        }
    }
}
```

#### 2d. Overlay Lower Dir Preparation (executor.rs)

**executor.rs:108-148** -- `prepare_lower_dir()` copies module files into staging:
```rust
for file in &module.files {
    if src.exists() {
        fs::copy(&src, &dst)?;  // Line 139: sync copy per file
    }
}
```

### Contrast with VFS Mode

VFS mode is near-instant because:
- No filesystem creation
- No file copying
- No mount syscalls per file
- Just ioctl calls to kernel driver (microseconds each)

### Recommended Fixes

1. **Add timeouts** to `Command::new()` calls in storage.rs (use `.timeout()` or spawn + wait_timeout)
2. **Skip EROFS/ext4 on auto mode** if device is booting -- use tmpfs by default on boot, create EROFS in background later
3. **Batch mount syscalls** in magic.rs if possible, or consider using a single recursive bind mount
4. **Add timeout to metamount.sh** (line 21): `timeout 60 "$BIN" mount`
5. **Consider async overlay preparation** -- prep lower dirs in parallel per partition

---

## 3. CRITICAL: metamount.sh Is Never Called

### The Problem

`metamount.sh` contains:
- Bootloop protection (counter increment, threshold check, config restore)
- The actual mount pipeline call: `$BIN mount` (handlers.rs:29-42)

But nothing in the boot sequence calls it:
- `post-fs-data.sh` calls `zeromount detect` only
- `service.sh` calls `zeromount mount --post-boot` (the watcher, NOT the pipeline)
- KSU does not auto-call custom-named scripts like `metamount.sh`

### Impact

1. The mount pipeline never runs at boot -- only when the watcher detects a module change
2. Bootloop protection in metamount.sh is dead code
3. Mode selection (VFS/overlay/magic) doesn't take effect until something triggers the watcher

### Evidence

- `post-fs-data.sh:17` -- only calls `$BIN detect`
- `service.sh:15` -- calls `$BIN mount --post-boot`
- `handlers.rs:9-27` -- `handle_mount(true)` starts watcher, never runs pipeline
- `watcher.rs:260-279` -- `start_module_watcher()` blocks forever in poll loop, no initial scan
- `metamount.sh:21` -- `$BIN mount` would run the pipeline, but no script calls metamount.sh

### Recommended Fix

Option A: Have `service.sh` call `metamount.sh` before starting the watcher:
```sh
# Run initial mount pipeline with bootloop guard
"$MODDIR/metamount.sh"
# Then start watcher for hot-reload
"$BIN" mount --post-boot
```

Option B: Add initial pipeline run inside `handle_mount(post_boot=true)` before starting watcher:
```rust
pub fn handle_mount(post_boot: bool) -> Result<()> {
    if post_boot {
        // Run pipeline once at boot
        let config = ZeroMountConfig::load(None)?;
        run_pipeline_with_bootloop_guard(config)?;
        // Then watch for changes
        start_module_watcher(modules_dir, || { ... })?;
    }
    ...
}
```

Option B is cleaner because it keeps the bootloop guard and pipeline in Rust, avoids shell → binary → shell indirection, and the watcher already has the callback for re-runs.

---

## 4. MEDIUM: VFS Ioctl Bugs

### A1: get_status() Always Returns enabled=false

**File:** `src/vfs/ioctls.rs:307-328`

`IOCTL_GET_STATUS` is defined as `_IOR` (read from kernel). The kernel writes data to the buffer and returns 0 on success. But the current code reads `enabled` from the ioctl return value (`ret != 0`), which is always 0 on success.

```rust
// Line 315-318 (approximate -- verify current state)
let ret = unsafe { raw_ioctl(self.raw_fd(), IOCTL_GET_STATUS, buf.as_mut_ptr() as *mut _) };
// 'ret' is 0 on success, but code uses it as enabled flag
```

**Same pattern as C4** (get_version), which was already fixed in session 19 by reading both return value AND buffer. Apply same fix here.

**Impact:** `zeromount status` and WebUI always show engine as inactive.

### A2: get_list() Ioctl Size Mismatch

**File:** `src/vfs/ioctls.rs:254-267`

`IOCTL_GET_LIST` is defined as `ior(0x5A, 7, 4)` -- encoded size is 4 bytes. But `get_list()` passes a 65536-byte buffer and expects `ret` to be the byte count.

If the kernel respects the encoded `_IOR` size, `copy_to_user` only copies 4 bytes. If the kernel ignores encoded size and uses its own protocol, this works but is undocumented.

**Action needed:** Verify kernel driver source to determine which behavior is correct. If kernel uses its own size, document it. If kernel uses encoded size, fix the ioctl definition to match the actual buffer size, or change the call convention.

### A3: Config Load Blocks All Subcommands on Fresh Install

**File:** `src/main.rs:44` (approximate)

Config load uses `?` -- if config file is missing/malformed, ALL subcommands fail including `zeromount version` and `zeromount detect`.

On fresh install, `customize.sh:52` runs `$BIN config defaults` to CREATE the config, but that subcommand also needs config to load first (chicken-and-egg).

**Action needed:** Verify if `ZeroMountConfig::load(None)` falls back to defaults on missing file. If yes, this is a false positive. If no, add `unwrap_or_default()`.

**Evidence from config.rs:** `load()` at line 283 calls `resolve()` which reads the file. Need to verify if it returns `Err` or falls back to `Default::default()` on missing file.

---

## 5. MEDIUM: probe_custom_cmd Mutation Risk (A4)

**File:** `src/susfs/mod.rs:556-582`

`probe_custom_cmd()` sends full-sized zeroed structs to the kernel to test if a command is supported. If the kernel handler accepts empty input, it may add garbage entries (empty pathnames, ino=0) to kernel state.

```rust
fn probe_custom_cmd(cmd: SusfsCommand) -> bool {
    // Sends zeroed struct to kernel -- may mutate state
    match cmd {
        SusfsCommand::AddSusKstatRedirect => {
            let mut data = StSusfsSusKstatRedirect { /* all zeros */ };
            // If kernel accepts this, it adds an empty kstat redirect
        }
        ...
    }
}
```

**Impact:** Low risk if kernel validates inputs and rejects empty paths. Higher risk if kernel blindly accepts.

**Recommended fix:** Use a read-only probe mechanism if available (e.g., query commands like ShowVersion first), or check if the kernel returns `EINVAL` for empty inputs without mutating state.

---

## 6. LOW: Shell Script Gaps (A8)

**Files:** `post-fs-data.sh`, `service.sh`, `metamount.sh`

ABI detection `case` blocks have no `*` default case:
```sh
case "$(uname -m)" in
    aarch64) ABI=arm64-v8a ;;
    armv7*|armv8l) ABI=armeabi-v7a ;;
    x86_64) ABI=x86_64 ;;
    i686|i386) ABI=x86 ;;
esac
```

Unknown arch leaves `ABI` unset → `BIN` becomes `$MODDIR/bin//zeromount` (double slash). Scripts exit gracefully via `[ -x "$BIN" ]` but error message is unclear.

Note: `customize.sh:19` handles this correctly with `abort "! Unsupported architecture"`.

**Fix:** Add `*) exit 0 ;;` to other scripts matching customize.sh's pattern.

---

## 7. LOW: Dead Code -- monitor.sh (B1-B6)

`module/monitor.sh` (328 lines) has 5 bugs but is dead code:
- B1: `register_module()` doubles paths (line 155)
- B2: `sync_module()` tracking format mismatch (lines 199-201)
- B3: `log_init` calls undefined `logging.sh` (line 22)
- B4: In `package.sh` SCRIPTS array but serves no purpose (Rust `mount --post-boot` replaces it)
- B6: No script launches it (no entry point)

`service.sh` calls `$BIN mount --post-boot` (Rust binary watcher), not `monitor.sh`.

**Recommended fix:** Delete `module/monitor.sh` and remove from `package.sh` SCRIPTS array (line 41).

---

## 8. LOW: package.sh ZIP Append Bug

**File:** `scripts/package.sh:187`

```sh
(cd "$STAGING" && zip -r9 "$OUT_PATH" .)
```

`zip -r9` appends to existing ZIPs rather than overwriting. If `zeromount-v2.0.0-dev.zip` exists from a previous build, old hash-named assets accumulate in the ZIP alongside new ones.

**Fix:** Add `rm -f "$OUT_PATH"` before the zip command:
```sh
rm -f "$OUT_PATH"
(cd "$STAGING" && zip -r9 "$OUT_PATH" .)
```

---

## 9. FIXED This Session (Session 20)

### WebUI Fixes (commit 9a3fd45)

1. **NONE mode color** -- Changed from red (`colorError`) to green (`colorSuccess`) at `StatusTab.tsx:65`. NONE = KSU default magic mount = valid mode, not error.
2. **NONE mode label** -- Changed from "Fallback" to "Default" at `StatusTab.tsx:51`.
3. **Mount mode descriptions** -- Added `mountModeDescription()` memo showing strategy details + storage backend for each mode.
4. **Reboot hint** -- "Switching mode requires reboot" text on both StatusTab and SettingsTab.
5. **Toast on strategy change** -- `store.ts:577`: `showToast('Mount strategy changed — reboot to apply', 'info')` after successful save.
6. **Header subtitle glow** -- Changed from accent-following gradient to fixed gold shimmer (`Header.css`), independent of accent color.

### Previously Fixed (Sessions 7-19)

- C1: `probe_custom_cmd` buffer sizes (session 19)
- C3: Logging overhaul (session 19)
- C4: `get_version()` ioctl dual-read (session 19)
- H8: `detect` command stdout output (session 19)
- H11: `status` reads stale file (session 19)
- Binary gate removal in detect/susfs.rs (session 19)
- WebUI dark chip contrast (session 19)
- SUSFS sync, activity feed, UX polish (commit 7dcb583)
- Mount engine settings + storage config (commit e97eb10)

---

## 10. Reference: File Map and Key Functions

### Boot-Critical Files

| File | Lines | Purpose |
|------|-------|---------|
| `module/post-fs-data.sh` | 18 | Detection phase (10s KSU timeout) |
| `module/service.sh` | 16 | Starts module watcher (blocks forever) |
| `module/metamount.sh` | 22 | Full pipeline + bootloop guard (NEVER CALLED) |
| `module/customize.sh` | 71 | Install hook |
| `src/cli/handlers.rs` | 352 | All CLI command handlers |
| `src/core/pipeline.rs` | 756 | Typestate mount pipeline |
| `src/detect/watcher.rs` | 361 | Inotify watcher (blocks forever at line 172) |

### Mount Engine Files

| File | Lines | Blocking Ops | Purpose |
|------|-------|-------------|---------|
| `src/mount/executor.rs` | 148 | prepare_lower_dir (fs::copy per file) | Dispatches to overlay/magic |
| `src/mount/overlay.rs` | 290 | libc::mount, fsopen/fsmount syscalls | OverlayFS mounting |
| `src/mount/magic.rs` | 255 | libc::mount per file, copy_dir_recursive | Bind mount per file |
| `src/mount/storage.rs` | 440 | mkfs.erofs, dd, mkfs.ext4, libc::mount | Storage backend creation |
| `src/mount/planner.rs` | 304 | None (pure analysis) | Mount plan generation |
| `src/mount/umount.rs` | 41 | libc::umount2 | Teardown |

### VFS Driver Files

| File | Lines | Purpose |
|------|-------|---------|
| `src/vfs/ioctls.rs` | 366 | All ioctl definitions and VfsDriver methods |
| `src/vfs/executor.rs` | 267 | VFS rule injection pipeline |
| `src/vfs/types.rs` | 103 | IoctlData, VfsRule, VfsStatus structs |

### Key Ioctl Definitions (ioctls.rs)

```
IOCTL_GET_VERSION: ior(0x5A, 4, 4)   -- returns driver version
IOCTL_GET_STATUS:  ior(0x5A, 11, 4)  -- returns engine status (BUG: A1)
IOCTL_GET_LIST:    ior(0x5A, 7, 4)   -- returns rule list (BUG: A2)
IOCTL_ADD_RULE:    iow(0x5A, 1, 24)  -- inject VFS rule
IOCTL_CLEAR_ALL:   io(0x5A, 3)       -- flush all rules
IOCTL_ENABLE:      io(0x5A, 8)       -- activate engine
IOCTL_DISABLE:     io(0x5A, 9)       -- deactivate engine
```

### Pipeline Typestate Chain (pipeline.rs)

```
Init → Detected → Planned → Mounted → Finalized
  │        │          │         │          │
  new()  detect()  scan_and   execute()  finalize()
                   _plan()
```

Each transition is a consuming move. Convenience function:
```rust
run_pipeline_with_bootloop_guard(config) → RuntimeState
  └── check_bootloop() → increment_bootcount() → run_full_pipeline()
```

### SUSFS Supercall Mechanism (ffi.rs)

```rust
fn supercall(cmd: SusfsCommand, data: *mut u8) -> Result<i32, i32> {
    libc::syscall(
        __NR_PRCTL_OR_EQUIVALENT,  // KSU hook
        KSU_INSTALL_MAGIC1,         // 0xDEADBEEF
        SUSFS_MAGIC,                // 0xFAFAFAFA
        cmd as u32,                 // command code
        data,                       // struct pointer
    )
}
```

### Cross-Compilation

```
Toolchain: /home/president/.cargo/bin/ (all 4 Android targets)
NDK: /opt/android-ndk-r25b/
Build: scripts/package.sh --build
Targets: arm64-v8a, armeabi-v7a, x86_64, x86
```

### Device Under Test

- Redmi 14C, Android 14, KernelSU Next
- ADB ID: QCFAGMU8S8GAPNW8
- Module path: /data/adb/modules/zeromount/
- Data path: /data/adb/zeromount/
- WebRoot: /data/adb/modules/zeromount/webroot/

---

## Priority Order for Next Session

1. **Wire metamount.sh into boot** (or add initial pipeline run to mount --post-boot handler)
2. **Fix A1** (get_status ioctl -- same pattern as already-fixed C4)
3. **Investigate A2** (get_list ioctl size -- needs kernel source verification)
4. **Fix A3** (config load fallback on fresh install)
5. **Add storage timeouts** (storage.rs Command::new calls)
6. **Delete monitor.sh** (dead code)
7. **Fix package.sh zip append**
8. **Add default ABI case** to shell scripts
