# Learnings

## Learnings Log

> **Context:** These learnings were captured during analysis of NoMount (the predecessor).
> They directly inform the ZeroMount v2 design. "NoMount" references below are historical.

### 2026-01-28: Architecture Before Code

**What happened:**
Built NoMount v1 without reading metamodule documentation or understanding KernelSU boot sequence.

**What went wrong:**
service.sh grew to 1081 lines. Added a "universal mount hijacker" (400 lines) to detect and unmount mounts from other metamodules. Created a 4-phase enable/disable dance to work around timing issues. The overlay engine contradicted the project's purpose.

**Root cause:**
Didn't understand that metamount.sh runs at boot step 6 (the correct injection point). Put everything in service.sh (late boot), which runs after mounts already exist. This forced the entire hijacker pattern as a workaround.

**Lesson:**
Read the documentation first. Understand the system's design before modifying its behavior. The metamodule contract is simple: metamount.sh mounts modules, respect skip_mount/disable, call notify-module-mounted. Fighting this contract creates exponential complexity.

**Action taken:**
1. Completed /learn mode for kernel patch (15 concepts), metamodule architecture, and SUSFS integration
2. Documented everything in DOMAIN.md
3. Made architectural decisions (DECISIONS.md) before writing code
4. Starting fresh as ZeroMount with correct execution timing

---

### 2026-01-28: Static Definitions in Headers Create Hidden Copies

**What happened:**
Kernel patch header (nomount.h) defined hash tables and spinlock as `static`, meaning every .c file that includes it gets its own copy.

**What went wrong:**
Wasted ~12KB kernel memory per including translation unit (4 files include it = ~48KB wasted).

**Root cause:**
`static DEFINE_HASHTABLE(...)` in a header creates file-scope instances. Should be defined in the .c file and declared extern in the header.

**Lesson:**
In kernel C: definitions go in .c files, declarations go in .h files. `static` in a header = one copy per includer.

**Action taken:**
Added to kernel-patch-v2 bug fix list.

---

### 2026-01-28: SUSFS and ZeroMount Are Complementary, Not Competing

**What happened:**
Initially unclear how ZeroMount and SUSFS relate. They both modify kernel behavior.

**What surprised:**
They operate at different layers with minimal overlap — SUSFS's open_redirect provides per-inode file redirection (close + re-open), which is a simpler version of what ZeroMount does at the VFS namei layer. ZeroMount supersedes open_redirect for bulk path redirection, but open_redirect remains useful for specific single-file cases like fonts. Beyond that overlap, SUSFS hides evidence (kstat spoofing, path hiding, mount hiding, maps hiding) while ZeroMount redirects file paths. Together they create a complete illusion.

**Root cause:**
Didn't read SUSFS source before assuming overlap.

**Lesson:**
The plug-and-socket architecture is correct: ZeroMount orchestrates, SUSFS enforces. The kstat_redirect dual-inode enhancement bridges both systems by spoofing metadata for both virtual and real inodes.

**Action taken:**
Documented in DOMAIN.md. SUSFS integration module kept in feature backlog.

---

### 2026-01-28: free_page/\_\_putname Allocator Mismatch (Original Bug)

**What happened:**
Clone analysis (Phase 1, patch A3) revealed a `free_page()` call on memory allocated by `__getname()` (slab allocator). This is a type confusion bug — freeing slab memory as if it were a raw page.

**Impact:**
Corrupts page allocator free lists. Eventually causes kernel panic or silent memory corruption.

**Key insight:**
This bug exists in the ORIGINAL nomount-kernel-5.10.patch too, at `nomount.c:376` and `nomount.c:415`. It's not a clone regression — it was there from the start.

**Action taken:**
Added to kernel-patch-v2 bug fix list. Fix: replace `free_page((unsigned long)page_buf)` with `__putname(page_buf)`.

---

### 2026-01-28: Clone Expanded from 4 to 9 Hook Points — 67% Justified

**What happened:**
Original NoMount hooks 4 kernel files (namei, d_path, readdir, permissions). Clone hooks 9 files.

**Analysis:**
- 6 are legitimate detection vector closures: stat spoofing, statfs spoofing, xattr/SELinux, overlayfs compat, maps hiding
- 1 is mixed: maps hooks (legitimate hiding + lineage scope creep)
- 2 are pure scope creep: ROM fingerprinting (base.c), mount hiding (proc_namespace.c)

**Lesson:**
The original NoMount's 4 hooks leave significant detection gaps. A detection tool can use `stat()`, `statfs()`, `ls -Z` (SELinux), or `/proc/PID/maps` to reveal redirection. The clone correctly identified these gaps but also added unrelated ROM-hiding code. Scope discipline matters.

---

### 2026-01-28: Start-Disabled Pattern Prevents Boot Deadlock

**What happened:**
Original NoMount starts enabled (`ATOMIC_INIT(1)`). Clone starts disabled (`ATOMIC_INIT(0)`) with explicit ENABLE ioctl.

**Why this matters:**
During early boot, `kern_path()` calls inside NoMount hooks can deadlock because filesystems aren't fully mounted yet. By starting disabled, all hooks are no-ops until userspace explicitly enables them after boot reaches a stable state.

**Action taken:**
ZeroMount will use ATOMIC_INIT(0) + ENABLE/DISABLE ioctls.

---

### 2026-01-29: dev_t Encoding/Decoding in SUSFS kstat Spoofing

**What happened:**
Investigated "Inconsistent Mount" detection. Initially suspected kernel was double-decoding dev_t values. Removed decode calls in susfs.c. Build failed due to unrelated changes. Restored kernel code.

**What was actually happening:**
The kernel's decode logic **IS CORRECT**:
1. Userspace sends encoded dev_t via stat() result (e.g., 64775)
2. Kernel calls `huge_decode_dev(64775)` → `MKDEV(253,7)` = 265289735
3. Stores MKDEV format internally
4. When spoofing stat(), the kernel's stat path calls `new_encode_dev(265289735)` → 64775

**The encoding math:**
```c
// Encode: MKDEV(253,7) → 64775
new_encode_dev(MKDEV(253,7)) = (7 & 0xff) | (253 << 8) | ((7 & ~0xff) << 12) = 64775

// Decode: 64775 → MKDEV(253,7)
major = (64775 & 0xfff00) >> 8 = 253
minor = (64775 & 0xff) | ((64775 >> 12) & 0xfff00) = 7
MKDEV(253, 7) = 265289735
```

**Why my "fix" was WRONG:**
Removing the decode would store 64775 (encoded format) where kernel expects MKDEV format. Then `new_encode_dev(64775)` = 265289735 (huge number) would be returned to userspace.

**Lesson:**
Trace the FULL data path before assuming a bug. The encode/decode roundtrip is symmetric and correct. Don't remove "suspicious" code without understanding the complete flow.

---

### 2026-01-29: SUSFS Spoofing Only Applies to "Umounted" Processes

**What happened:**
Root shell (adb) always showed wrong dev_t (64815) even after kstat was registered successfully.

**Why:**
SUSFS spoof in `generic_fillattr()` has this condition:
```c
if (likely(susfs_is_current_proc_umounted()) &&
        unlikely(inode->i_mapping->flags & BIT_SUS_KSTAT)) {
```

`susfs_is_current_proc_umounted()` checks `TIF_PROC_UMOUNTED` flag on current thread. This flag is set by KernelSU/SUSFS during zygote spawn for non-root apps.

**Root shell is NOT umounted** → spoof doesn't apply → shows actual device (64815).
**Detection apps ARE umounted** → spoof applies → shows spoofed device (64775).

**Lesson:**
Cannot verify SUSFS spoofing from root shell. Must test with actual detection apps (Native Detector, Holmes, etc.). The "wrong" values in root shell are expected behavior, not a bug.

---

### 2026-01-29: Dual Inode Registration Handles ZeroMount + SUSFS Interaction

**What happened:**
Confused about which inode to register for kstat spoofing when ZeroMount redirects paths.

**The design:**
SUSFS `kstat_redirect` registers BOTH inodes:
1. `virtual_ino` — Original /vendor file's inode (resolved via `kern_path(virtual_pathname)`)
2. `target_ino` — Module file's inode (resolved via `kern_path(real_pathname)`)

```c
hash_add(SUS_KSTAT_HLIST, &new_entry->node, new_entry->target_ino);  // Real file
hash_add(SUS_KSTAT_HLIST, &virtual_entry->node, virtual_ino);  // Virtual file (if different)
```

**Why both:**
- Before ZeroMount: stat() resolves original /vendor inode
- After ZeroMount: stat() resolves module file inode
- By registering both, spoof works regardless of access path

**Lesson:**
The dual-inode design in SUSFS kstat_redirect was built specifically for mount-redirection scenarios. It's not a bug or redundancy — it's the correct solution for VFS redirection.

---

### 2026-01-29: Sucompat stat() Hook Missing Detection Bypass Check

**What happened:**
Disclosure app detected "KSU/AP detected (sucompat SCA)" — a side-channel attack detecting KernelSU's sucompat mechanism.

**Root cause investigation:**
1. Reverse engineered Duck Detector/Disclosure native libs
2. Found detection: `stat("/system/bin/su")` returns SUCCESS (redirected to /system/bin/sh) instead of ENOENT (stock behavior)
3. KernelSU sucompat has 3 hooks: `ksu_handle_stat()`, `ksu_handle_faccessat()`, `ksu_handle_execveat_sucompat()`
4. Upstream SUSFS commit d671f35 added `susfs_is_current_proc_umounted()` check to execveat and faccessat
5. **BUT `ksu_handle_stat()` was MISSING this check!**

**The behavioral difference:**
| Syscall | Stock Android | KernelSU (broken) | KernelSU (fixed) |
|---------|---------------|-------------------|------------------|
| `stat("/system/bin/su")` | ENOENT | SUCCESS | ENOENT (if umounted) |

**Fix implemented:**
Added to `ksu_handle_stat()`:
```c
#ifdef CONFIG_KSU_SUSFS
    if (likely(susfs_is_current_proc_umounted()) || !ksu_su_compat_enabled) {
        return 0;  // Skip transformation for detection apps
    }
#endif
```

**Implementation gotcha:**
First fix attempt failed because the build script checked if `susfs_is_current_proc_umounted` exists **anywhere** in sucompat.c. It does — in execveat/faccessat. Must check **within ksu_handle_stat function scope**.

**Lesson:**
When multiple functions need the same check, verify EACH function has it. "The file has the check" ≠ "all functions have the check". Also: grep for string existence is not function-scoped analysis.

**Files modified:**
- `/home/claudetest/gki-build/kernelsu-next-vanilla/.github/workflows/build.yml` — Added awk-based injection step
- Build run: 21467986635 (pending)
