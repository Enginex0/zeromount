# Kernel/VFS Verification Report

> **Analysts:** kernel-analyst-1 + kernel-analyst-2 (Pair C, cross-checked)
> **Scope:** VFS01-07, DET01-07, CO01-04
> **Sources:** `zeromount-core.patch`, `zeromount-kernel-5.10.patch`, `zm.c`, injection scripts, kernel 5.10 source
> **Date:** 2026-02-08

---

## VFS03: Ioctl Command Count

**Claim:** "All 10 ioctl interactions from zm.c (304 lines freestanding C)"

**VERIFIED -- 10 ioctl commands in the kernel driver, but only 9 in zm.c.**

Kernel header (`include/linux/zeromount.h` in patch, lines 1264-1273):

| # | Macro | Direction | Seq | Code |
|---|-------|-----------|-----|------|
| 1 | `ZEROMOUNT_IOC_ADD_RULE` | `_IOW(0x5A, 1, struct zeromount_ioctl_data)` | 1 | `0x40185A01` (ARM64) |
| 2 | `ZEROMOUNT_IOC_DEL_RULE` | `_IOW(0x5A, 2, struct zeromount_ioctl_data)` | 2 | `0x40185A02` (ARM64) |
| 3 | `ZEROMOUNT_IOC_CLEAR_ALL` | `_IO(0x5A, 3)` | 3 | `0x5A03` |
| 4 | `ZEROMOUNT_IOC_GET_VERSION` | `_IOR(0x5A, 4, int)` | 4 | `0x80045A04` |
| 5 | `ZEROMOUNT_IOC_ADD_UID` | `_IOW(0x5A, 5, unsigned int)` | 5 | `0x40045A05` |
| 6 | `ZEROMOUNT_IOC_DEL_UID` | `_IOW(0x5A, 6, unsigned int)` | 6 | `0x40045A06` |
| 7 | `ZEROMOUNT_IOC_GET_LIST` | `_IOR(0x5A, 7, int)` | 7 | `0x80045A07` |
| 8 | `ZEROMOUNT_IOC_ENABLE` | `_IO(0x5A, 8)` | 8 | `0x5A08` |
| 9 | `ZEROMOUNT_IOC_DISABLE` | `_IO(0x5A, 9)` | 9 | `0x5A09` |
| 10 | `ZEROMOUNT_IOC_REFRESH` | `_IO(0x5A, 10)` | 10 | `0x5A0A` |

The kernel switch statement handles all 10 (`zeromount-core.patch`, `fs/zeromount.c:1108-1120`).

**zm.c only defines 9 ioctl codes** (`zm.c:120-128`):
- `IOCTL_ADD` (0x40185A01), `IOCTL_DEL` (0x40185A02), `IOCTL_CLEAR` (0x5A03), `IOCTL_VER` (0x80045A04), `IOCTL_ADD_UID` (0x40045A05), `IOCTL_DEL_UID` (0x40045A06), `IOCTL_LIST` (0x80045A07), `IOCTL_ENABLE` (0x5A08), `IOCTL_DISABLE` (0x5A09)

**MISSING from zm.c:** `IOCTL_REFRESH` (0x5A0A). This is consistent with VFS07's claim.

**Correction to DECISIONS.md:** VFS03 says "All 10 ioctl interactions from zm.c" -- zm.c actually only uses 9. The 10th (REFRESH) exists only in the kernel, not in zm.c.

---

## VFS04: Ghost Directory Bug (dirs_ht not cleaned in del_rule)

**Claim:** "Kernel bug (BUG-H1: dirs_ht not cleaned) requires kernel patch fix."

**VERIFIED -- `del_rule` does NOT clean `dirs_ht`.**

`zeromount_ioctl_del_rule()` at `zeromount-core.patch`, `fs/zeromount.c:934-972`:
- Removes rule from `zeromount_rules_ht` (line 956: `hash_del_rcu(&rule->node)`)
- Removes from `zeromount_ino_ht` (line 958: `hash_del_rcu(&rule->ino_node)`)
- Removes from `zeromount_rules_list` (line 959: `list_del(&rule->list)`)
- Frees rule via RCU (line 967: `call_rcu(&rule->rcu, zeromount_free_rule_rcu)`)
- **NO reference to `zeromount_dirs_ht` anywhere in this function**

`zeromount_ioctl_clear_rules()` at `fs/zeromount.c:974-998`:
- Cleans `zeromount_rules_ht` (lines 983-988)
- Cleans `zeromount_uid_ht` (lines 991-994)
- **Also does NOT clean `zeromount_dirs_ht`**

**Both `del_rule` AND `clear_all` leak `dirs_ht` entries.** The claim in CO01 that "clear_all" also needs a fix is correct. The `zeromount_dir_node` and `zeromount_child_name` entries allocated in `zeromount_auto_inject_parent()` (lines 772-856) are never freed by any ioctl path.

**Bug severity:** Memory leak on every hot-reload cycle. Each `CLEAR_ALL` + re-inject leaves orphaned `dirs_ht` entries in kernel memory.

---

## VFS05: ARM64 vs ARM32 Struct Sizes

**Claim:** "ARM64 build produces 0x40185A01 (24-byte struct), ARM32 build produces 0x400C5A01 (12-byte struct)"

**VERIFIED with caveats.**

`struct zeromount_ioctl_data` from `include/linux/zeromount.h` (patch lines 1276-1280):
```c
struct zeromount_ioctl_data {
    char __user *virtual_path;   // pointer
    char __user *real_path;      // pointer
    unsigned int flags;          // 4 bytes
};
```

**ARM64 (LP64):** pointers = 8 bytes each. `8 + 8 + 4 = 20 bytes`, with 4 bytes padding for alignment = **24 bytes**.
- `_IOW(0x5A, 1, struct zeromount_ioctl_data)` => direction=01 (write), size=24=0x18 => `0x40185A01`. **MATCHES claim.**

**ARM32 (ILP32):** pointers = 4 bytes each. `4 + 4 + 4 = 12 bytes`, no padding needed.
- `_IOW(0x5A, 1, struct zeromount_ioctl_data)` => direction=01 (write), size=12=0x0C => `0x400C5A01`. **MATCHES claim.**

**zm.c hardcodes ARM64 values** (`zm.c:120`): `#define IOCTL_ADD 0x40185A01`. The ARM32 `ioctl_data` struct in zm.c (lines 69-75) uses split lo/hi fields:
```c
struct ioctl_data {
    unsigned int vp_lo;  // 4 bytes
    unsigned int vp_hi;  // 4 bytes
    unsigned int rp_lo;  // 4 bytes
    unsigned int rp_hi;  // 4 bytes
    unsigned int flags;  // 4 bytes
    unsigned int _pad;   // 4 bytes
};  // = 24 bytes total
```

**BUG CONFIRMED (BUG-H2):** ARM32 zm.c sends `IOCTL_ADD = 0x40185A01` (24-byte struct) but the kernel expects `0x400C5A01` (12-byte struct). The ARM32 struct in zm.c is padded to 24 bytes to match the hardcoded ARM64 ioctl number, but the kernel's `copy_from_user(&data, ...)` with `sizeof(struct zeromount_ioctl_data)` will only read 12 bytes on ARM32. This means the kernel reads `vp_lo` and `rp_lo` correctly as the pointer values, but the ioctl number mismatch means `_IOC_TYPE(cmd)` check may still pass (0x5A matches), but `_IOC_SIZE` will be wrong.

Actually, the kernel `compat_ioctl` handler points to the same `zeromount_ioctl` function (patch line 1165: `.compat_ioctl = zeromount_ioctl`). Since the switch statement compares `cmd` against compile-time `ZEROMOUNT_IOC_ADD_RULE`, on a 32-bit kernel compilation the constant would be `0x400C5A01`, not `0x40185A01`. **The mismatch is real -- zm.c ARM32 sends the wrong ioctl number.** VFS05's fix approach (compile-time derivation in Rust) is correct.

---

## VFS06: GET_STATUS Ioctl (0x80045A0B)

**Claim:** "New GET_STATUS ioctl (proposed 0x80045A0B)"

**VERIFIED -- This ioctl does NOT exist in the current kernel patch.**

The kernel switch statement (`fs/zeromount.c:1108-1120`) handles commands 1-10. There is no case for sequence number 11 (which would be `0x80045A0B` for `_IOR(0x5A, 11, int)`). The `default:` case returns `-EINVAL`.

The `zeromount_enabled` atomic is available (`fs/zeromount.c:67`) but no ioctl exposes it for querying. The only way to read engine state currently is checking if the device exists and doing `GET_VERSION`.

**No code conflict:** Adding sequence number 11 is safe since 1-10 are used. The proposed `0x80045A0B = _IOR(0x5A, 11, int)` is correctly calculated.

---

## VFS07: REFRESH Ioctl (0x5A0A)

**Claim:** "REFRESH is 0x5A0A" and "missing from zm.c"

**VERIFIED -- both claims correct.**

Kernel defines `ZEROMOUNT_IOC_REFRESH = _IO(ZEROMOUNT_IOC_MAGIC, 10)` (header line 1273). Since `_IO` has no direction bits and no size: `_IO(0x5A, 10)` = `0x00005A0A` = `0x5A0A`. **Matches claim.**

Kernel handler exists (`fs/zeromount.c:1118`):
```c
case ZEROMOUNT_IOC_REFRESH: zeromount_force_refresh_all(); return 0;
```

The `zeromount_force_refresh_all()` function (lines 229-263) iterates all rules, copies their virtual paths under spinlock, then calls `zeromount_flush_dcache()` on each to invalidate dcache entries.

**zm.c (304 lines) does NOT define `IOCTL_REFRESH`** -- confirmed by examining lines 120-128: the last defined ioctl is `IOCTL_DISABLE = 0x5A09`. The `zm.c` command parser (lines 143-267) handles: add, del, clear, blk, unb, list, ver, enable, disable. No "refresh" command.

---

## DET02: Kernel Capability Probe Mechanism

**Claim:** "(1) /dev/zeromount existence, (2) GET_VERSION ioctl, (3) /proc/filesystems, (4) /proc/config.gz for CONFIG_ZEROMOUNT=y"

**VERIFIED -- the kernel driver uses miscdev, plus sysfs.**

1. **`/dev/zeromount`** -- Created via `misc_register(&zeromount_device)` at `fs/zeromount.c:1176`. The miscdevice is named `"zeromount"` with mode `0600` (line 1170). **Existence check is valid.**

2. **`GET_VERSION` ioctl** -- Returns `ZEROMOUNT_VERSION` (= 1) directly (line 1109). Notably, `GET_VERSION` is the ONLY ioctl that does NOT require `CAP_SYS_ADMIN` (line 1103: `if (cmd != ZEROMOUNT_IOC_GET_VERSION) { if (!capable(CAP_SYS_ADMIN)) return -EPERM; }`). **This is correct design -- non-root probing for version.**

3. **`/proc/filesystems`** -- ZeroMount does NOT register a filesystem type. It uses `fs_initcall(zeromount_init)` (line 1189), not `register_filesystem()`. **The `/proc/filesystems` probe would NOT find "zeromount".** This claim in DET02 is INCORRECT for the current implementation.

4. **`/proc/config.gz`** -- Would show `CONFIG_ZEROMOUNT=y` if kernel config is exposed. This is a standard kernel feature. **Valid but only available if `CONFIG_IKCONFIG_PROC=y`.**

**Additional probe surface:** The driver creates a sysfs kobject at `/sys/kernel/zeromount/debug` (lines 1179-1183). This could also be probed.

**Correction needed for DET02:** Step 3 (`/proc/filesystems`) should be removed or replaced with `/sys/kernel/zeromount/` existence check. ZeroMount is not a filesystem -- it's a miscdevice.

---

## CO01: dirs_ht Leak Code Path

**Traced the exact leak path:**

1. **Allocation:** `zeromount_auto_inject_parent()` (`fs/zeromount.c:772-856`) allocates:
   - `struct zeromount_dir_node` (line 829: `kzalloc(sizeof(*dir_node), GFP_ATOMIC)`)
   - `dir_node->dir_path` (line 832: `kstrdup(parent_path, GFP_ATOMIC)`)
   - `struct zeromount_child_name` (line 845: `kzalloc(sizeof(*child), GFP_ATOMIC)`)
   - `child->name` (line 847: `kstrdup(name, GFP_ATOMIC)`)
   - Adds to `zeromount_dirs_ht` (line 834: `hash_add_rcu(zeromount_dirs_ht, &dir_node->node, hash)`)

2. **Trigger:** Called from `zeromount_ioctl_add_rule()` (line 927) when a virtual path doesn't exist on the real filesystem (the `kern_path()` for `rule->virtual_path` returns non-zero).

3. **Leak on del_rule:** `zeromount_ioctl_del_rule()` (lines 934-972) never touches `zeromount_dirs_ht`. After deleting a rule, its corresponding dir_node + children remain in the hash table.

4. **Leak on clear_all:** `zeromount_ioctl_clear_rules()` (lines 974-998) only cleans `zeromount_rules_ht` and `zeromount_uid_ht`. `zeromount_dirs_ht` is not cleaned.

5. **Consequence:** After `CLEAR_ALL` + re-inject, orphaned `dirs_ht` entries remain. Note: `zeromount_auto_inject_parent()` has a `child_exists` dedup check (`strcmp(child->name, name)` at core patch lines 837-841), so exact filename duplicates won't appear on re-inject. The real problem is **stale/ghost entries** -- dir_nodes from previously-injected paths that are no longer backed by active rules. These cause the "ghost directory" symptom: file appears in `ls`/readdir but can't be opened (since no rule maps the virtual path to a real file anymore). (Nuance identified by kernel-analyst-2.)

**Memory per leaked rule:** ~`sizeof(zeromount_dir_node)` + path string + N * (`sizeof(zeromount_child_name)` + name string). Approximately 100-300 bytes per directory-level entry.

---

## CO02: Partition Lists from 4 Shell Scripts

**Extracted actual lists:**

| Script | File:Line | Partition List |
|--------|-----------|---------------|
| `metamount.sh` | `:14` | `system vendor product system_ext odm oem my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_stock mi_ext cust optics prism` **(20 partitions)** |
| `monitor.sh` | `:15` | `system vendor product system_ext odm oem mi_ext my_heytap prism optics` **(10 partitions)** |
| `sync.sh` | `:14` | `system vendor product system_ext odm oem mi_ext my_heytap prism optics oem_dlkm system_dlkm vendor_dlkm` **(13 partitions)** |
| `zm-diag.sh` | `:10` | `system vendor product system_ext odm oem` **(6 partitions)** |

**Union of all 4 lists (23 unique partitions):**
`system vendor product system_ext odm oem my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_stock mi_ext cust optics prism oem_dlkm system_dlkm vendor_dlkm`

**Analysis:**
- `zm-diag.sh` has the most restrictive list (6) -- would miss OEM-specific partitions
- `monitor.sh` has 10 -- misses manufacturer partitions
- `sync.sh` has 13 -- adds DLKMs but misses manufacturer partitions
- `metamount.sh` has 20 -- most complete but still misses DLKMs

**CO02 claim of "23 unique partitions" is VERIFIED.** The decision to create a unified constant is clearly justified by the inconsistency.

**Partitions unique to each script:**
- Only in `metamount.sh`: `my_bigball my_carrier my_company my_engineering my_manifest my_preload my_product my_region my_stock cust`
- Only in `sync.sh`: `oem_dlkm system_dlkm vendor_dlkm`
- All 4 share: `system vendor product system_ext odm oem`

---

## Kernel Injection Point Verification

### namei.c Injection Points

Verified against `/home/claudetest/gki-build/kernel-test/android12-5.10-2024-05/common/fs/namei.c`:

1. **`#include "mount.h"`** -- Found at line 44. Script anchors on this. **VALID.**

2. **`audit_getname(result);` followed by `return result;`** -- Found at lines 205-206 in `getname_flags()`. Script injects the `zeromount_getname_hook(result)` call between audit and return. **VALID anchor point.**

3. **`int generic_permission(struct inode *inode, int mask)`** -- Found at line 349 with `int ret;` at line 351. **VALID -- script pattern matches.**

4. **`int inode_permission(struct inode *inode, int mask)`** -- Found at line 442 with `int retval;` at line 444. **VALID -- script pattern matches.**

### readdir.c Injection Points

Verified against `/home/claudetest/gki-build/kernel-test/android12-5.10-2024-05/common/fs/readdir.c`:

1. **`SYSCALL_DEFINE3(getdents,`** -- Found at line 271. Has `int error;` at line 280, `return -EBADF;` at line 284, `error = buf.error;` at line 288, `fdput_pos(f);` at line 298. **All anchor points VALID.**

2. **`SYSCALL_DEFINE3(getdents64,`** -- Found at line 354. Has `int error;` at line 363, `return -EBADF;` at line 367, `error = buf.error;` at line 371, `fdput_pos(f);` at line 382. **All anchor points VALID.**

3. **`COMPAT_SYSCALL_DEFINE3(getdents,`** -- Found at line 522. Has `int error;` at line 531, `return -EBADF;` at line 535, `error = buf.error;` at line 539, `fdput_pos(f);` at line 549. **All anchor points VALID.**

### Kernel Version Concerns

The injection scripts are designed for 5.10 (android12-5.10). The patterns (`audit_getname`, `generic_permission`, `inode_permission`, `SYSCALL_DEFINE3(getdents...)`) are stable VFS interfaces that exist across 5.x kernels. However:

- **6.x kernels** may have refactored `getdents` to use the new `class_*` patterns or different local variable structures. The `zeromount-kernel-5.10.patch` appears to be essentially identical to `zeromount-core.patch` (same 1158-line `zeromount.c`, same 160-line header), suggesting the core patch targets a single kernel variant.
- The `compat_ioctl = zeromount_ioctl` approach is correct for 5.10 since `compat_ioctl` uses the same function pointer type.

### 5.10 vs Core Patch Feature Delta (cross-checked with kernel-analyst-2)

CONTEXT.md claims 5.10 patch is "missing: statfs spoofing, xattr spoofing, relative-path stat, directory-already-redirected check in readdir, recursive auto-parent injection."

**2 of 5 "missing" claims are WRONG:**

| Feature | Core Patch | 5.10 Patch | CONTEXT.md Claim |
|---------|-----------|-----------|-----------------|
| `zeromount_spoof_statfs()` | Line 656 | Line 567 | "missing" -- **WRONG, present** |
| `zeromount_spoof_xattr()` | Line 727 | Line 638 | "missing" -- **WRONG, present** |
| `zeromount_build_absolute_path()` | Line 414 | Absent | "missing" -- **CORRECT** |
| "Skip injection if directory redirected" readdir check | Lines 539, 607 | Absent | "missing" -- **CORRECT** |
| Recursive `auto_inject_parent(parent_path, DT_DIR)` | Line 815 | Absent | "missing" -- **CORRECT** |

---

## Additional Cross-Checked Findings (from kernel-analyst-2)

### Concurrency Model
- Single `DEFINE_SPINLOCK(zeromount_lock)` for all write operations. **CONFIRMED.**
- RCU (`rcu_read_lock` + `hash_for_each_possible_rcu`) for all hot-path reads. **CONFIRMED.**
- `call_rcu()` / `kfree_rcu()` for safe deferred frees. **CONFIRMED.**

### zeromount_should_skip() -- 8 Safety Conditions (zeromount.c:86-107)
All independently verified:
1. `ZEROMOUNT_DISABLED()` -- atomic check
2. `zm_is_recursive()` -- android_oem_data1 bit 0 / journal_info fallback
3. `in_interrupt() || in_nmi() || oops_in_progress`
4. `PF_KTHREAD | PF_EXITING`
5. `!current->mm || !current->nsproxy`
6. `PF_MEMALLOC_NOFS`
7. `zeromount_is_critical_process()` -- PID 1 or PID 2
8. `susfs_is_current_proc_umounted()` (under `#ifdef CONFIG_KSU_SUSFS`)

### CO03: Enable-before-SUSFS Race (BUG-M3)
Kernel side confirmed: `enable` just does `atomic_set(&zeromount_enabled, 1)` with no coupling to SUSFS state. The race is purely a script-ordering issue in metamount.sh. The Rust pipeline fix (inject -> SUSFS -> enable -> refresh) is the correct approach.

---

## DET01: Scenario Definitions

**VERIFIED against kernel source.** The 4 scenarios are logically consistent:
- FULL: `/dev/zeromount` exists + version ioctl works + SUSFS binary found with full capabilities
- SUSFS_FRONTEND: Driver present + partial SUSFS
- KERNEL_ONLY: Driver present + no SUSFS binary
- NONE: No driver

The detection order (device existence -> ioctl probe -> SUSFS binary check) is sound. No kernel code conflicts.

---

## DET03-DET07: Detection System Claims

**DET03:** SUSFS three-layer probe is a userspace concern. No kernel code to verify. The `.disabled` marker check, binary search paths, and custom ioctl probing are all runtime behavior for the Rust binary.

**DET04:** inotify-based watching replaces 5s polling. Pure userspace. No kernel concern.

**DET05:** Strategy selection is pure Rust binary logic. No kernel concern.

**DET06:** Graceful degradation. The kernel driver operates independently of SUSFS binary -- confirmed by the `#ifdef CONFIG_KSU_SUSFS` guards in the kernel code (only `zeromount_should_skip` and `zeromount_is_uid_blocked` reference SUSFS). If SUSFS is absent, the driver functions normally.

**DET07:** Status JSON at userspace level. The kernel's `zeromount_enabled` atomic (line 67) and rule/UID counts would need to be exposed for accurate reporting. Currently, only `GET_VERSION` is queryable. `GET_STATUS` (VFS06) would add the engine-active flag.

---

## Summary Table

| Claim | Verdict | Notes |
|-------|---------|-------|
| VFS03: 10 ioctl commands | PARTIALLY CORRECT | Kernel has 10, zm.c has 9 (missing REFRESH) |
| VFS04: dirs_ht not cleaned | CONFIRMED | Neither del_rule nor clear_all touches dirs_ht |
| VFS05: ARM64=24B, ARM32=12B | CONFIRMED | Struct sizes match. BUG-H2 in zm.c confirmed |
| VFS06: GET_STATUS 0x80045A0B | CONFIRMED NEW | Does not exist yet, no conflicts |
| VFS07: REFRESH=0x5A0A, missing from zm.c | CONFIRMED | Kernel has handler, zm.c lacks it |
| DET02: Probe mechanism | PARTIALLY INCORRECT | /proc/filesystems won't work (not a filesystem) |
| CO01: dirs_ht leak | CONFIRMED | Full leak path traced, both del_rule and clear_all |
| CO02: 23 unique partitions | CONFIRMED | 4 scripts have 6/10/13/20 partitions respectively |
| CO03: Enable-before-SUSFS race | CONFIRMED | Script ordering issue, kernel enable is atomic-only |
| Injection points (namei.c) | VALID for 5.10 | All 4 anchor patterns found |
| Injection points (readdir.c) | VALID for 5.10 | All anchor patterns found in 3 syscalls |
| 5.10 patch "missing" claims | 2 of 5 WRONG | statfs + xattr spoofing ARE in 5.10 patch |
| Concurrency model | CONFIRMED | spinlock writes, RCU reads, deferred frees |
| zeromount_should_skip() | CONFIRMED | All 8 conditions verified with line citations |
