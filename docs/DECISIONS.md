# Decisions Log

## Decisions

### Decision 8: Discard procmounts and base-hide-stuff Hooks

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Clone added 9 hook injection scripts vs original's 4. Two are scope creep.

**Decision:**
Discard `inject-procmounts-hooks.sh` (mount hiding — ZeroMount doesn't create mounts) and `inject-base-hide-stuff.sh` (ROM fingerprinting — unrelated to VFS redirection).

**Consequences:**
- We gain: Focused hook surface, fewer kernel touchpoints
- We lose: Nothing relevant to VFS redirection

---

### Decision 7: Add stat/statfs/xattr/overlayfs Hooks

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Original NoMount has 4 hook points. Clone analysis revealed 4 legitimate detection gaps: stat() leaks dev/ino, statfs() leaks filesystem type, xattr leaks SELinux context, overlayfs undoes stat spoofing.

**Decision:**
Add hooks for fs/stat.c, fs/statfs.c, fs/xattr.c, fs/overlayfs/inode.c. These close real detection vectors.

**Consequences:**
- We gain: Resistance to stat/statfs/SELinux/overlayfs detection
- We must now: Maintain 8 hook points instead of 4

---

### Decision 6: Start Disabled + ENABLE/DISABLE Ioctls

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Original starts enabled (ATOMIC_INIT(1)). This causes boot deadlock when kern_path() is called before filesystems are mounted.

**Decision:**
Start disabled (ATOMIC_INIT(0)). metamount.sh calls ENABLE ioctl after rules are loaded.

**Consequences:**
- We gain: No early-boot deadlock
- We must now: Always call ENABLE in metamount.sh after rule loading

---

### Decision 5: Use Shell Injection Scripts Instead of Single Patch

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Original is a single .patch file. Clone uses shell scripts (inject-*.sh) that dynamically inject hooks using sed/awk. This makes the patch work across kernel versions where line numbers shift.

**Decision:**
Use injection scripts for VFS hooks. Keep core implementation as a single patch (zeromount.c + header).

**Consequences:**
- We gain: Cross-kernel-version compatibility
- We lose: Slightly more complex build process
- We must now: Test injection scripts against each target kernel version

---

### Decision 4: New Identity - ZeroMount

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Rebuilding from scratch with full understanding of metamodule architecture, kernel internals, and SUSFS coupling. Need clean identity to distinguish from failed NoMount attempt.

**Decision:**
Project renamed to ZeroMount. Binary: zm. Device: /dev/zeromount. Module ID: zeromount. Data: /data/adb/zeromount.

**Consequences:**
- We gain: Clean slate, no confusion with old implementation
- We lose: Nothing (old code was architecturally wrong)
- We must now: Rename everything in kernel patch, userspace, and build scripts

---

### Decision 3: Discard Universal Mount Hijacker

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Old service.sh (1081 lines) detected and unmounted overlay/bind/loop/tmpfs mounts from other metamodules. This was ~400 lines of complexity.

**Options Considered:**
1. **Keep hijacker** - Handle transition from other metamodules
   - Pros: Backwards compatibility
   - Cons: Fighting the framework, 400 lines, wrong execution timing

2. **Discard hijacker** - ZeroMount IS the metamodule, no mounts to hijack
   - Pros: 400 lines removed, correct architecture
   - Cons: Can't coexist with mount-based metamodules (by design - only one metamodule allowed)

**Decision:**
Discard. ZeroMount is the metamodule. There are no mounts to hijack because we don't create any. The single-metamodule constraint means no other mounting system is active.

**Consequences:**
- We gain: 400 lines removed, clean architecture
- We lose: Nothing (the constraint is enforced by KernelSU itself)
- We must now: Ensure metamount.sh handles all module injection directly

---

### Decision 2: metamount.sh as Primary Injection Point

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Old implementation put primary injection logic in service.sh (late boot), not metamount.sh (boot step 6). This caused timing issues requiring the 4-phase enable/disable dance.

**Options Considered:**
1. **Keep service.sh as primary** - Late boot, after all other scripts
   - Pros: More state available
   - Cons: Wrong timing, forces mount hijacking workaround

2. **Use metamount.sh as primary** - Boot step 6, correct metamodule timing
   - Pros: Correct per metamodule contract, no workarounds needed
   - Cons: Less state available (but we don't need it)

**Decision:**
metamount.sh is the primary injection point. service.sh handles only late-boot concerns (UID exclusions, SUSFS deferred operations).

**Consequences:**
- We gain: Correct boot timing, no 4-phase dance
- We lose: Nothing
- We must now: Put clear/iterate/inject/notify logic in metamount.sh

---

### Decision 1: VFS Redirection Over Mounting

**Date:** 2026-01-28
**Status:** Accepted

**Context:**
Need to make KernelSU module files accessible at system paths without creating detectable mount entries.

**Options Considered:**
1. **Overlay/bind mounts** - Traditional approach
   - Pros: Simple, well-understood
   - Cons: Detectable via /proc/mounts, /proc/self/mountinfo

2. **VFS path redirection** - Kernel-level path swapping at namei layer
   - Pros: Zero mounts, undetectable by mount-based detection
   - Cons: Requires kernel patch, more complex

**Decision:**
VFS path redirection (ZeroMount kernel patch) because it solves the root cause. Combined with SUSFS kstat spoofing for metadata consistency.

**Consequences:**
- We gain: Zero mount evidence, undetectable module access
- We lose: Requires custom kernel (GKI build)
- We must now: Maintain kernel patch across Android kernel versions
