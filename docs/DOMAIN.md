# Domain Knowledge

## Required Reading (In Order)

1. `patches/nomount-kernel-5.10.patch` - The complete kernel patch (643-line core + hooks in 4 subsystems)
2. `include/linux/nomount.h` (in patch) - Data structures, constants, API declarations
3. Linux VFS documentation - Virtual Filesystem layer (namei.c, d_path.c, readdir.c)
4. Linux RCU documentation - Read-Copy-Update concurrency model
5. `METAMODULE_COMPLETE_GUIDE.md` - KernelSU metamodule architecture (the bridge between userspace and kernel)
6. `src/zm.c` - Freestanding userspace binary (raw syscalls, no libc)
7. `module/` - Metamodule shell scripts (metamount.sh, service.sh, customize.sh, etc.)

**Status:** [x] Completed

---

## Key Concepts

| Concept | Definition | Why It Matters |
|---------|------------|----------------|
| VFS (Virtual File System) | Linux's abstraction layer that sits between userspace syscalls and actual filesystems | ZeroMount hooks INTO the VFS layer, below mounts, making it invisible to mount-based detection |
| namei / path resolution | The kernel process of converting a string path ("/etc/foo") into an inode | ZeroMount swaps paths HERE, so the kernel opens a different file than requested |
| d_path | Kernel function that converts an inode/dentry back into a human-readable path string | ZeroMount hooks this to prevent real storage paths from leaking via /proc |
| getdents / readdir | Syscalls that list directory contents ("ls") | ZeroMount injects virtual entries so injected files appear in directory listings |
| dentry cache (dcache) | In-memory cache of previous path lookups | Must be invalidated when adding rules, or old cached lookups bypass ZeroMount |
| RCU (Read-Copy-Update) | Lock-free read concurrency primitive | Enables ZeroMount to check rules on every file open without performance penalty |
| inode number (i_ino) | Unique identifier for a file on a filesystem | ZeroMount tracks real file inodes to match permission bypasses and d_path translation |
| UID blocking | Per-Android-app exclusion from ZeroMount effects | Module managers need to see real paths; all other apps see the illusion |
| NOMOUNT_MAGIC_POS | Sentinel value (0x7000000) in directory file position | Separates real directory entries from injected virtual entries in readdir |
| /system prefix | Android mounts system partition at both / and /system | nomount_match_path() handles both so a single rule covers both access patterns |
| Metamodule | KernelSU plugin that controls HOW modules are mounted/installed. Identified by `metamodule=1` in module.prop | ZeroMount IS a metamodule — it replaces the default mounting strategy with kernel-level path redirection |
| metamount.sh | Metamodule's boot-time hook that mounts all regular modules | For ZeroMount: iterates modules and calls ioctl ADD_RULE instead of mount operations |
| metainstall.sh | Hook sourced during installation of regular modules (not the metamodule itself) | Can customize module installation — receives MODPATH, ZIPFILE, install_module() |
| metauninstall.sh | Hook run when regular modules are removed | Cleans up rules/resources the metamodule created for that module |
| skip_mount flag | File at /data/adb/modules/X/skip_mount — metamodule should not mount this module | Module handles itself or is script-only; metamodule must check and respect this |
| disable flag | File at /data/adb/modules/X/disable — metamodule ignores this module entirely | Not mounted, not processed, treated as nonexistent |
| /data/adb/metamodule | Symlink KernelSU creates → /data/adb/modules/<metamodule_id> | Stable access path regardless of metamodule name |
| KSU source tag | Mount source must be set to "KSU" for KernelSU to track/cleanup mounts | NOT needed by ZeroMount since it creates zero mounts |
| notify-module-mounted | Command: `/data/adb/ksud kernel notify-module-mounted` | Tells KernelSU mounting is complete so boot can proceed |
| zm binary | Freestanding C binary with zero libc dependency, uses raw ARM syscalls. Extended with enable/disable commands | The ioctl bridge between shell scripts and the /dev/zeromount kernel driver |
| /dev/zeromount | The project device name (decided in DECISIONS.md #4). `/dev/vfs_helper` was used in a previous dev iteration; `/dev/nomount` is the name in the current patch source pre-rename | All userspace code must reference /dev/zeromount after the rename is applied |
| SUSFS | Separate kernel subsystem for hiding paths, spoofing kstat, hiding mounts/maps | Complementary to ZeroMount: ZeroMount redirects, SUSFS hides evidence |
| kstat_redirect (dual-inode) | Custom SUSFS enhancement: spoofs metadata for BOTH virtual and real inodes | Prevents stat() from detecting inode mismatch between original and redirected file |
| open_redirect | SUSFS feature that redirects open() syscall by inode number | Alternative to ZeroMount's VFS hooks for specific file types like fonts |
| sus_path | SUSFS feature that hides a path from readdir and stat | Used to hide /dev/zeromount and module storage paths from detection apps |
| sus_map | SUSFS feature that hides entries from /proc/pid/maps | Used to hide .so files loaded from module paths |
| Freestanding binary | No libc, no dynamic linker, custom _start(), inline asm syscalls | Runs on any Android regardless of libc version, minimal footprint |
| .exclusion_list | File at /data/adb/zeromount/.exclusion_list with one UID per line | Persists UID exclusions across reboots, replayed by service.sh |

---

## System Behavior

> **Note:** Code examples show pre-rename `nomount_*` identifiers. Per D4, these become `zeromount_*` in v2 implementation.

```
ADDING A RULE (userspace -> kernel):
  userspace ioctl(ADD_RULE, {virtual="/etc/fonts/x", real="/data/adb/modules/fonts/x"})
      |
      v
  nomount_ioctl_add_rule()
      |-- copy paths from userspace
      |-- check CAP_SYS_ADMIN
      |-- hash virtual_path
      |-- create nomount_rule struct
      |-- kern_path(real_path) -> store real_ino, real_dev
      |-- hash_add_rcu to nomount_rules_ht
      |-- if virtual_path doesn't exist on disk:
      |     |-- nomount_auto_inject_parent() -> add to nomount_dirs_ht
      |     |-- set rule->is_new = true
      |-- nomount_flush_dcache(virtual_path) -> invalidate cached lookup

FILE OPEN (process opens a redirected path):
  open("/etc/fonts/x")
      |
      v
  getname_flags() -> nomount_getname_hook()
      |-- check: enabled? UID not blocked? starts with '/'?
      |-- nomount_resolve_path("/etc/fonts/x")
      |     |-- hash path, lookup nomount_rules_ht
      |     |-- found rule with NM_FLAG_ACTIVE
      |     |-- return kstrdup("/data/adb/modules/fonts/x")
      |-- getname_kernel(real_path) -> new filename struct
      |-- putname(old_name)
      |-- kernel opens /data/adb/modules/fonts/x transparently
      v
  inode_permission() -> nomount_is_injected_file()
      |-- real_ino matches rule -> return 0 (allow)

PATH DISPLAY (process reads /proc/self/fd/N):
  readlink(/proc/self/fd/N)
      |
      v
  d_path() -> nomount_get_virtual_path_for_inode()
      |-- scan rules for matching real_ino where is_new=true
      |-- return "/etc/fonts/x" instead of "/data/adb/modules/fonts/x"

DIRECTORY LISTING (ls /etc/fonts/):
  getdents64(fd)
      |
      v
  1. If f_pos < MAGIC_POS: iterate_dir() for real entries
  2. Set pos = MAGIC_POS, start injecting
  3. nomount_inject_dents64() -> find entries in nomount_dirs_ht for this dir
  4. Write fake dirent structs to userspace buffer
  5. On next call, if pos >= MAGIC_POS: skip real iterate, only inject
```

---

## Architecture: The 7 Hook Points

```
  Userspace Process
      |
      |  open("/etc/fonts/x")       readlink(/proc/self/fd/3)     ls /etc/fonts/
      v                              v                              v
  +-----------+                 +-----------+                 +-----------+
  | namei.c   |                 | d_path.c  |                 | readdir.c |
  | getname   |                 | d_path()  |                 | getdents  |
  +-----+-----+                 +-----+-----+                 +-----+-----+
        |                             |                              |
        v                             v                              v
  nomount_getname_hook()    nomount_get_virtual_   nomount_inject_dents64()
  nomount_resolve_path()    path_for_inode()       nomount_find_next_injection()
        |                             |                              |
        +----------+------------------+------------------------------+
                   |
                   v
            nomount_rules_ht (hash table, 1024 buckets, RCU-protected)
            nomount_dirs_ht  (directory injection entries)
            nomount_uid_ht   (per-app opt-out UIDs)

  Additional Hook Points (v2):
      |
      +-- stat hooks (fs/stat.c) — I3: syscall-level stat spoofing
      +-- statfs hooks (fs/statfs.c) — I4: filesystem type spoofing
      +-- xattr hooks (fs/xattr.c) — I5: SELinux context spoofing
      +-- maps hooks (fs/proc/task_mmu.c) — I7: /proc/pid/maps hiding
```

---

## Gotchas & Common Mistakes

| Mistake | Why It's Wrong | Correct Approach |
|---------|----------------|------------------|
| Forgetting dcache invalidation | Cached dentry lookups bypass ZeroMount hooks entirely | Always call nomount_flush_dcache() after adding rules |
| Not handling /system prefix | Android apps access /system/etc/foo and /etc/foo interchangeably | nomount_match_path() strips /system prefix automatically |
| Blocking the module manager UID | Manager can't see real paths to install modules | Add manager UID to nomount_uid_ht so it sees reality |
| Using GFP_KERNEL inside RCU read lock | RCU read sections can't sleep; GFP_KERNEL may sleep | Use GFP_ATOMIC inside rcu_read_lock() sections |
| Forgetting is_new flag semantics | d_path hook only applies to NEW files (not replacements) | Files that already exist at virtual path don't need d_path translation |
| Not checking NOMOUNT_DISABLED() first | Operations proceed even after kill switch activated | Every exported function checks this before any work |
| Hooking overlayfs directly | Android uses overlayfs extensively — direct hooks cause detection flood (I6) | Rely on SUSFS kstat_redirect which fires after overlayfs operations |
| Using VFS-level stat hooks (generic_fillattr, vfs_getattr_nosec) | Conflicts with SUSFS, causes Gboard font crashes (I3) | Use syscall-level hooks (newfstatat, fstatat64) with BIT_SUS_KSTAT cooperation check |

---

## The Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 3: KernelSU Core                                        │
│  - Root access, module management, SELinux, boot lifecycle      │
│  - Manages /data/adb/modules/ directory                         │
│  - Calls metamodule hooks at defined boot stages                │
│  - Does NOT mount anything itself                               │
└────────────────────────┬────────────────────────────────────────┘
                         │ calls metamount.sh at boot step 6
                         │ sources metainstall.sh during module install
                         │ calls metauninstall.sh during module remove
                         v
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 2: Metamodule (meta-zeromount)                           │
│  - metamodule=1 in module.prop                                  │
│  - metamount.sh: iterates modules, calls ioctl ADD_RULE         │
│  - metainstall.sh: hooks regular module installation            │
│  - metauninstall.sh: cleans up on module removal                │
│  - Respects skip_mount and disable flags                        │
│  - THE BRIDGE: translates module file layout → kernel rules     │
└────────────────────────┬────────────────────────────────────────┘
                         │ opens /dev/zeromount
                         │ ioctl(fd, ADD_RULE, {virtual, real, flags})
                         │ ioctl(fd, ADD_UID, uid)
                         v
┌─────────────────────────────────────────────────────────────────┐
│  LAYER 1: Kernel ZeroMount Subsystem (fs/nomount.c)            │
│  - 4 VFS hooks: namei, permissions, d_path, readdir             │
│  - 3 hash tables: rules, dirs, UIDs                             │
│  - RCU concurrency, atomic kill switch                          │
│  - Zero mounts, zero /proc/mounts evidence                      │
└─────────────────────────────────────────────────────────────────┘
```

## Boot Timeline (ZeroMount-specific)

```
BOOT SEQUENCE:
═══════════════════════════════════════════════════════════════
 Kernel init
   └─ fs_initcall: nomount_init() → /dev/zeromount created

 post-fs-data stage:
   1. Common post-fs-data.d/ scripts
   2. KernelSU prunes modules, loads SELinux rules
   3. meta-zeromount's post-fs-data.sh (optional setup)
   4. Regular modules' post-fs-data.sh
   5. Load system.prop
   6. meta-zeromount's metamount.sh ← CRITICAL MOMENT
      │
      ├─ zm enable                       ← enable subsystem
      ├─ zm clear                        ← defensive: clear stale rules
      ├─ for each /data/adb/modules/*:
      │    ├─ skip if disable or skip_mount exists
      │    ├─ walk system/ directory tree
      │    └─ zm add <virtual> <real> for each file
      │
      ├─ ioctl(ADD_UID) for manager app
      └─ notify-module-mounted           ← signal KernelSU boot complete

   7. post-mount stage (post-mount.d, post-mount.sh)

 service stage:
   meta-zeromount's service.sh → regular modules' service.sh

 boot-completed stage:
   meta-zeromount's boot-completed.sh → regular modules'
═══════════════════════════════════════════════════════════════
 From step 6 onward: all file opens are redirected by kernel
```

---

## Gotchas & Common Mistakes (continued)

| Mistake | Why It's Wrong | Correct Approach |
|---------|----------------|------------------|
| Not calling notify-module-mounted | KernelSU doesn't know mounting is done, boot may hang or misbehave | Always call `/data/adb/ksud kernel notify-module-mounted` at end of metamount.sh |
| Ignoring skip_mount flag | Module explicitly opted out of being mounted | Check for skip_mount file before processing each module |
| Ignoring disable flag | Module is completely disabled by user | Check for disable file, skip entirely |
| Trying to install two metamodules | KernelSU enforces single-metamodule constraint | Uninstall first, reboot, install new one |
| Putting heavy logic in shell scripts | Shell is slow and error-prone for complex file tree walks | Use a compiled binary (C/Rust) called from metamount.sh |
| Forgetting metainstall.sh is sourced not executed | It shares the installer's variable/function scope | Don't use exit (kills installer), use return or abort() |

---

## Userspace Architecture

```
MODULE PACKAGE (ZIP):
  module/
  ├── module.prop            metamodule=true, id=zeromount
  ├── customize.sh           Install-time: select arch binary, check /dev/zeromount
  ├── metamount.sh           Boot step 6: iterate modules, call zm add per file
  ├── metainstall.sh         Regular module install hook (pass-through)
  ├── service.sh             Late boot: replay UID exclusions from .exclusion_list
  ├── monitor.sh             Cosmetic: update module.prop description with status
  └── bin/zm                 Freestanding ARM binary (no libc)

DATA DIRECTORY:
  /data/adb/zeromount/
  ├── zeromount.log           Boot log (created by metamount.sh)
  ├── .verbose               Touch to enable verbose logging
  └── .exclusion_list        One UID per line, replayed at service stage
```

### zm Binary (src/zm.c) — The Ioctl Bridge

Freestanding C. No libc, no malloc, no printf. Raw `svc 0` syscalls.

```
Commands:
  zm add <virtual_path> <real_path>   → ioctl ADD_RULE
  zm del <virtual_path>               → ioctl DEL_RULE
  zm clear                            → ioctl CLEAR_ALL
  zm ver                              → ioctl GET_VERSION → prints to stdout
  zm list                             → ioctl GET_LIST → prints to stdout
  zm blk <uid>                        → ioctl ADD_UID
  zm unb <uid>                        → ioctl DEL_UID

Flow: open("/dev/zeromount") → build ioctl_data with argv pointers → ioctl() → exit
```

Auto-detects directory vs file via fstatat() and sets NM_DIR flag.
Converts relative real_path to absolute via getcwd().
Supports aarch64 and arm32 via compile-time #ifdef.

### metamount.sh — The Boot Orchestrator

```
Boot step 6 flow:
  1. Create /data/adb/zeromount/ if missing
  2. Start log file
  3. Check /dev/zeromount exists (self-disable if not)
  4. For each /data/adb/modules/*:
     ├── Skip self (zeromount)
     ├── Skip disabled/removed modules
     ├── For each partition dir (system, vendor, product, system_ext, odm, oem):
     │   └── find -type f | while read → zm add /$relative_path $real_path
     └── Count active modules
  5. Launch monitor.sh in background
```

### Observed Gaps (v0.1.0)

| Gap | Impact |
|-----|--------|
| No `notify-module-mounted` call | KernelSU may not know mounting is complete |
| No `skip_mount` flag check | Modules that opt out of mounting still get processed |
| No `metauninstall.sh` | No cleanup when regular modules are removed |
| Empty `versionCode` in module.prop | KernelSU may reject or mishandle versioning |
| No `CLEAR` at start of metamount.sh | Stale rules from previous boot may persist |
| Shell find+while per file | Slow for large modules; compiled binary would be faster |

---

## Kernel Logging System

ZeroMount uses a runtime-toggleable logging system to prevent performance degradation on hot paths.

```
ZM_LOG() macro:
  if (unlikely(READ_ONCE(zm_debug) >= 1))    ← ~2ns when off
      pr_info("[ZM] " fmt, ...)              ← ~500ns when on

Control: /sys/kernel/zeromount/debug
  0 = off (production)   ~2ns per call
  1 = standard debug     ~500ns per call
  2 = verbose            high-frequency paths included

Boot param: zm_debug=1 on kernel cmdline
```

Key design constraints:
- `unlikely()` — branch predictor optimization, ~500x fewer mispredictions
- `READ_ONCE()` — prevents compiler caching, toggle takes effect immediately
- Arguments not evaluated when off — `ZM_LOG("x=%s", fn())` skips `fn()` entirely
- Rate-limited variant `ZM_LOG_RL()` for paths that fire thousands of times/sec

---

## Clone Analysis Findings (Phase 1: Kernel Layer)

Analysis of the broken previous implementation at `/home/claudetest/gki-build/nomount-vfs-clone/`.
The clone renamed everything to `vfs_dcache` (obfuscation) and expanded from 643 lines to 2885 lines.

### Core Patch: 18 KEEP, 7 FIX, 7 DISCARD

**KEEP (genuine enhancements):**
- Start disabled (`ATOMIC_INIT(0)`) + ENABLE/DISABLE ioctls — prevents boot deadlock
- `d_backing_inode()` instead of `d_inode` — correct for overlayfs
- Hash tables moved from header to .c (fixes bug #f)
- Device check in inode matching (fixes bug #c)
- `kfree_rcu()` for UID deletion (fixes bug #a)
- `nomount_flush_dcache()` path_put fix — memory leak
- `NOMOUNT_MAGIC_POS` as 64-bit ULL — prevents collision
- Per-CPU recursion guard — prevents infinite VFS loops
- `nomount_normalize_path()` — Android /system/etc symlink
- `nomount_is_critical_process()` — prevents boot loops (init, zygote, vold)
- Reverse lookup table `nomount_targets_ht` — O(1) by real_path
- Recursive parent injection — needed for deep paths
- `nomount_dev_open()` root-only check — security hardening
- Centralized CAP_SYS_ADMIN in ioctl dispatch (clone enhancement — original only checks in add_rule)
- Symbol obfuscation (`nomount_*` → `vfs_dcache_*`)
- Mount hiding hash table
- Maps pattern hiding hash table
- SUSFS integration (#ifdef guarded)

**FIX (good idea, bad execution):**
- `nomount_lazy_resolve_real_ino()` — calls kern_path() in hot permission check path (DISASTROUS)
- Stat spoofing functions — 100+ lines of unconditional pr_info spam
- SELinux context function — dead code, not wired into security subsystem
- Statfs spoofing — triple-pass lookup over-engineered
- Partition dev caching — too many hardcoded OEM paths
- Static timestamps (2009) — should match real partition timestamps
- Concurrency barriers — missing read-side `smp_rmb()`

**DISCARD:**
- `nomount_get_partition_dev()` 60-line if-chain — use array directly
- `nomount_cache_partition_metadata()` per-rule — 110 lines of parent-walking
- `nomount_force_refresh_all()` on every add_rule — O(n) waste
- `SET_PARTITION_DEV` ioctl — userspace shouldn't set kernel dev_t
- `AS_NOMOUNT_HAS_HIDDEN` bit 40 — fragile, may collide
- `nomount_is_root_caller()` — redundant with capable()
- All unconditional pr_info() — defeats macro system, detection vector

### 5 Supplementary Patches: ALL KEEP

| Patch | Fixes | Also in Original? |
|-------|-------|-------------------|
| memory-safety | 4 null-deref guards | No |
| concurrency-barriers | ARM64 write reordering | No (clone-specific) |
| logic-api-fix | `free_page`/`__putname` mismatch | **YES — original has same bug** |
| performance-hotpath | pr_info → NM_DBG in hot paths | No |
| fix-null-isb | 4 more null i_sb checks | No |

**New discovery:** `free_page`/`__putname` allocator mismatch bug exists in BOTH original AND clone. Patch A3 fixes it.

### 9 Injection Scripts: 6 Legitimate, 1 Mixed, 2 Scope Creep

| Script | Target | Verdict | Reason |
|--------|--------|---------|--------|
| namei-hooks | fs/namei.c | **KEEP** (fix readlink) | Core 3 hooks + readlink has compile error |
| readdir-hooks | fs/readdir.c | **KEEP** (fix compat) | Missing getdents/compat hooks |
| stat-hooks | fs/stat.c | **KEEP** (syscall-level only — VFS-level hooks removed per I3, conflicted with SUSFS) | New: stat spoofing (original missing) |
| statfs-hooks | fs/statfs.c | **KEEP** | New: statfs spoofing (original missing) |
| xattr-hooks | fs/xattr.c | **KEEP** | New: SELinux context spoofing (fixes bug #h) |
| overlayfs-hooks | fs/overlayfs/inode.c | **DISCARD** | Abandoned per I6 — caused detection flood. SUSFS kstat_redirect handles overlayfs re-spoofing. |
| maps-hooks | fs/proc/task_mmu.c | **FIX** | Keep map hiding, discard lineage hacks |
| base-hide-stuff | fs/proc/base.c | **DISCARD** | ROM fingerprinting — scope creep |
| procmounts-hooks | fs/proc_namespace.c | **DISCARD** | Mount hiding — ZeroMount doesn't create mounts |

### Bug Audit Status (8 original bugs)

| Bug | Clone Status |
|-----|-------------|
| (a) RCU use-after-free | **FIXED** (kfree_rcu) |
| (b) Compat getdents return | **IGNORED** (readdir hooks external) |
| (c) Inode collision (no dev) | **FIXED** (ino+dev check) |
| (d) O(n) permission scan | **IGNORED** (still full scan, WORSE with kern_path in hot path) |
| (e) O(n) d_path scan | **PARTIALLY** (reverse table exists but not used for inode lookup) |
| (f) Static tables in header | **FIXED** (extern in .h, define in .c) |
| (g) Broad permission bypass | **IGNORED** |
| (h) No SELinux handling | **PARTIALLY** (xattr hook exists, context function is dead code) |

---

## Phase 2 Tasks (Pending — Next Session)

- [ ] Task #4: Analyze extended zm.c binary (11K vs 5K)
- [ ] Task #5: Analyze module scripts (metamount.sh, service.sh, post-fs-data.sh)
- [ ] Task #6: Analyze support scripts (logging, monitor, overlay, susfs_integration, sync, uninstall)
- [ ] Task #7: Synthesize final KEEP/DISCARD/FIX decision matrix

---

## Questions Still Unanswered

- [ ] What happens if the real file is deleted after a rule is added?
- [ ] Is there a race between rule addition and dcache invalidation?
- [ ] How does this affect inotify/fanotify watches on virtual paths?
- [ ] Does notify-module-mounted need to be called even when zero mounts happened?
- [ ] How does KernelSU handle metamodule failure during boot (timeout?)?
