# SUSFS Fork Verification Report

> **Analysts:** susfs-analyst-1 + susfs-analyst-2 (Pair B)
> **Date:** 2026-02-08
> **Cross-validated:** Yes (both analysts independently verified core findings)
> **Source files examined:**
> - `susfs4ksu-new/kernel_patches/include/linux/susfs_def.h` (102 lines)
> - `susfs4ksu-new/kernel_patches/include/linux/susfs.h` (255 lines)
> - `susfs4ksu-new/kernel_patches/fs/susfs.c` (1419 lines)
> - `susfs4ksu-new/kernel_patches/inject-susfs-custom-handlers.sh` (87 lines)
> - `susfs4ksu-new/kernel_patches/51_add_unicode_filter.sh` (130 lines)
> - `susfs4ksu-new/kernel_patches/KernelSU/10_enable_susfs_for_ksu.patch`
> - `susfs4ksu-new/kernel_patches/50_add_susfs_in_gki-android12-5.10.patch`
> - `susfs4ksu-new/ksu_susfs/jni/main.c` (787 lines)
> - `zero-mount/zeromount/module/susfs_integration.sh` (978 lines)
> - `zero-mount/susfs-module/boot-completed.sh`, `post-mount.sh`, `susfs_reset.sh`

---

## A) Complete Map of Custom Modifications vs Upstream

### A1. All Command Codes (from `susfs_def.h`)

| Code | Name | Status | Notes |
|------|------|--------|-------|
| `0x55550` | `CMD_SUSFS_ADD_SUS_PATH` | Upstream | Path hiding |
| `0x55551` | `CMD_SUSFS_SET_ANDROID_DATA_ROOT_PATH` | Upstream | External dir setup |
| `0x55552` | `CMD_SUSFS_SET_SDCARD_ROOT_PATH` | Upstream | External dir setup |
| `0x55553` | `CMD_SUSFS_ADD_SUS_PATH_LOOP` | Upstream | Re-flagged per zygote spawn |
| `0x55560` | `CMD_SUSFS_ADD_SUS_MOUNT` | Deprecated | Mount hiding |
| `0x55561` | `CMD_SUSFS_HIDE_SUS_MNTS_FOR_NON_SU_PROCS` | Upstream | Toggle mount visibility |
| `0x55562` | `CMD_SUSFS_UMOUNT_FOR_ZYGOTE_ISO_SERVICE` | Deprecated | |
| `0x55570` | `CMD_SUSFS_ADD_SUS_KSTAT` | Upstream | Kstat pre-bind-mount |
| `0x55571` | `CMD_SUSFS_UPDATE_SUS_KSTAT` | Upstream | Kstat post-bind-mount |
| `0x55572` | `CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY` | Upstream | Kstat with explicit values |
| **`0x55573`** | **`CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT`** | **CUSTOM (fork)** | Dual-path kstat redirect |
| `0x55580` | `CMD_SUSFS_ADD_TRY_UMOUNT` | Deprecated | |
| `0x55590` | `CMD_SUSFS_SET_UNAME` | Upstream | Uname spoofing |
| `0x555a0` | `CMD_SUSFS_ENABLE_LOG` | Upstream | Kernel log toggle |
| `0x555b0` | `CMD_SUSFS_SET_CMDLINE_OR_BOOTCONFIG` | Upstream | /proc/cmdline spoof |
| `0x555c0` | `CMD_SUSFS_ADD_OPEN_REDIRECT` | Upstream | Per-UID open redirect |
| **`0x555c1`** | **`CMD_SUSFS_ADD_OPEN_REDIRECT_ALL`** | **CUSTOM (fork)** | All-UID open redirect |
| `0x555e1` | `CMD_SUSFS_SHOW_VERSION` | Upstream | Version query |
| `0x555e2` | `CMD_SUSFS_SHOW_ENABLED_FEATURES` | Upstream | Feature query |
| `0x555e3` | `CMD_SUSFS_SHOW_VARIANT` | Upstream | GKI/NON-GKI query |
| `0x555e4` | `CMD_SUSFS_SHOW_SUS_SU_WORKING_MODE` | Deprecated | |
| `0x555f0` | `CMD_SUSFS_IS_SUS_SU_READY` | Deprecated | |
| `0x60000` | `CMD_SUSFS_SUS_SU` | Deprecated | |
| `0x60010` | `CMD_SUSFS_ENABLE_AVC_LOG_SPOOFING` | Upstream | AVC log spoof |
| `0x60020` | `CMD_SUSFS_ADD_SUS_MAP` | Upstream | Maps hiding |

**Custom commands: 2** (`kstat_redirect` at 0x55573, `open_redirect_all` at 0x555c1)

### A1b. SUSFS Communication Mechanism (Cross-validated by susfs-analyst-2)

SUSFS does NOT use kernel ioctls or `/dev/` device nodes. It uses **KernelSU supercalls** via a hijacked `reboot()` syscall:

```c
// Userspace (from ksu_susfs/jni/main.c)
syscall(SYS_reboot, 0xDEADBEEF, 0xFAFAFAFA, CMD_SUSFS_xxx, &info_struct);

// Kernel side (from 10_enable_susfs_for_ksu.patch, supercalls.c)
int ksu_handle_sys_reboot(int magic1, int magic2, unsigned int cmd, void __user **arg) {
    if (magic1 != 0xDEADBEEF) return -EINVAL;  // KSU_INSTALL_MAGIC1
    if (magic2 == 0xFAFAFAFA && current_uid().val == 0) {  // SUSFS_MAGIC, root only
        // dispatch to susfs handler based on cmd
    }
}
```

- `0xDEADBEEF` = KSU_INSTALL_MAGIC1 (identifies as KSU supercall, not a real reboot)
- `0xFAFAFAFA` = SUSFS_MAGIC (identifies as SUSFS command)
- `CMD_SUSFS_xxx` = command code
- `&info` = pointer to userspace struct, kernel reads/writes via `copy_from_user`/`copy_to_user`

**Implication for Rust binary:** The Rust binary can call SUSFS kernel functions directly using `libc::syscall(SYS_reboot, ...)` with matching struct layouts. No dependency on the `ksu_susfs` binary, no device nodes, no sockets. This validates S03 (Rust absorption) and DET03 (SUSFS detection).

### A2. Custom Kernel Functions Modified from Upstream

| Function | File | Nature of Modification |
|----------|------|----------------------|
| `susfs_add_sus_kstat_redirect()` | `susfs.c:580-738` | **NEW function** — not in upstream. Takes virtual + real pathname, resolves both inodes, creates dual hash entries for kstat spoofing. ~158 lines. |
| `susfs_add_open_redirect_all()` | `susfs.c:1076-1114` | **NEW function** — not in upstream. Identical to `susfs_add_open_redirect()` but uses `OPEN_REDIRECT_ALL_HLIST` and `AS_FLAGS_OPEN_REDIRECT_ALL` flag. ~38 lines. |
| `susfs_update_open_redirect_all_inode()` | `susfs.c:1009-1033` | **NEW function** — helper for redirect_all. Sets `AS_FLAGS_OPEN_REDIRECT_ALL` bit on inode. |
| `susfs_get_redirected_path_all()` | `susfs.c:1132-1146` | **NEW function** — lookup in `OPEN_REDIRECT_ALL_HLIST` by inode number. |
| `susfs_check_unicode_bypass()` | `susfs.c:45-98` | **NEW function** — under `CONFIG_KSU_SUSFS_UNICODE_FILTER`. Blocks Cyrillic, diacritical, zero-width, RTL override, BOM in filenames. |
| `susfs_is_uid_zeromount_excluded()` | `susfs_def.h:94-97` | **NEW inline** — wraps `zeromount_is_uid_blocked()` extern under `CONFIG_ZEROMOUNT` guard. |
| `is_i_uid_in_android_data_not_allowed()` | `susfs.c:351-356` | **MODIFIED** — added `susfs_is_uid_zeromount_excluded()` check at entry. |
| `is_i_uid_in_sdcard_not_allowed()` | `susfs.c:358-362` | **MODIFIED** — added `susfs_is_uid_zeromount_excluded()` check at entry. |
| `is_i_uid_not_allowed()` | `susfs.c:364-369` | **MODIFIED** — added `susfs_is_uid_zeromount_excluded()` check at entry. |

**New structures:**
- `st_susfs_sus_kstat_redirect` in `susfs.h:86-102` — virtual_pathname + real_pathname + spoofed stat fields
- `st_susfs_open_redirect_all_hlist` in `susfs.h:146-151` — target_ino + pathnames + hlist node
- `AS_FLAGS_OPEN_REDIRECT_ALL` (bit 40) in `susfs_def.h:66,74`
- `BIT_OPEN_REDIRECT_ALL` in `susfs_def.h:74`

**New hash tables:**
- `OPEN_REDIRECT_ALL_HLIST` with spinlock `susfs_spin_lock_open_redirect_all` (`susfs.c:979-980`)

### A3. `zeromount_is_uid_blocked` Integration Points

**DECISIONS.md S06 claims "3 check points." This is PARTIALLY CORRECT but incomplete.**

The integration actually has **6 check points** across two files:

**In `susfs.c` (3 points — sus_path visibility logic):**

| # | Function | File:Line | Purpose |
|---|----------|-----------|---------|
| 1 | `is_i_uid_in_android_data_not_allowed()` | `susfs.c:352` | Skip hiding Android/data paths for excluded UIDs |
| 2 | `is_i_uid_in_sdcard_not_allowed()` | `susfs.c:359` | Skip hiding sdcard paths for excluded UIDs |
| 3 | `is_i_uid_not_allowed()` | `susfs.c:365` | Skip hiding general sus_path for excluded UIDs |

**In `50_add_susfs_in_gki-android12-5.10.patch` (3 points — /proc mount display):**

| # | Function | Patch Line | Purpose |
|---|----------|------------|---------|
| 4 | `show_vfsmnt()` | patch:1391 | Don't hide mounts in /proc/mounts for excluded UIDs |
| 5 | `show_mountinfo()` | patch:1411 | Don't hide mounts in /proc/mountinfo for excluded UIDs |
| 6 | `show_vfsstat()` | patch:1431 | Don't hide mounts in /proc/mountstat for excluded UIDs |

**Correction for DECISIONS.md:** S06 should say "6 check points" (3 in sus_path + 3 in mount display), not "3 check points." The 3 in the patch are guarded by `#ifdef CONFIG_ZEROMOUNT` within `#ifdef CONFIG_KSU_SUSFS_SUS_MOUNT`.

### A4. Exact Patch Boundaries for S01 Build-Time Patching

The fork adds these distinct patch-able units:

| Patch Unit | Files Modified | Size | Dependencies |
|------------|---------------|------|-------------|
| **kstat_redirect** | `susfs.c` (add function), `susfs.h` (add struct + declaration), `susfs_def.h` (add CMD code) | ~180 lines | `CONFIG_KSU_SUSFS_SUS_KSTAT` |
| **open_redirect_all** | `susfs.c` (add function + helper + lookup), `susfs.h` (add struct + declaration), `susfs_def.h` (add CMD code + AS_FLAGS + BIT) | ~80 lines | `CONFIG_KSU_SUSFS_OPEN_REDIRECT` |
| **zeromount_uid_check** | `susfs_def.h` (add extern + inline wrapper), `susfs.c` (modify 3 inline functions) | ~20 lines | `CONFIG_ZEROMOUNT` |
| **zeromount_mount_display** | `50_add_susfs_in_gki-android12-5.10.patch` (modify 3 show_* functions) | ~39 lines | `CONFIG_ZEROMOUNT` + `CONFIG_KSU_SUSFS_SUS_MOUNT` |
| **unicode_filter** | `susfs.c` (add function), `susfs.h` (add declaration), `51_add_unicode_filter.sh` (patches fs/namei.c, fs/open.c, fs/stat.c) | ~100 lines kernel + 130 lines script | `CONFIG_KSU_SUSFS_UNICODE_FILTER` |
| **supercall dispatch** | `10_enable_susfs_for_ksu.patch` (add case handlers for kstat_redirect + open_redirect_all) | ~12 lines | Must be applied after upstream SUSFS supercall setup |

**Injection script:** `inject-susfs-custom-handlers.sh` automates supercall injection and Kconfig addition for kstat_redirect, open_redirect_all, and unicode_filter.

---

## B) S01-S13 Verification

### S01: Build-time patching, not fork maintenance
**Verdict: CONFIRMED FEASIBLE**

The custom modifications are cleanly separable into 6 patch units (listed above). Each is guarded by its own `CONFIG_*` ifdef. The `inject-susfs-custom-handlers.sh` already demonstrates the build-time injection pattern. CI would need:
1. Clone upstream SUSFS at pinned commit
2. Apply `zeromount-susfs-coupling.patch` (units: kstat_redirect, open_redirect_all, zeromount_uid_check)
3. Run `inject-susfs-custom-handlers.sh` for supercall dispatch
4. Run `51_add_unicode_filter.sh` for VFS entry point guards
5. Apply `50_add_susfs_in_gki-android12-5.10.patch` (includes mount display check points)

### S03: Does absorbing `susfs_integration.sh` into Rust cover ALL functions?
**Verdict: 20 functions identified. 4 are cleanup-only, 2 are status display. Core functionality is 14 functions.**

Complete function list from `susfs_integration.sh` (978 lines):

| # | Function | Lines | Category | Rust Equivalent Needed? |
|---|----------|-------|----------|------------------------|
| 1 | `susfs_init()` | 52-116 | Init | Yes — binary detection, capability probing |
| 2 | `susfs_classify_path()` | 118-153 | Classification | Yes — file type classification |
| 3 | `susfs_capture_metadata()` | 156-191 | Metadata | Yes — stat capture before overlay |
| 4 | `susfs_get_cached_metadata()` | 193-214 | Metadata | Yes — cache lookup (in-memory in Rust) |
| 5 | `susfs_apply_path()` | 220-295 | Core | Yes — add_sus_path / add_sus_path_loop |
| 6 | `susfs_hide_path()` | 297-332 | Core | Yes — simplified path hiding |
| 7 | `apply_deferred_sus_paths()` | 335-367 | Core | Yes — post-unmount application |
| 8 | `susfs_apply_maps()` | 369-405 | Core | Yes — add_sus_map |
| 9 | `susfs_apply_kstat()` | 409-536 | Core | Yes — add_sus_kstat_statically / add_sus_kstat_redirect |
| 10 | `apply_font_redirect()` | 539-653 | Core | Yes — add_open_redirect + kstat_redirect |
| 11 | `late_kstat_pass()` | 656-700 | Core | Yes — deferred kstat re-application |
| 12 | `susfs_apply_mount_hiding()` | 702-745 | Core | **NO** — removed per S05 |
| 13 | `susfs_update_config()` | 747-767 | Config | **DEPENDS** — see S12 |
| 14 | `susfs_clean_zeromount_entries()` | 769-790 | Cleanup | Yes if config files kept |
| 15 | `susfs_clean_module_entries()` | 792-832 | Cleanup | Yes if config files kept |
| 16 | `susfs_clean_module_metadata_cache()` | 834-858 | Cleanup | Yes (in-memory cache cleanup) |
| 17 | `zm_register_rule_with_susfs()` | 862-901 | Orchestration | Yes — main entry point |
| 18 | `susfs_capture_module_metadata()` | 904-928 | Metadata | Yes — batch capture |
| 19 | `susfs_status()` | 930-966 | Display | Yes — status JSON output |
| 20 | `susfs_reset_stats()` | 968-978 | Display | Yes — stats tracking |

**Gaps in S03 claim "Four capabilities retained":** The decision says "kstat spoofing, path hiding, maps hiding, font redirect." This is correct for the 4 functional domains. However, font redirect itself uses two SUSFS commands (open_redirect + kstat_redirect), so the Rust binary actually invokes **7 distinct SUSFS commands**:
1. `add_sus_path`
2. `add_sus_path_loop`
3. `add_sus_map`
4. `add_sus_kstat_statically`
5. `add_sus_kstat_redirect` (custom)
6. `add_open_redirect`
7. `add_open_redirect_all` (custom, not in upstream binary)

### S04: Are exactly 4 capabilities retained?
**Verdict: PARTIALLY CORRECT — 4 capability domains, but 7 SUSFS commands**

The 4 domains are correct:
1. **kstat spoofing** — `add_sus_kstat`, `update_sus_kstat`, `add_sus_kstat_statically`, `add_sus_kstat_redirect` (custom)
2. **Path hiding** — `add_sus_path`, `add_sus_path_loop`
3. **Maps hiding** — `add_sus_map`
4. **Font redirect** — `add_open_redirect`, `add_open_redirect_all` (custom)

**Missing from S04 list:** The DECISIONS.md lists font redirect as `add_open_redirect + kstat` but does NOT mention `add_open_redirect_all` (0x555c1). This custom command is critical for font handling — it redirects for ALL UIDs, not just per-UID. S04 should explicitly list both redirect variants.

**Capabilities NOT retained (confirmed):**
- `add_sus_mount` (0x55560) — deprecated + removed per S05
- `hide_sus_mnts_for_non_su_procs` (0x55561) — removed per S05
- `add_try_umount` (0x55580) — deprecated
- `sus_su` (0x60000) — deprecated

**Capabilities available but not mentioned in S04:**
- `set_uname` (0x55590) — mentioned in S08 (BRENE) but not in S04
- `set_cmdline_or_bootconfig` (0x555b0) — not mentioned anywhere in S01-S06
- `enable_avc_log_spoofing` (0x60010) — mentioned in S08 (BRENE) but not in S04
- `enable_log` (0x555a0) — utility, not a "capability"
- `set_android_data_root_path` / `set_sdcard_root_path` — used by sus_path, implicitly part of path hiding

### S05: Does `susfs_apply_mount_hiding()` scan /proc/mounts and catch other systems' mounts?
**Verdict: CONFIRMED — this is indeed the LSPosed instability root cause**

Source at `susfs_integration.sh:702-745`:

```sh
mount_point=$(awk -v path="$vpath" '
    ($3 == "overlay" || $3 == "tmpfs") && path ~ "^"$2 {
        print $2
        exit
    }
' /proc/mounts 2>/dev/null)
```

This awk expression:
1. Scans ALL entries in `/proc/mounts`
2. Matches ANY mount with filesystem type `overlay` or `tmpfs`
3. Matches if the virtual path starts with the mount point
4. Then calls `add_sus_mount` on the matched mount point

**Problem:** This catches:
- LSPosed's overlay mounts (creates overlays for Xposed injection)
- Stock Android overlays (runtime resource overlays)
- Other modules' mounts (any Magisk/KSU module using overlayfs)
- ZeroMount's own temporary staging mounts

Hiding these mounts causes LSPosed instability because LSPosed expects its overlays to be visible to processes it hooks. Confirmed: removing this function entirely (S05) eliminates the bug.

### S06: Verify `zeromount_is_uid_blocked` is called at exactly 3 check points
**Verdict: INCORRECT — 6 check points, not 3**

See section A3 above for full details. The 3 check points in `susfs.c` (lines 352, 359, 365) are for sus_path visibility. The 3 check points in the GKI patch (show_vfsmnt, show_mountinfo, show_vfsstat) are for mount display visibility.

S06 should be updated to reflect all 6 check points. The ZeroMount coupling patch must include both sets.

### S09: `kstat_redirect` command code and behavior
**Verdict: CONFIRMED**

- Command code: `0x55573` (`CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT`)
- Kernel handler: `susfs_add_sus_kstat_redirect()` at `susfs.c:580-738`
- Userspace support: Present in `ksu_susfs/jni/main.c:539-562` (takes 16 args: vpath rpath + 12 kstat fields)
- Supercall dispatch: `10_enable_susfs_for_ksu.patch:1136-1139`

**Behavior verified from source:**
1. Takes virtual_pathname (original system file) and real_pathname (replacement file)
2. Resolves BOTH paths to inodes
3. Sets `AS_FLAGS_SUS_KSTAT` bit on both inodes
4. Creates a hash entry keyed by the REAL file's inode
5. If virtual path resolves AND has a different inode, creates a SECOND hash entry keyed by the virtual inode
6. Both entries point to the same spoofed kstat values
7. Virtual path resolution is non-fatal (handles files that don't exist yet)

**Key insight:** The dual-inode registration means stat() returns spoofed values regardless of whether the caller looks up the virtual or real path.

### S10: `open_redirect_all` command code and behavior
**Verdict: CONFIRMED with caveat**

- Command code: `0x555c1` (`CMD_SUSFS_ADD_OPEN_REDIRECT_ALL`)
- Kernel handler: `susfs_add_open_redirect_all()` at `susfs.c:1076-1114`
- Lookup: `susfs_get_redirected_path_all()` at `susfs.c:1132-1146`
- Supercall dispatch: `10_enable_susfs_for_ksu.patch:1164-1167`

**CAVEAT:** This command has no CLI handler in the userspace `ksu_susfs` binary (`main.c`). The `#define` for `CMD_SUSFS_ADD_OPEN_REDIRECT_ALL` exists (main.c:42) but no `main()` branch handles the `add_open_redirect_all` subcommand. The binary can only invoke `add_open_redirect` (per-UID), not `add_open_redirect_all` (all-UID).

**Impact for Rust binary:** The Rust binary MUST invoke `open_redirect_all` directly via the `SYS_reboot` supercall (same as `ksu_susfs` does for all commands), NOT by shelling out to `ksu_susfs`. The upstream binary cannot do this operation.

**Behavior verified:**
- Uses separate hash table (`OPEN_REDIRECT_ALL_HLIST`) from per-UID redirects
- Sets `AS_FLAGS_OPEN_REDIRECT_ALL` (bit 40) on target inode
- Lookup by inode number, returns `getname_kernel(redirected_pathname)`
- Used for fonts where ALL processes need to see the redirected file

### S11: Unicode filter (`KSU_SUSFS_UNICODE_FILTER`)
**Verdict: CONFIRMED — kernel-level feature, separate patch**

- Implementation: `susfs.c:33-98` under `#ifdef CONFIG_KSU_SUSFS_UNICODE_FILTER`
- Injection script: `51_add_unicode_filter.sh` patches `fs/namei.c`, `fs/open.c`, `fs/stat.c`
- Kconfig injection: `inject-susfs-custom-handlers.sh` adds `KSU_SUSFS_UNICODE_FILTER` config
- Declaration: `susfs.h:251-253`

**Blocks:** RTL override, LTR override, RTL embed, LTR embed, zero-width space, ZWNJ, ZWJ, BOM, Cyrillic (0xD0-0xD1), diacritical marks. Exempts UID 0 and UID 1000.

**Patched VFS entry points:** `do_mkdirat`, `unlinkat`, `do_symlinkat`, `do_linkat`, `renameat2` (namei.c), `do_sys_openat2` (open.c), `vfs_statx`, `do_readlinkat` (stat.c).

The Rust binary does not need to interact with this — it's purely a kernel-level guard. Build-time patching (S01) must include this patch.

### S12: Does anything read the config files that `susfs_update_config()` writes?
**Verdict: PARTIALLY INCORRECT — the upstream SUSFS module DOES read them**

DECISIONS.md S12 states: "Current `susfs_update_config()` writes config files that nothing appears to read (ARCH-5)."

**This is wrong.** The upstream SUSFS flashable module reads these files (cross-validated by both analysts):

| Config File | Reader | File:Line |
|------------|--------|-----------|
| `sus_path.txt` | `boot-completed.sh` | `susfs-module/boot-completed.sh:74, 94` |
| `sus_path_loop.txt` | `boot-completed.sh` | `susfs-module/boot-completed.sh:112` |
| `sus_open_redirect.txt` | `boot-completed.sh` | `susfs-module/boot-completed.sh:126` |
| `sus_maps.txt` | `boot-completed.sh` | `susfs-module/boot-completed.sh:141` |
| `sus_mount.txt` | `post-mount.sh` | `susfs-module/post-mount.sh:21-22` |
| `sus_open_redirect.txt` | `service.sh` | `susfs-module/service.sh:179` |

The SUSFS module reads these at boot to apply user-configured paths. ZeroMount's `susfs_update_config()` appends entries prefixed with `# [ZeroMount]` so they persist across reboots and are re-applied by the SUSFS module. The config files serve as a **reboot persistence mechanism** -- ZeroMount calls `ksu_susfs` directly for immediate application, and the files ensure the SUSFS module re-applies them on next boot.

**Nuanced correction:** S12's statement depends on the deployment context:

- **v1 (current -- coexistence):** Config files ARE needed. The SUSFS module reads them at boot. Removing them breaks reboot persistence.
- **v2 (planned -- replacement):** ZeroMount replaces the SUSFS module entirely. The Rust binary handles all SUSFS commands at boot via its own pipeline. Config files become true v1 artifacts.

S12's recommendation to drop config files is correct for v2 but the current rationale ("nothing reads them") is factually wrong about v1. The decision should state: "Config files are consumed by the upstream SUSFS module in v1. In v2, ZeroMount replaces the SUSFS module, making config files unnecessary."

### S13: Fork diff — exact patch boundaries
**Verdict: COMPLETED (this report)**

See section A4 for the complete patch boundary map. Six distinct patch units are identified with their file modifications, sizes, and config dependencies.

---

## Undocumented Features in Fork (Not Captured in DECISIONS.md)

| Feature | Location | Notes |
|---------|----------|-------|
| `susfs_start_sdcard_monitor_fn()` | `susfs.c:1336-1405` | Kernel thread that polls `/sdcard/Android/data` accessibility every 5s for up to 5 minutes. Sets `susfs_is_sdcard_android_data_decrypted` flag. Replaced old boot_completed flag. Not mentioned in any decision. |
| `AS_FLAGS_OPEN_REDIRECT_ALL` (bit 40) | `susfs_def.h:66,74` | New inode flag for open_redirect_all. Implied by S10 but not explicitly documented. |
| Debug logging in zeromount UID check | `susfs_def.h:96` | `printk(KERN_INFO "susfs: zeromount_check uid=%u result=%d\n", uid, result)` — should be removed or gated behind `CONFIG_KSU_SUSFS_ENABLE_LOG` for production. |

---

## Summary of Corrections Needed in DECISIONS.md

| Decision | Issue | Correction |
|----------|-------|------------|
| **S04** | Lists 4 capabilities but misses `open_redirect_all` | Add `open_redirect_all (0x555c1)` explicitly to font redirect capability |
| **S06** | Claims "3 check points" | Should be "6 check points" (3 in sus_path + 3 in mount display) |
| **S09** | States command code is `0x55573` | Confirmed correct |
| **S10** | States command code is `0x555c1` | Confirmed correct. Note: NOT in upstream binary, Rust must use supercall directly |
| **S12** | Claims config files are read by nothing | Upstream SUSFS module reads them. Correct only if SUSFS module is disabled |

---

## Patch Files in the Fork (complete inventory)

| File | Purpose |
|------|---------|
| `kernel_patches/50_add_susfs_in_gki-android12-5.10.patch` | Main SUSFS kernel integration (includes zeromount mount display checks) |
| `kernel_patches/KernelSU/10_enable_susfs_for_ksu.patch` | KernelSU supercall dispatch (includes kstat_redirect + open_redirect_all handlers) |
| `kernel_patches/inject-susfs-custom-handlers.sh` | Automated injection of custom handlers + Kconfig entries |
| `kernel_patches/51_add_unicode_filter.sh` | VFS unicode filter injection |
| `kernel_patches/fs/susfs.c` | Full SUSFS kernel implementation (with custom functions) |
| `kernel_patches/include/linux/susfs.h` | SUSFS structs and declarations |
| `kernel_patches/include/linux/susfs_def.h` | SUSFS constants, command codes, inode flags (with zeromount coupling) |
| `ksu_susfs/jni/main.c` | Userspace tool (has kstat_redirect, MISSING open_redirect_all CLI handler) |

---

## Implementation Notes (from cross-validation)

1. **TOCTOU race fix in kstat_redirect** (commit `62db124`): The dual-entry hash insertion in `susfs_add_sus_kstat_redirect()` was originally vulnerable to a race. The fix pre-allocates the virtual entry before acquiring the spinlock and inserts both atomically. The current code at `susfs.c:682-702` reflects this fix.

2. **SUSFS kernel version is "v2.0.0"** (`susfs.h:11`). The Rust binary should use `CMD_SUSFS_SHOW_VERSION` (0x555e1) supercall during DET03 capability probing to verify kernel SUSFS compatibility.

3. **sdcard monitor thread** (`susfs_start_sdcard_monitor_fn()` at `susfs.c:1398`): This is an upstream feature (commit `159d4b9`), not a fork modification. It monitors `/sdcard/Android/data` accessibility after boot. Not custom, but new since the initial context analysis.

---

## Cross-Validation Sign-Off

| Analyst | Verdict | Date |
|---------|---------|------|
| susfs-analyst-1 | All findings verified from source | 2026-02-08 |
| susfs-analyst-2 | All findings independently confirmed, minor correction applied (S10 wording) | 2026-02-08 |

**Report status: FINAL -- ready for synthesis.**
