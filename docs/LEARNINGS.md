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

---

### 2026-01-29: ZeroMount Kernel Artifacts Expose Presence

**What happened:**
Detection app (crackme) still detected root even with sucompat fix applied and no ZeroMount rules configured.

**Root cause:**
ZeroMount kernel module creates visible artifacts:
- `/dev/zeromount` — char device (mode 0600, but existence detectable via stat())
- `/sys/kernel/zeromount/` — sysfs kobject (mode 0755, world-readable directory)

Detection apps don't need to READ these paths — just `stat()` returning SUCCESS proves ZeroMount is loaded.

**Detection vector:**
```c
stat("/dev/zeromount")      // Returns 0 → ZeroMount kernel present
stat("/sys/kernel/zeromount") // Returns 0 → ZeroMount active
```

**Why this wasn't obvious:**
1. Focused on VFS hook behavior, not kernel module registration side-effects
2. Testing from root shell — SUSFS hiding doesn't apply to root
3. Assumed "no rules = invisible" — wrong, presence itself is detectable

**Fix:**
Add SUSFS sus_path_loop in service.sh to hide both paths:
```bash
ksu_susfs add_sus_path_loop /dev/zeromount
ksu_susfs add_sus_path_loop /sys/kernel/zeromount
```

**Why sus_path_loop (not sus_path):**
`sus_path_loop` re-flags paths as hidden for each zygote-spawned process. Regular `sus_path` only applies once. Apps spawned after boot need the loop variant.

**Verification (dmesg):**
```
susfs:[0][13709][susfs_run_sus_path_loop] re-flag '/dev/zeromount' as SUS_PATH for uid: 99039
susfs:[0][13709][susfs_run_sus_path_loop] re-flag '/sys/kernel/zeromount' as SUS_PATH for uid: 99039
```

**Lesson:**
Kernel modules have side-effects beyond their intended functionality. Device nodes, sysfs entries, procfs entries, and even kernel symbol exports can reveal presence. Always audit what artifacts a kernel module creates and hide them from detection apps.

**Files modified:**
- `module/service.sh` — Added SUSFS sus_path_loop calls
- `releases/zeromount-v1.0.1.zip` — Updated module package

---

### 2026-01-29: WebUI Competition Pattern — Parallel Agent Design Contest

**What happened:**
Needed to design a world-class WebUI but had multiple valid approaches. Instead of picking one upfront, ran a "World Cup" competition between 3 architect agents.

**The approach:**
1. **Proposal phase**: 3 agents wrote design proposals with full creative freedom
   - Alpha: Minimalist ("nothing left to take away")
   - Beta: Bold Expressionist ("make users FEEL something")
   - Gamma: Futurist ("glassmorphism, aurora gradients")

2. **Build phase**: All 3 built complete implementations simultaneously
   - Same tech stack (Solid.js + TypeScript)
   - Same backend integration (zm commands)
   - Different visual designs

3. **Judgement phase**: User reviewed all 3 via Playwright, crowned a winner

**Results:**
- Alpha: Clean but too minimal for a power tool
- **Beta: WINNER** — "truly impressed beyond imagination"
- Gamma: Fascinating features to cherry-pick

**Why this worked:**
- Avoided analysis paralysis on design direction
- Got 3 production-quality implementations to compare
- User could make informed choice based on actual code
- Best features can be merged (Beta base + Gamma glassmorphism)

**Lesson:**
For subjective decisions (UI design, architecture), competition beats committee. Build multiple approaches in parallel, let the work speak for itself. The winning design emerged from actual implementations, not theoretical debates.

**Pattern for future:**
```
1. Define constraints (tech stack, features, timeline)
2. Deploy N agents with different philosophies
3. Let them build independently
4. Judge based on results, not proposals
5. Cherry-pick best features across implementations
```

**Files created:**
- `proposals/proposal-alpha.md`, `proposal-beta.md`, `proposal-gamma.md`
- `webui-v2-alpha/`, `webui-v2-beta/`, `webui-v2-gamma/`

---

### 2026-01-30: KSU WebUI Exec API Uses Callback Pattern, Not Promises

**What happened:**
ConfigTab worked perfectly in browser with mock data but completely failed on real device. All shell commands silently returned nothing.

**Root cause:**
KernelSU WebUI's `ksu.exec()` uses a **callback pattern**, not direct promises:

```javascript
// WRONG - what I wrote
const result = await ksu.exec(cmd);

// CORRECT - callback pattern
const callbackName = `exec_cb_${Date.now()}`;
window[callbackName] = (errno, stdout, stderr) => {
    delete window[callbackName];
    resolve({ errno, stdout, stderr });
};
ksu.exec(cmd, '{}', callbackName);
```

**Why this wasn't caught:**
Mock mode bypasses `ksu.exec()` entirely. Playwright tests pass 100% but prove nothing about real device behavior.

**Lesson:**
When integrating with platform APIs (KSU, Android PM, etc.), **read the actual API implementation** from a working reference project. Don't assume standard patterns. The TrickyAddon reference had the correct pattern all along.

**Action taken:**
Rewrote `execCommand()` in api.ts to use proper callback pattern. All shell commands now work.

---

### 2026-01-30: Modern Android Apps Use Adaptive Icons (aapt Extraction Fails)

**What happened:**
Tried to extract app icons using `aapt dump badging` + `unzip`. Got XML files instead of PNGs for 60%+ of apps.

**Root cause:**
Modern Android apps (8.0+) use **adaptive icons** - XML files that reference vector drawables, not PNG bitmaps:
```
application: label='Netflix' icon='res/ex.xml'
```

The XML contains adaptive-icon definitions, not actual image data.

**Solution:**
Use KernelSU's native `getPackagesIcons()` API:
```javascript
const ksu = globalThis.ksu;  // NOT window.ksu
const result = ksu.getPackagesIcons(JSON.stringify([packageName]), 100);
const parsed = JSON.parse(result);
imgEl.src = parsed[0].icon;  // base64 PNG, rendered by Android
```

This uses Android's native icon renderer which properly handles adaptive icons, vector drawables, WebP, and all formats.

**Lesson:**
Platform APIs exist for a reason. Don't reinvent icon extraction when the OS already has a proper renderer. aapt is for metadata (app names), not for icon extraction in modern Android.

---

### 2026-01-30: Shell Pipe + While Loop = Subshell Variable Loss

**What happened:**
App list generation script failed silently. Variables set inside `while read` loop were empty after the loop.

**Root cause:**
In bash, piping to `while read` runs the loop in a **subshell**:
```bash
# Variables set here are LOST after loop ends
cat file | while read line; do
    count=$((count + 1))  # This increments a SUBSHELL copy
done
echo $count  # Always 0 or original value
```

**Solutions:**
1. Write to temp file inside loop: `echo "$data" >> "$TMP_FILE"`
2. Use process substitution: `while read line; do ...; done < <(cat file)`
3. Use for loop with IFS manipulation

**Action taken:**
`refresh_apps.sh` writes each entry to a temp file, then assembles JSON after the loop completes.

---

### 2026-01-30: Quick Iteration Workflow — Push Don't ZIP

**What happened:**
Wasted significant time rebuilding ZIP files for every small change during debugging.

**Better workflow:**
```bash
# For WebUI changes
pnpm build
adb push webroot-beta/index.html /data/local/tmp/
adb push webroot-beta/assets/. /data/local/tmp/assets/
adb shell "su -c 'cp -r /data/local/tmp/* /data/adb/modules/zeromount/webroot/'"

# For shell script changes
adb push module/service.sh /data/local/tmp/
adb shell "su -c 'cp /data/local/tmp/service.sh /data/adb/modules/zeromount/'"
```

**Lesson:**
Only build ZIP when user explicitly says "zip it". Direct file push enables rapid iteration.

---

### 2026-01-30: Reference Project Semantic Understanding Required

**What happened:**
Blindly followed System App Nuker's approach without realizing it scans SYSTEM apps while ZeroMount needs USER apps.

**The difference:**
- System App Nuker: `find /system/app /system/priv-app` → System bloatware
- ZeroMount ConfigTab: `pm list packages -3 -U` → Third-party user apps

**Lesson:**
When given a reference project:
1. Deploy multiple agents to deeply analyze it
2. Understand the SEMANTIC PURPOSE, not just the code patterns
3. Ask: "Does this reference solve MY problem, or a different one?"

Copying code without understanding context leads to fundamental mismatches.

---

### 2026-01-30: globalThis vs window for KSU API Access

**What happened:**
Icon loading failed silently when using `window.ksu`.

**Why:**
KernelSU WebUI injects the `ksu` object into `globalThis`, not `window`. In some JavaScript contexts these differ.

**Correct pattern:**
```javascript
const ksu = (globalThis as any).ksu;
if (typeof ksu?.getPackagesIcons === 'function') {
    // Use API
}
```

**Lesson:**
Check reference implementations for exact global access patterns. `globalThis` is the modern standard for accessing globals across all JavaScript environments.

---

### 2026-01-30: Complete ZeroMount + SUSFS Architecture (4-Agent Deep Dive)

**What happened:**
Deployed 4 specialized agents to analyze the entire ZeroMount kernel patch and SUSFS source code after confusion about why VFS redirection tests showed both UIDs seeing the same file.

**Key Architecture Understanding:**

```
┌─────────────────────────────────────────────────────────────┐
│                        ZeroMount                             │
│  (VFS Path Redirection - custom kernel patch)               │
│                                                              │
│  Hook: getname_flags() in fs/namei.c                        │
│  Purpose: When app opens /system/foo, serve /data/adb/.../foo│
│  Storage: Kernel hash tables (NOT persistent)               │
│  Commands: zm add/del/blk/unb/enable/disable                │
│  Engine: Starts DISABLED, must call `zm enable`             │
└─────────────────────────────────────────────────────────────┘
                              +
┌─────────────────────────────────────────────────────────────┐
│                         SUSFS                                │
│  (Path HIDING - makes files invisible, NOT different)        │
│                                                              │
│  add_sus_path: Hide path (returns ENOENT to non-root)       │
│  add_sus_kstat: Spoof file metadata (inode, device)         │
│  add_sus_mount: Hide mount points from /proc/mounts         │
│  add_sus_map: Hide from /proc/self/maps                     │
│  DOES NOT provide content redirection                       │
└─────────────────────────────────────────────────────────────┘
```

**Critical UID Semantics (INVERTED from intuition):**

| Action | What it means |
|--------|---------------|
| `zm blk <uid>` | UID sees **REAL** files (bypasses redirection) |
| No `zm blk` | UID sees **MODULE** files (redirected) |
| Root (uid 0) | Always sees **REAL** files (by design) |

**Why root sees real files:**
This is intentional for security research. Root processes need to access actual files for debugging, while detection apps (regular UIDs) see spoofed content.

**Boot Sequence:**
1. `metamount.sh` runs (KSU metamodule hook)
2. Clears old rules: `zm clear`
3. Scans `/data/adb/modules/*/system|vendor|product/*`
4. For each file: `zm add /<virtual_path> <real_module_path>`
5. Enables engine: `zm enable` ← CRITICAL
6. `service.sh` runs later:
   - Hides /dev/zeromount via SUSFS
   - Applies UID exclusions from .exclusion_list

**Rule Storage:**
- Rules exist ONLY in kernel memory
- NOT persistent across reboots
- `metamount.sh` re-injects rules every boot
- Path normalization: `/system/foo` stored as `/foo`

**File Access Flow:**
```
App opens /system/framework/services.jar
    │
    ▼
getname_flags() called
    │
    ▼
zeromount_getname_hook() intercepts
    │
    ├── Check: ZEROMOUNT_DISABLED()? → skip if disabled
    ├── Check: zeromount_is_critical_process()? → skip for init/kthreadd
    ├── Check: zeromount_is_uid_blocked(current_uid())? → skip if blocked
    ├── Check: path starts with '/'? → skip relative paths
    │
    ▼
zeromount_resolve_path() looks up rule
    │
    ├── Hash lookup in zeromount_rules_ht
    ├── Match: /system/framework/services.jar OR /framework/services.jar
    │
    ▼
If rule found AND UID not blocked:
    - Free original filename struct
    - Return new filename pointing to module file
    - App reads module content without knowing
```

**SUSFS vs ZeroMount - Critical Distinction:**

| Aspect | SUSFS sus_path | ZeroMount |
|--------|----------------|-----------|
| What it does | Makes path invisible | Returns different content |
| Result | ENOENT (not found) | File opens with module data |
| Use case | Hide /dev/zeromount | Overlay system files |
| Process filter | Non-root only | Configurable per-UID |

**Why SUSFS hiding works from root shell:**
It doesn't! `susfs_is_current_proc_umounted()` returns false for root shell. SUSFS spoofing only applies to zygote-spawned app processes with `TIF_PROC_UMOUNTED` flag set.

**Lesson:**
ZeroMount and SUSFS are COMPLEMENTARY systems:
- ZeroMount: Changes WHAT apps see (content redirection)
- SUSFS: Changes WHETHER apps see (path/mount/stat hiding)
Together they create a complete illusion for detection apps while allowing root debugging.

**Files analyzed:**
- `/home/claudetest/zero-mount/nomount/patches/zeromount-kernel-5.10.patch`
- `/home/claudetest/zero-mount/nomount/patches/zeromount-core.patch`
- `/home/claudetest/gki-build/susfs4ksu-new/kernel_patches/fs/susfs.c`
- `/home/claudetest/zero-mount/nomount/src/zm.c`
- `/home/claudetest/zero-mount/nomount/module/metamount.sh`
- `/home/claudetest/zero-mount/nomount/module/service.sh`

---

### 2026-01-30: WebUI Performance Fixes (12-Fix Comprehensive Audit)

**What happened:**
User reported WebUI was extremely slow after reboot (35-55s) and icons weren't persisting. Deployed 5 specialized agents to audit the entire codebase.

**Root causes found:**

1. **Double aapt calls** - service.sh called aapt TWICE per app (name + icon separately)
2. **30-retry blocking loop** - api.ts blocked UI for up to 30 seconds waiting for JSON
3. **iconCache inside function** - Recreated on every re-render, losing cached icons
4. **Trigger comparison bug** - `newTrigger !== lastTriggerTimestamp` fails when both null
5. **Stale closure** - `existingPkgs` captured outside setInstalledApps updater

**Fixes implemented:**
- Single aapt call with cached output
- Return empty immediately, let polling handle updates
- Move iconCache to module scope
- Explicit null handling in trigger comparison
- Create existingPkgs inside updater function
- Initialize trigger file on daemon start
- 30s timeout on KSU exec callbacks
- Reduce polling from 10s to 5s

**Expected results:**
- Boot: 35-55s → 5-15s
- App detection: 15s+ → 5-10s
- Icons persist across re-renders

---

### 2026-01-30: Cross-Patch Kernel Integration via Exported Function

**What happened:**
User excluded detector apps via WebUI (`zm blk <uid>`), but detectors still couldn't detect root artifacts. Discovered that `zm blk` only affected ZeroMount VFS redirection - SUSFS continued hiding paths independently.

**Root cause:**
ZeroMount and SUSFS were operating as INDEPENDENT kernel systems with no communication:
- ZeroMount: Stored blocked UIDs in `zeromount_uid_ht` hash table
- SUSFS: Had its own hiding logic checking `susfs_is_current_proc_umounted()`
- No shared data or function calls between them

**Solution:**
Export ZeroMount's UID check function and have SUSFS call it:

```c
// ZeroMount (zeromount-core.patch) - EXPORT the function
bool zeromount_is_uid_blocked(uid_t uid) { ... }
EXPORT_SYMBOL(zeromount_is_uid_blocked);

// SUSFS (susfs_def.h) - CALL the exported function
#ifdef CONFIG_ZEROMOUNT
#include <linux/zeromount.h>
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) {
    return zeromount_is_uid_blocked(uid);
}
#else
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) { return false; }
#endif

// SUSFS hiding checks - ADD the exclusion check
static inline bool is_i_uid_not_allowed(uid_t i_uid) {
    if (susfs_is_uid_zeromount_excluded(current_uid().val))
        return false;  // Don't hide from excluded UIDs
    return (likely(susfs_is_current_proc_umounted()) &&
        unlikely(current_uid().val != i_uid));
}
```

**Key design decisions:**

1. **Single source of truth**: ZeroMount owns the exclusion list, SUSFS queries it
2. **CONFIG guard**: `#ifdef CONFIG_ZEROMOUNT` allows SUSFS to build without ZeroMount
3. **Stub fallback**: When ZeroMount not built, function returns `false` (no exclusions)
4. **Early return**: Exclusion check comes FIRST, before other hiding logic

**Files modified:**
- `zeromount-core.patch`: Remove `static`, add `EXPORT_SYMBOL`, header declaration, stub
- `susfs_def.h`: Add `susfs_is_uid_zeromount_excluded()` helper
- `susfs.c`: Modify `is_i_uid_not_allowed()` and `is_i_uid_in_android_data_not_allowed()`
- VFS patch: Modify `show_vfsmnt()`, `show_mountinfo()`, `show_vfsstat()` mount hiding

**Result:**
Now when user calls `zm blk <uid>`:
- ZeroMount: Bypasses VFS redirection for that UID
- SUSFS: Bypasses ALL hiding (paths, mounts, stats) for that UID
- Detector app sees EVERYTHING real

**Lesson:**
When kernel patches need to share state, export functions rather than duplicating data structures. The owning patch exports an accessor function, consuming patches call it with proper CONFIG guards. This maintains separation of concerns while enabling integration.
