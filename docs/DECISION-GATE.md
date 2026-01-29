# Decision Gate — ZeroMount v2

> **Instructions:** For each item, mark your decision in the DECISION column:
> - **ACCEPT** — Include in v2 implementation
> - **DISCARD** — Do not implement
> - **MODIFY** — Accept with changes (add note)
>
> Items marked with ⚠️ have a recommendation. Items marked 🔴 are critical.

---

## Group 1: Architecture Decisions (Previously Decided — Confirm or Revisit)

> **Status: ALL ACCEPTED** (2026-01-28)

| ID | Decision | Rationale | Rec. | DECISION |
|----|----------|-----------|------|----------|
| D1 | VFS path redirection instead of mounting | Zero evidence in /proc/mounts. Operates below mount layer. Requires custom kernel. | ACCEPT | **ACCEPT** |
| D2 | metamount.sh as primary injection point (not service.sh) | Correct boot timing at step 6. service.sh runs too late. | ACCEPT | **ACCEPT** |
| D3 | Discard universal mount hijacker (~400 lines) | ZeroMount IS the metamodule — no mounts to hijack. Unnecessary complexity. | ACCEPT | **ACCEPT** |
| D4 | Rename to ZeroMount (zm, /dev/zeromount, CONFIG_ZEROMOUNT, magic 'Z') | Clean identity separation from prototype. Consistent naming. | ACCEPT | **ACCEPT** |
| D5 | Shell injection scripts (sed/awk) vs single monolithic patch | Cross-kernel-version compatibility. Each hook is a separate maintainable script. | ACCEPT | **ACCEPT** |
| D6 | Start disabled + explicit ENABLE/DISABLE ioctls | Prevents early-boot deadlock when kern_path() called before filesystems mounted. | ACCEPT | **ACCEPT** |
| D7 | Add stat/statfs/xattr hooks (3 new hook points) | Closes 3 detection gaps SUSFS doesn't cover: statfs (filesystem type), xattr (SELinux context), stat (cooperative with SUSFS). Overlayfs re-spoofing is handled by SUSFS kstat_redirect — see I6 discard notes. | ACCEPT | **ACCEPT** |
| D8 | Discard procmounts + base-hide-stuff hooks | Zero mounts = nothing for procmounts to hide. base-hide-stuff is process-level fingerprinting (scope creep, SUSFS handles separately). | ACCEPT | **ACCEPT** |

### D7 Deliberation Notes (Source-Verified 2026-01-28, Updated after I6 analysis)

**Question raised:** Does SUSFS kstat_redirect + dual-inode already cover D7's detection vectors?

**Finding after source code review:**

| Detection Vector | SUSFS Covers? | ZeroMount Hook Needed? | Source Evidence |
|---|---|---|---|
| **stat()** metadata (ino, dev, size, times) | **YES** — `generic_fillattr()` in fs/stat.c, `susfs_sus_ino_for_generic_fillattr()` at susfs.c:788-812. Dual-inode at susfs.c:674-694 handles both paths. | Cooperative — ZeroMount checks `BIT_SUS_KSTAT` (bit 35) on inode, skips if SUSFS active. Fallback for non-SUSFS kernels. | Clone core patch: `nomount_syscall_spoof_stat()` explicitly checks bit 35 and returns early |
| **statfs()** filesystem type | **NO** — zero statfs hooks in SUSFS | **YES** — `inject-statfs-hooks.sh` hooks `user_statfs()` and `fd_statfs()` to spoof f_type/f_bsize | Without this: `statfs("/etc/fonts/x")` returns data partition magic instead of system partition magic |
| **xattr** SELinux labels | **NO** — zero xattr hooks in SUSFS | **YES** — `inject-xattr-hooks.sh` hooks `vfs_listxattr()` + `__vfs_getxattr()` to spoof security.selinux | Without this: `getxattr(..., "security.selinux")` returns `unlabeled` instead of `system_file` |
| **overlayfs** re-spoofing | **YES** — SUSFS `generic_fillattr()` hook fires after overlayfs operations | **NO** — I6 DISCARDED. Direct overlayfs hooking caused detection flood. SUSFS kstat_redirect handles this via its VFS-level hook. | Git commit `b376490`: "OverlayFS Hooking is BAD... caused MORE detections" |

**Conclusion:** D7 provides 3 hooks (stat, statfs, xattr). Overlayfs is covered by SUSFS kstat_redirect — our own overlayfs hook (I6) was abandoned after causing detection floods.

---

## Group 2: Kernel Bugs

> **Status: 16 ACCEPT, 1 MODIFY (K17)** (2026-01-28)

| ID | Bug | Current State (Source-Verified) | Proposed Fix | Severity | Rec. | DECISION |
|----|-----|-------------------------------|--------------|----------|------|----------|
| K1 | 🔴 RCU use-after-free in UID deletion | `kfree(entry)` after `hash_del_rcu()` at nomount.c:737 | Use `kfree_rcu(entry, rcu)` or `call_rcu()` | CRITICAL | ACCEPT | **ACCEPT** |
| K2 | Compat getdents return value | `buf.count` used instead of `count` at readdir.c:890 — injected entries not counted | Use `count` variable (matches getdents/getdents64 pattern) | MEDIUM | ACCEPT | **ACCEPT** |
| K3 | Inode collision — no dev check | `nomount_is_injected_file()` checks only `i_ino`, not `(ino, dev)` pair | Add `rule->real_dev == inode->i_sb->s_dev` check | MEDIUM | ACCEPT | **ACCEPT** |
| K4 | O(n) permission scan | `hash_for_each_rcu` (full table scan) in permission check | Replace with inode-keyed hash table for O(1) lookup | PERF | ACCEPT | **ACCEPT** |
| K5 | O(n) d_path scan | `hash_for_each_rcu` (full table scan) in d_path hook | Use reverse lookup table `nomount_targets_ht` | PERF | ACCEPT | **ACCEPT** |
| K6 | Static hash tables in header | `static DEFINE_HASHTABLE(...)` in nomount.h — one copy per includer (~24KB waste) | Move to .c file, use `extern` in header | WASTE | ACCEPT | **ACCEPT** |
| K7 | Overly broad permission bypass | Returns 0 for ANY permission mask (read/write/exec) on injected files | Restrict to MAY_READ + MAY_EXEC only (no write to injected files) | SECURITY | ACCEPT | **ACCEPT** |
| K8 | No SELinux context handling | xattr not spoofed — stat shows wrong security context | Add xattr hook patch (Decision #7). Wire nomount_get_spoofed_selinux_context() | DETECTION | ACCEPT | **ACCEPT** |
| K9 | EXPORT_SYMBOL on kill switch | `EXPORT_SYMBOL(nomount_enabled)` — any module can disable ZeroMount | Remove export. Control only via ioctl. | SECURITY | ACCEPT | **ACCEPT** |
| K10 | Rename all identifiers | nomount → zeromount, NOMOUNT → ZEROMOUNT, 'N' → 'Z', /dev/nomount → /dev/zeromount | Full rename pass across all kernel files | IDENTITY | ACCEPT | **ACCEPT** |
| K11 | Add ENABLE/DISABLE ioctls | No enable/disable commands exist in original (only 7 ioctls) | Add IOCTL_ENABLE (cmd 8) and IOCTL_DISABLE (cmd 9) | FEATURE | ACCEPT | **ACCEPT** |
| K12 | Start disabled at boot | Original: `ATOMIC_INIT(1)` — starts ENABLED, causes boot deadlock risk | Change to `ATOMIC_INIT(0)` — requires explicit ENABLE from metamount.sh | CRITICAL | ACCEPT | **ACCEPT** |
| K13 | ⚠️ Missing path_put() in flush_dcache | `kern_path()` succeeds but `path_put()` never called — memory leak per rule add | Add `path_put(&path)` after `d_invalidate()` | MEDIUM | ACCEPT | **ACCEPT** |
| K14 | ⚠️ free_page/__putname allocator mismatch | `free_page()` used on slab memory from `__getname()` at ~line 241 and ~299 | Replace `free_page()` with `__putname()` on error paths | CRITICAL | ACCEPT | **ACCEPT** |
| K15 | ⚠️ Only add_rule checks CAP_SYS_ADMIN | del_rule, clear, add_uid, del_uid, get_list have NO privilege check | Centralize CAP_SYS_ADMIN check in ioctl dispatch (clone approach) | SECURITY | ACCEPT | **ACCEPT** |
| K16 | ⚠️ NOMOUNT_MAGIC_POS lacks ULL suffix | `0x7000000` treated as int — potential sign extension on 64-bit | Change to `0x7000000ULL` (or clone's `0x7000000000000000ULL`) | LOW | ACCEPT | **ACCEPT** |
| K17 | ⚠️ /system prefix stripping is one-directional | Rule `/etc/foo` matches input `/system/etc/foo` but NOT reverse | Keep one-directional. Add /system prefix stripping to CE5's normalize_path at rule-add time. | LOW | MODIFY | **MODIFY** |

### K17 Deliberation Notes

**Question:** Should /system prefix matching be bidirectional?

**Decision:** No. Keep one-directional input stripping. Instead, extend CE5 (`normalize_path`) to strip `/system` prefix from virtual_path at ADD_RULE time. Since we control rule creation via metamount.sh, all rules will be canonical (no `/system` prefix). The one-directional input strip then catches both app access patterns (`/etc/foo` and `/system/etc/foo`). Bidirectional adds hot-path complexity for a case we can prevent at rule-add time.

---

## Group 3: Clone Enhancements to Merge

> **Status: ALL ACCEPTED** (2026-01-28)
>
> Note: CE2=K13, CE3=K16, CE5 extended by K17, CE8=K15, CE9=K5. Implementation unified.

| ID | Enhancement | What It Does | Source Evidence | Rec. | DECISION |
|----|-------------|-------------|-----------------|------|----------|
| CE1 | d_backing_inode() instead of d_inode | Correct inode access for overlayfs — d_inode returns overlay inode, d_backing_inode returns real | Clone uses throughout; original uses d_inode directly | ACCEPT | **ACCEPT** |
| CE2 | path_put() fix in flush_dcache | Fixes memory leak after kern_path() | Clone line 484 adds path_put | ACCEPT | **ACCEPT** |
| CE3 | MAGIC_POS as 64-bit ULL | Prevents sign extension / overflow on 64-bit systems | Clone: `0x7000000000000000ULL` vs original `0x7000000` | ACCEPT | **ACCEPT** |
| CE4 | Per-CPU recursion guard | Prevents infinite recursion when ZeroMount hooks trigger themselves | `DEFINE_PER_CPU(int, nomount_in_hook)` with ENTER/EXIT macros | ACCEPT | **ACCEPT** |
| CE5 | nomount_normalize_path() | Strips trailing slashes, handles edge cases. Extended per K17: also strips /system prefix at rule-add time. | Clone line 157 | ACCEPT | **ACCEPT** |
| CE6 | nomount_is_critical_process() | Skips hooks for init (pid 1) and kthreadd (pid 2) | Clone line 239 | ACCEPT | **ACCEPT** |
| CE7 | nomount_dev_open() root-only check | Adds explicit root check in device open (defense in depth beyond mode 0600) | Clone line 2774 | ACCEPT | **ACCEPT** |
| CE8 | Centralized CAP_SYS_ADMIN in ioctl dispatch | All ioctls (except GET_VERSION) require CAP_SYS_ADMIN | Clone line 2804 — replaces per-function checks | ACCEPT | **ACCEPT** |
| CE9 | Reverse lookup table (nomount_targets_ht) | O(1) lookup by real_path for d_path/stat operations | Clone line 109 + export at line 3147 | ACCEPT | **ACCEPT** |
| CE10 | Runtime debug toggle | /sys/kernel/zeromount/debug — 0=off, 1=standard, 2=verbose | Runtime control without reboot | ACCEPT | **ACCEPT** |
| CE11 | ZM_LOG/NM_DBG macro system | Rate-limited, level-gated kernel logging with lazy arg evaluation | Replaces ~130 lines of unconditional pr_info | ACCEPT | **ACCEPT** |

---

## Group 4: Supplementary Patches

> **Status: ALL ACCEPTED** (2026-01-28)

| ID | Patch | Description | Lines | Rec. | DECISION |
|----|-------|-------------|-------|------|----------|
| P1 | memory-safety | 4 null-deref guards (buffer overflow, vpath/dentry/sb checks) | ~50 | ACCEPT | **ACCEPT** |
| P2 | concurrency-barriers | ARM64 write reordering — smp_wmb() + WRITE_ONCE for rule fields | ~30 | ACCEPT | **ACCEPT** |
| P3 | logic-api-fix | free_page → __putname allocator mismatch fix (=K14) | ~20 | ACCEPT | **ACCEPT** |
| P4 | logging-infrastructure | Establish ZM_LOG macro system (=CE11). Original has only 1 pr_info (init message) — no cleanup needed. Clone's 130 scattered pr_info are clone-only; v2 builds from clean original and uses ZM_LOG from day one. | ~50 | ACCEPT | **ACCEPT** |
| P5 | fix-null-isb | 4 additional null i_sb checks in is_injected, spoof_stat, spoof_statfs, selinux | ~40 | ACCEPT | **ACCEPT** |

### P4 Clarification (Verified 2026-01-28)

Original patch has exactly **1 log line**: `pr_info("NoMount: Loaded\n")` at nomount.c:776. Zero hot-path logging. The ~130 unconditional `pr_info` are a **clone-only problem** (the clone developer added debug prints everywhere). Since v2 builds from the original, P4 becomes: "establish ZM_LOG macro infrastructure and use it for any new logging." Preventive, not corrective.

---

## Group 5: Injection Scripts

> **Status: 6 ACCEPT, 3 DISCARD** (2026-01-28) — Evidence-based revision after git history review

| ID | Script | Target File | What It Hooks | Evidence | Rec. | DECISION |
|----|--------|-------------|---------------|----------|------|----------|
| I1 | inject-namei-hooks.sh | fs/namei.c | getname_flags + readlink | In build, working. Minor compile error fix needed. | ACCEPT | **ACCEPT** |
| I2 | inject-readdir-hooks.sh | fs/readdir.c | getdents + compat_getdents | In build, working. | ACCEPT | **ACCEPT** |
| I3 | inject-stat-hooks.sh | fs/stat.c | stat/lstat/fstat metadata spoofing | **MODIFIED** — VFS-level hooks (generic_fillattr, vfs_getattr_nosec) removed per commits bff3159 + 1ecb9c5. Caused "detection flood" and "SUSFS conflicts / Gboard crashes". Use syscall-level hooks only. | ACCEPT | **ACCEPT** (syscall-level only) |
| I4 | inject-statfs-hooks.sh | fs/statfs.c | statfs filesystem type spoofing | In build, working. SUSFS doesn't cover statfs. | ACCEPT | **ACCEPT** |
| I5 | inject-xattr-hooks.sh | fs/xattr.c | SELinux context spoofing via xattr | In build, working. SUSFS doesn't cover xattr. | ACCEPT | **ACCEPT** |
| I6 | inject-overlayfs-hooks.sh | fs/overlayfs/inode.c | Re-spoof after overlayfs operations | **ABANDONED** — HANDOFF doc: "caused MORE detections", "Android uses OverlayFS extensively", "must be ABANDONED". Not in build workflow. | DISCARD | **DISCARD** |
| I7 | inject-maps-hooks.sh | fs/proc/task_mmu.c | Hide module paths from /proc/pid/maps | In build. Keep core hiding, discard lineage hacks if any. | ACCEPT | **ACCEPT** |
| I8 | inject-base-hide-stuff.sh | fs/proc/base.c | ROM fingerprinting — hides /proc entries | Not in build. Scope creep per D8. | DISCARD | **DISCARD** |
| I9 | inject-procmounts-hooks.sh | fs/proc_namespace.c | Mount hiding in /proc/mounts | In build BUT dead code. Zero mounts = nothing to hide. Align with D8. | DISCARD | **DISCARD** |

### I3 + I6 Deliberation Notes (Git History Evidence 2026-01-28)

**I3 (stat hooks):**
- Commit `bff3159`: "revert(hooks): Remove post-getattr hook - caused detection flood"
- Commit `1ecb9c5`: "Remove dead stat hooks that conflict with SUSFS... causing Gboard font crashes"
- Resolution: VFS-level hooks removed, syscall-level hooks (`newfstatat`, `fstatat64`) retained with SUSFS cooperation check (BIT_SUS_KSTAT skip)

**I6 (overlayfs hooks):**
- Commit `b376490` (HANDOFF_DEBUG_SESSION.md): "CRITICAL LESSON: OverlayFS Hooking is BAD"
- Quote: "After flashing kernel with OverlayFS hooks: MORE detections appeared, not fewer. Android system uses OverlayFS extensively."
- Resolution: Script abandoned, removed from build workflow

---

## Group 6: Binary Bugs

> **Status: 13 ACCEPT, 1 DISCARD** (2026-01-28)

| ID | Bug | Current State (Source-Verified) | Proposed Fix | Severity | Rec. | DECISION |
|----|-----|-------------------------------|--------------|----------|------|----------|
| B1 | 🔴 FD leak — never closes device | `open()` at nm.c:146, no `close()` before exit | Add `sys1(SYS_CLOSE, fd)` before exit | CRITICAL | ACCEPT | **ACCEPT** |
| B2 | Stack buffer overflow in path resolution | No bounds check on cwd+path at nm.c:177-186 | Add length check: `if (l + 1 + src_len >= PATH_MAX)` | HIGH | ACCEPT | **ACCEPT** |
| B3 | ~~STAT_MODE_IDX wrong~~ | **VERIFIED CORRECT**: idx 4 = byte offset 16 = st_mode on aarch64 | ~~No fix needed~~ — Remove from bug list | NOT A BUG | DISCARD | **DISCARD** |
| B4 | UID parsing overflow | No overflow/digit validation at nm.c:213-214 | Add digit validation and overflow check | MEDIUM | ACCEPT | **ACCEPT** |
| B5 | Version display single digit | `res + '0'` at nm.c:232 — version ≥ 10 produces garbage | Use itoa loop for multi-digit | LOW | ACCEPT | **ACCEPT** |
| B6 | No error output | Zero writes to stderr in entire binary | Add error messages to fd 2 for failures | LOW | ACCEPT | **ACCEPT** |
| B7 | Silent failure on bad argc | `goto do_exit` with no message at nm.c:161 | Print usage on bad argc | LOW | ACCEPT | **ACCEPT** |
| B8 | Unknown command exits 0 | Unrecognized command falls through to `exit_code = 0` | Set exit_code = 1 for unknown commands, print error | LOW | ACCEPT | **ACCEPT** |
| B9 | Rename device path | `/dev/nomount` at nm.c:146 | Change to `/dev/zeromount` | IDENTITY | ACCEPT | **ACCEPT** |
| B10 | Recalculate ioctl codes | Magic `0x4E` ('N') at nm.c:122-128 | Change to `0x5A` ('Z') | IDENTITY | ACCEPT | **ACCEPT** |
| B11 | Add enable/disable commands | No enable/disable in original | Add `enable` and `disable` commands mapping to new ioctls | FEATURE | ACCEPT | **ACCEPT** |
| B12 | Keep blk/unb commands | Clone dropped UID commands but UID exclusion is core feature | Keep blk/unb (or rename to adduid/deluid) | FEATURE | ACCEPT | **ACCEPT** |
| B13 | Add status + resolve commands | `status` reads enable state; `resolve` tests path resolution | Include in v2 — useful for debugging | DEBUG | ACCEPT | **ACCEPT** |
| B14 | Usage string omits 'ver' command | nm.c:142 lists 6 commands, omits `ver` | Add `ver` to usage string | LOW | ACCEPT | **ACCEPT** |

---

## Group 7: Script Issues

> **Status: 10 ACCEPT, 1 DISCARD** (2026-01-28)

| ID | Issue | Current State (Source-Verified) | Proposed Fix | Severity | Rec. | DECISION |
|----|-------|-------------------------------|--------------|----------|------|----------|
| S1 | 🔴 Missing notify-module-mounted | Zero matches in ALL scripts — KernelSU may hang at boot | Add `/data/adb/ksud kernel notify-module-mounted` at end of metamount.sh | CRITICAL | ACCEPT | **ACCEPT** |
| S2 | 🔴 Missing skip_mount flag check | metamount.sh:35 checks disable/remove but NOT skip_mount | Add `[ -f "$mod_path/skip_mount" ] && continue` | CRITICAL | ACCEPT | **ACCEPT** |
| S3 | Missing metauninstall.sh | File does not exist in either original or clone | Create cleanup script for module removal | MEDIUM | ACCEPT | **ACCEPT** |
| S4 | Empty versionCode in module.prop | module.prop:5 has `versionCode=` (empty) | Set to integer (e.g., `versionCode=1`) | LOW | ACCEPT | **ACCEPT** |
| S5 | Non-standard 'remove' flag check | metamount.sh:35 checks both `disable` and `remove` | Keep — `remove` is a valid KernelSU flag for pending removal | LOW | ACCEPT | **ACCEPT** |
| S6 | No APEX partition in TARGET_PARTITIONS | metamount.sh:7 lists system/vendor/product/system_ext/odm/oem — no apex | APEX files are sealed containers, not modified by KernelSU modules | LOW | DISCARD | **DISCARD** |
| S7 | Rename all script identifiers | All scripts use nomount/nm naming | Change to zeromount/zm throughout | IDENTITY | ACCEPT | **ACCEPT** |
| S8 | monitor.sh race condition | No sleep before sed — KernelSU may read module.prop before sed completes | Add `sleep 1` before sed write | LOW | ACCEPT | **ACCEPT** |
| S9 | service.sh no error checking | `nm block "$uid"` with no return code check (6-line script) | Add error check and logging | LOW | ACCEPT | **ACCEPT** |
| S10 | No CLEAR_ALL at start of metamount.sh | Rules from previous boot persist if script re-runs (debug, module update) | Add `zm clear` as first command in metamount.sh — defensive, costs nothing | MEDIUM | ACCEPT | **ACCEPT** |
| S11 | Missing metamodule=1 in module.prop | module.prop lacks metamodule identifier — KernelSU won't recognize as metamodule | Add `metamodule=1` line per KernelSU metamodule spec (METAMODULE_COMPLETE_GUIDE line 157-169) | CRITICAL | ACCEPT | **ACCEPT** |

---

## Group 8: SUSFS Integration

> **Status: 11 ACCEPT, 10 DISCARD** (2026-01-28)

### KEEP (include in v2)

| ID | Component | Purpose | Rec. | DECISION |
|----|-----------|---------|------|----------|
| SU1 | susfs_init() | Detect SUSFS binary + capabilities at boot | ACCEPT | **ACCEPT** |
| SU2 | susfs_apply_kstat() with fallback | kstat_redirect → kstat_statically cascade | ACCEPT | **ACCEPT** |
| SU3 | susfs_apply_path() | Hide paths from readdir and stat | ACCEPT | **ACCEPT** |
| SU4 | susfs_apply_maps() | Hide .so from /proc/pid/maps | ACCEPT | **ACCEPT** |
| SU6 | Path classification logic | Categorize files by type for correct SUSFS operation | ACCEPT | **ACCEPT** |
| SU7 | kstat_redirect dual-inode enhancement | Spoof metadata for BOTH virtual and real inodes | ACCEPT | **ACCEPT** |
| SU8 | inject-susfs-custom-handlers.sh | Build automation for SUSFS kernel integration | ACCEPT | **ACCEPT** |
| SU9 | ZM_LOG/NM_DBG logging macros | Runtime debug toggle for kernel logging | ACCEPT | **ACCEPT** |

### DISCARD (remove from v2)

| ID | Component | Why Discard | Rec. | DECISION |
|----|-----------|-------------|------|----------|
| SU5 | apply_font_redirect() | ZeroMount metamodule handles all path redirection including fonts — SUSFS only needs kstat | DISCARD | **DISCARD** |
| SU10 | Overlay engine (~400 lines) | ZeroMount creates zero overlays | DISCARD | **DISCARD** |
| SU11 | Mount hiding engine (~200 lines) | ZeroMount creates zero mounts | DISCARD | **DISCARD** |
| SU12 | Bind mount detection | No bind mounts to detect | DISCARD | **DISCARD** |
| SU13 | Process ancestry check | Unrelated to VFS redirection | DISCARD | **DISCARD** |
| SU14 | /proc/mounts filtering | Nothing to filter — zero mounts | DISCARD | **DISCARD** |
| SU15 | overlay-status.sh script | No overlays to monitor | DISCARD | **DISCARD** |
| SU16 | mount-status.sh script | No mounts to monitor | DISCARD | **DISCARD** |
| SU17 | process-status.sh script | Unrelated scope creep | DISCARD | **DISCARD** |
| SU18 | Duplicate logging system | Replaced by ZM_LOG macros | DISCARD | **DISCARD** |

### SIMPLIFY (reduce scope)

| ID | Component | Current → Target | Rec. | DECISION |
|----|-----------|-----------------|------|----------|
| SU19 | susfs_integration.sh | 1335 lines → ~200 lines (init + classify + apply per-file) | ACCEPT | **ACCEPT** |
| SU20 | Monitoring | Full service → simple watchdog | ACCEPT | **ACCEPT** |
| SU21 | Error handling | Complex retry logic → simple log + continue | ACCEPT | **ACCEPT** |

---

## Group 9: Custom SUSFS Modifications (Your Fork Commits)

> These are modifications YOU made to the upstream SUSFS codebase. Each needs a decision for the new ZeroMount architecture.

### New APIs

| ID | Modification | Commit | What It Does | ZeroMount Relevance | Rec. | DECISION |
|----|-------------|--------|-------------|---------------------|------|----------|
| SF1 | `susfs_add_sus_kstat_redirect()` | 26485e7 | Register virtual→real path kstat spoofing with all stat fields | Core integration: apps stat() redirected files and get original metadata | ACCEPT | |
| SF2 | Dual-inode registration | 398e35c | Resolve BOTH virtual and real inodes, create 2 hash entries | Prevents detection regardless of which inode path the app takes | ACCEPT | |
| SF3 | `susfs_add_open_redirect_all()` | f49f3c7 | File redirect for ALL UIDs (not just UID < 2000) | Font/audio modules need user apps (UID ≥ 10000) to see redirected files | ACCEPT | |
| SF4 | `susfs_get_redirected_path_all()` | f49f3c7 | Lookup redirect in OPEN_REDIRECT_ALL_HLIST | Runtime lookup for the above feature | ACCEPT | |
| SF5 | `susfs_update_open_redirect_all_inode()` | f49f3c7 | Set AS_FLAGS_OPEN_REDIRECT_ALL bit on target inode | Marks inodes for the do_filp_open() hook check | ACCEPT | |

### Unicode Filter

| ID | Modification | Commit | What It Does | ZeroMount Relevance | Rec. | DECISION |
|----|-------------|--------|-------------|---------------------|------|----------|
| SF6 | `susfs_check_unicode_bypass()` function | 62db124 | Blocks Unicode attacks (RTL/LTR override, zero-width chars, Cyrillic, diacritical, non-ASCII) on Android/data and Android/obb paths | Security feature — prevents invisible filename manipulation. Independent of ZeroMount. | DISCUSS | |
| SF7 | Unicode filter hooks script (51_add_unicode_filter.sh) | 295b83f | Injects hooks into 9 syscalls (mkdirat, unlinkat, symlinkat, linkat, renameat2, faccessat, openat2, statx, readlinkat) | Build-time injection via sed (cross-kernel compatible) | DISCUSS | |
| SF8 | CONFIG_KSU_SUSFS_UNICODE_FILTER Kconfig | 62db124 | Compile-time toggle for Unicode filter (default y) | Feature gate — can disable if causing false positives | ACCEPT | |
| SF9 | Boot safety guard (`susfs_unicode_filter_ready`) | 62db124 | Returns false until susfs_init() completes — prevents early-boot panics | Critical: filter must not run before init | ACCEPT | |
| SF10 | Full filter vs Bidi-only decision | 592e8f5 vs e781be0 | Full filter has 5 features (Bidi, zero-width, Cyrillic, diacritical, catch-all). Bidi-only strips 3 features to avoid false positives. Current branch: FULL. | Which level? Full = max security, Bidi-only = safer for international users | DISCUSS | |

### Security Hardening (Upstream Bug Fixes)

| ID | Modification | Commit | What It Does | Impact | Rec. | DECISION |
|----|-------------|--------|-------------|--------|------|----------|
| SF11 | strncpy null-termination (16+ instances) | f49f3c7 | Added missing null terminators after every strncpy() in SUSFS | Prevents buffer over-reads from unterminated strings | ACCEPT | |
| SF12 | NULL deref after kzalloc failure (2 instances) | f49f3c7 | Fixed cmdline + enabled_features functions that crashed on OOM | Prevents kernel panic on low memory | ACCEPT | |
| SF13 | Hash table race conditions (3 functions) | 62db124 | Added spin_lock_irqsave() around hash lookups in fillattr, show_map_vma, get_redirected_path | Prevents data corruption on SMP systems | ACCEPT | |
| SF14 | Atomic dual-inode registration | 62db124 | Both hash entries added under single spinlock (was separate locks) | Prevents partial registration on concurrent access | ACCEPT | |
| SF15 | Atomic hash update in update_sus_kstat | 62db124 | hash_del + hash_add under single lock | Prevents window where entry is missing | ACCEPT | |
| SF16 | Log leakage fix | 62db124 | pr_info() → SUSFS_LOGI() respecting enable_log | Prevents SUSFS operations from appearing in dmesg when logging disabled | ACCEPT | |

### Build Infrastructure

| ID | Modification | Commit | What It Does | ZeroMount Relevance | Rec. | DECISION |
|----|-------------|--------|-------------|---------------------|------|----------|
| SF17 | inject-susfs-custom-handlers.sh | d84d6cc | Injects supercall handlers, Kconfig entries, defconfig for custom functions | Essential for CI/CD — replaces inline sed in build workflow | ACCEPT | |

### VFS Hook Modifications

| ID | Modification | Commit | What It Does | ZeroMount Relevance | Rec. | DECISION |
|----|-------------|--------|-------------|---------------------|------|----------|
| SF18 | do_filp_open() dual check (ALL before regular) | f49f3c7 | Priority: BIT_OPEN_REDIRECT_ALL checked first, then BIT_OPEN_REDIRECT (UID < 2000) | Ensures user-app redirects take priority | ACCEPT | |

### Reverted/Abandoned from wip Branch

| ID | Feature | Why Abandoned | Include in v2? | Rec. | DECISION |
|----|---------|--------------|----------------|------|----------|
| SF19 | SUS_PROC (process hiding) | Removed in d87b820 — too invasive, hiding processes is beyond scope | No | DISCARD | |
| SF20 | AVC spoofing (defer to extras.c) | Experimental, specific to ReSukiSU fork | No | DISCARD | |
| SF21 | susfs_mnt_id_backup namespace fix | May still be relevant for mount ID consistency | Check if needed | DISCUSS | |
| SF22 | 64-bit newfstatat SYSCALL_DEFINE4 hook | Added stat64 hook for 64-bit architectures | Needed for aarch64 stat spoofing | ACCEPT | |

---

## Summary

| Group | Items | Accept | Discard | Modify | Pending |
|-------|-------|--------|---------|--------|---------|
| 1. Architecture | 8 | 8 | 0 | 0 | 0 |
| 2. Kernel Bugs | 17 | 16 | 0 | 1 | 0 |
| 3. Clone Enhancements | 11 | 11 | 0 | 0 | 0 |
| 4. Supplementary Patches | 5 | 5 | 0 | 0 | 0 |
| 5. Injection Scripts | 9 | 6 | 3 | 0 | 0 |
| 6. Binary Bugs | 14 | 13 | 1 | 0 | 0 |
| 7. Script Issues | 11 | 10 | 1 | 0 | 0 |
| 8. SUSFS Integration | 21 | 11 | 10 | 0 | 0 |
| **TOTAL** | **96** | **80** | **15** | **1** | **0** |

*Note: Group 9 (Custom SUSFS Mods, 22 items) excluded from totals — decisions pending.*

> When complete, this document becomes the **authoritative implementation spec** for ZeroMount v2.
