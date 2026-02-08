# Mount Engine Decisions (ME01-ME12) Verification Report

> **Analyst:** mount-analyst-1
> **Date:** 2026-02-08
> **Sources:** DECISIONS.md (ME section), kernel source (android12-5.10-2024-05), metamount.sh, mountify context, hybrid_mount context

---

## ME01: Storage backend cascade -- EROFS -> tmpfs -> ext4

**Decision claims:** EROFS preferred (compressed read-only, matches Android's native partition format). tmpfs fallback if kernel supports xattr (`CONFIG_TMPFS_XATTR=y`). ext4 loopback as last resort.

**Verification:**

1. **EROFS support in 5.10 GKI kernel: CONFIRMED**
   - Source: `/home/claudetest/gki-build/kernel-test/android12-5.10-2024-05/common/fs/erofs/` directory exists with full EROFS implementation (super.c, inode.c, data.c, dir.c, namei.c, xattr.c, zdata.c, etc.)
   - GKI defconfig at `common/arch/arm64/configs/gki_defconfig:592` has `CONFIG_EROFS_FS=y`
   - EROFS compression support (`CONFIG_EROFS_FS_ZIP`) defaults to `y` in Kconfig
   - Kconfig description explicitly mentions "Android OS for mobile phones" as a target use case
   - hybrid_mount already implements EROFS as a storage backend with `mkfs.erofs -z lz4hc` (context-meta-hybrid_mount.md, section 7)

2. **tmpfs xattr support: NUANCED -- NOT enabled by default in GKI**
   - `CONFIG_TMPFS=y` is set in GKI defconfig (line 586)
   - `CONFIG_TMPFS_XATTR` defaults to `n` in `common/fs/Kconfig:190-193`
   - `CONFIG_TMPFS_POSIX_ACL` (which auto-selects `TMPFS_XATTR` via `select`) is NOT set in GKI defconfig
   - This means **stock GKI 5.10 kernels may NOT have tmpfs xattr support**
   - The decision is correct that runtime probing is needed, but the claim that tmpfs is a reliable "fallback" is optimistic. On stock GKI, tmpfs xattr may fail, making ext4 the actual fallback for most devices
   - Mountify's `customize.sh` (context-mountify.md, section 3, step 4) does exactly this: tests tmpfs xattr at install time and sets a `no_tmpfs_xattr` flag if it fails
   - **Important caveat:** OEM kernels frequently add `CONFIG_TMPFS_XATTR=y` or `CONFIG_TMPFS_POSIX_ACL=y`, so real-world device coverage varies. The cascade design is correct.

3. **ext4 loopback feasibility: CONFIRMED**
   - Both mountify and hybrid_mount implement ext4 loopback images successfully
   - Mountify creates sparse images with `dd seek=$sparse_size` and `mkfs.ext4 -O ^has_journal`
   - hybrid_mount calculates image size as `max(module_size * 1.2, 64MB)` with `e2fsck` repair

**Verdict: CORRECT with caveat** -- The cascade order is sound. However, the decision should note that on stock GKI 5.10, tmpfs xattr is likely absent, making the effective cascade EROFS -> ext4 for many devices. The runtime probing design handles this correctly.

---

## ME02: OverlayFS mounting -- new mount API + legacy fallback

**Decision claims:** New `fsopen`/`fsmount`/`move_mount` API (Linux 5.2+) preferred. Legacy `mount()` syscall as fallback. "All KSU-supported kernels are 5.x+, so new API should be available."

**Verification:**

1. **fsopen/fsmount/move_mount in 5.10 kernel: CONFIRMED**
   - `SYSCALL_DEFINE2(fsopen, ...)` at `common/fs/fsopen.c:115`
   - `SYSCALL_DEFINE3(fsmount, ...)` at `common/fs/namespace.c:3476`
   - `sys_move_mount` declared at `common/include/linux/syscalls.h:1024`
   - These syscalls were introduced in Linux 5.2 and are present in Android's 5.10 GKI kernel

2. **"All KSU-supported kernels are 5.x+" claim: PARTIALLY CORRECT**
   - KernelSU officially supports 5.x+ kernels for its latest versions
   - However, some older KSU builds targeted 4.14 kernels (GKI 4.14 for Android 11)
   - The new mount API was NOT available in 4.14 kernels (added in 5.2)
   - The legacy fallback is therefore essential, not just theoretical

3. **hybrid_mount implements both: CONFIRMED**
   - context-meta-hybrid_mount.md section 6 shows both new-style (`fsopen/fsmount/move_mount`) and legacy (`mount()`) implementations
   - Legacy fallback uses `mount(source, dest, "overlay", empty_flags, "lowerdir=...")`
   - Commas in paths are escaped with `\,` for legacy API

**Verdict: CORRECT** -- The dual-path approach is well-validated by both kernel source inspection and hybrid_mount's existing implementation. The legacy fallback is necessary for older kernels.

---

## ME05: BFS planner -- never mount at partition roots

**Decision claims:** Breadth-first planner determines minimum overlay mount points. Hard constraint: never overlay-mount at `/system`, `/vendor`, etc. directly -- always mount one level deeper.

**Verification against current metamount.sh:**

1. **Current metamount.sh does NOT use BFS or overlay mounts at all**
   - metamount.sh (`/home/claudetest/zero-mount/zeromount/module/metamount.sh`) is a VFS-based approach
   - It uses a `zm` binary (`$LOADER`) to inject VFS rules via `$LOADER add "$virtual_path" "$real_path"`
   - There is NO overlay mounting logic, NO `controlled_depth()`, NO `single_depth()`
   - The current code iterates modules and injects per-file VFS rules, not mount points

2. **Mountify implements the pattern described: CONFIRMED**
   - `single_depth()` handles `/system/*` subdirectories (not `/system` itself)
   - `controlled_depth()` handles sensitive partitions one level deeper
   - context-mountify.md section 5: "Perform overlay mounts -- `single_depth()` for `/system/*` subdirectories, `controlled_depth()` for each target partition"
   - Specifically excludes `odm`, `product`, `system_ext`, `vendor` from `single_depth()` for special handling

3. **hybrid_mount implements BFS: CONFIRMED**
   - context-meta-hybrid_mount.md section 6: `SENSITIVE_PARTITIONS` includes all 20 of the 21 builtin partitions except `system`
   - Planner descends into subdirectories for sensitive partitions rather than mounting at root
   - "When the target matches a sensitive partition name or 'system', the planner re-queues each subdirectory instead of creating an overlay at the root"

**Verdict: CORRECT as a design decision for the new Rust binary.** The claim "matches mountify's `controlled_depth()`" is accurate. The current metamount.sh does NOT implement this because it uses VFS redirection (per-file), not overlay mounts. The BFS planner is new functionality for the overlay/magic mount fallback paths.

---

## ME06: SAR child overlay handling

**Decision claims:** Detect System-as-Root symlink situation: `/product` may be symlink to `/system/product` (legacy) or a separate mount point (modern SAR). Resolve before mounting.

**Verification:**

1. **SAR partition layout is real and handled by references: CONFIRMED**
   - Mountify context section 5: "`controlled_depth()` handles legacy vs modern partition layout: Legacy (`/$folder` is a symlink to `/system/$folder`): mounts at `/system/$folder/subdir`. Modern (`/$folder` is a real mount point): mounts at `/$folder/subdir`"
   - hybrid_mount context section 6 (magic mount): "Handles Android SAR partition symlinks: for `vendor`, `system_ext`, `product`, `odm`, moves nodes from `system` subtree to root if the partition exists at `/<partition>` and `/system/<partition>` is a symlink"

2. **Current metamount.sh handles this implicitly:**
   - The TARGET_PARTITIONS list includes both standalone names (`vendor`, `product`, `system_ext`) and `system` -- the VFS rule injection uses the path directly (`/$relative_path`) so symlink resolution happens naturally at the kernel VFS layer

3. **"Matches mountify.sh line 150 pattern" claim: PLAUSIBLE**
   - Mountify's `controlled_depth()` handles both layouts. The line reference is from the context document rather than the actual mountify source (which we don't have direct access to), but the logic described matches the SAR handling pattern

**Verdict: CORRECT** -- SAR symlink detection is a real requirement confirmed by both reference implementations. The decision correctly identifies the two cases and the need for resolution before mounting.

---

## ME08: Full whiteout and opaque directory support

**Decision claims:** Support all three whiteout formats: char device (`mknod c 0 0`), xattr (`trusted.overlay.whiteout=y`), AUFS (`.wh.*` files). Plus opaque directories (`trusted.overlay.opaque=y`). "Matches `metamount.sh:171-207` detection logic."

**Verification against metamount.sh:**

1. **Character device whiteout (format 1): CONFIRMED**
   - `metamount.sh:176-181`: `is_whiteout()` checks `[ -c "$path" ]` then verifies major=0, minor=0 via `busybox stat -c '%t'` and `'%T'`

2. **Xattr whiteout (format 2): CONFIRMED**
   - `metamount.sh:184-186`: Checks zero-size file with `trusted.overlay.whiteout="y"` xattr via `getfattr`

3. **AUFS whiteout (format 3): CONFIRMED**
   - `metamount.sh:199-207`: `is_aufs_whiteout()` checks for `.wh.*` prefix in filename

4. **Opaque directory: CONFIRMED**
   - `metamount.sh:191-197`: `is_opaque_dir()` checks `trusted.overlay.opaque="y"` xattr

5. **Line reference accuracy: CONFIRMED**
   - The decision claims "matches `metamount.sh:171-207`" -- the `is_whiteout()` function starts at line 171, `is_aufs_whiteout()` ends at line 207. Accurate reference.

6. **getfattr availability handling: CONFIRMED**
   - `metamount.sh:153-169`: Multiple fallback paths for `getfattr` (system, toybox, busybox, $PATH)
   - `HAS_GETFATTR` flag guards all xattr-dependent checks

**Verdict: FULLY CORRECT** -- All three whiteout formats AND opaque directories are implemented in the current metamount.sh exactly as described. The line reference is accurate.

---

## ME09: Source="KSU" on all mounts

**Decision claims:** Hardcode mount source name to `"KSU"` for all overlay/tmpfs mounts. Required for KernelSU's `try_umount` to recognize and reverse these mounts per-app.

**Verification:**

1. **Mount source name matters for try_umount: NUANCED**
   - KSU's `try_umount` mechanism works via **explicit registration of mount paths**, not by scanning source names in `/proc/mounts`
   - Mountify uses `ksu_susfs add_try_umount` (SUSFS path) or `ksud kernel umount add` (native KSU path) to register individual mount points
   - hybrid_mount uses `TryUmount` from the `ksu` crate to register paths: `send_umountable(path)` -> `list.umount()`
   - The source name appears in `/proc/mounts` but is primarily used by **zygisk-based unmount tools** (Shamiko, ZygiskNext), not by KSU's kernel-level `try_umount`

2. **Mountify's approach: Source name is configurable, not hardcoded to "KSU"**
   - Mountify uses `MOUNT_DEVICE_NAME` config variable (default `"overlay"`)
   - Can be set to `"KSU"`, `"APatch"`, or `"magisk"` for compatibility with different unmount tools
   - This is the "device name" / source field in the mount syscall

3. **hybrid_mount's approach: Source name from config**
   - `mountsource = "KSU"` in config.toml
   - `detect_mount_source()` returns `"KSU"` if `ksu::version()` succeeds, else `"APatch"`
   - Used as the `source` parameter in mount calls

4. **Decision accuracy:**
   - The claim "Required for KernelSU's `try_umount` to recognize" is **misleading** -- `try_umount` uses explicit path registration, not source name matching
   - The source name matters for **third-party zygisk unmount tools** that scan `/proc/mounts`
   - Hardcoding to `"KSU"` is reasonable as a default but the claim about WHY is imprecise

**Verdict: PARTIALLY CORRECT** -- Using `"KSU"` as the mount source is consistent with reference implementations and is reasonable. However, the stated rationale is incorrect: KSU's `try_umount` does NOT identify mounts by source name. It works via explicit mount path registration. The source name matters for zygisk-based tools (Shamiko, ZygiskNext) that grep `/proc/mounts`. The decision should also note that APatch may need `"APatch"` as the source name (the decision already mentions this deferral).

---

## ME11: Random mount paths -- auto-generated per boot, never persisted

**Decision claims:** All staging areas use a random 12-char alphanumeric path under `/mnt/` (or `/mnt/vendor/` fallback). Never written to config, never exposed.

**Verification:**

1. **Reference implementations confirm the pattern:**
   - Mountify uses `/mnt/mountify/` or `/mnt/vendor/mountify/` (user-configurable `FAKE_MOUNT_NAME`)
   - hybrid_mount uses `/mnt/<random-10-chars>/` (10-char random, not 12)
   - ZeroMount's decision specifies 12-char

2. **`/mnt/` usability on Android: CONFIRMED with caveats**
   - Mountify probes `/mnt` and `/mnt/vendor` for writability (context-mountify.md section 4, step 5)
   - `/mnt/` is typically writable at post-fs-data time on rooted devices
   - No known SELinux restrictions that would block file creation under `/mnt/` at this boot stage (both mountify and hybrid_mount use it successfully)
   - **Caveat:** `/mnt/` is a tmpfs in Android, so the directory is inherently ephemeral (lost on reboot). This actually supports the "never persisted" design naturally.

3. **Security model:**
   - Randomization prevents prediction of the staging path
   - Per-boot generation means the path changes every reboot
   - Detection window is between directory creation and backing file deletion (ME12)

**Verdict: CORRECT** -- The pattern is well-established in both reference implementations. The 12-char (vs hybrid_mount's 10-char) is a minor detail difference, not a design flaw. `/mnt/` is a valid location. No significant SELinux concerns at the boot stage where this runs.

---

## ME12: NukeExt4Sysfs -- destroy backing evidence

**Decision claims:** Delete backing file after mount (kernel keeps inode alive via open reference). Hide loop device from sysfs via SUSFS if available. EROFS images also nuked after mount.

**Verification:**

1. **Deleting mounted file keeps inode alive: CONFIRMED (standard Linux behavior)**
   - This is fundamental Linux VFS behavior: `unlink()` on a file that has open file descriptors (including mount references) decrements the link count to 0 but the inode remains alive until all references are released
   - The mount holds a reference to the superblock which holds a reference to the backing block device, keeping the file data accessible
   - This is the same mechanism that allows `rm` of a file while a process has it open -- the file continues to exist in memory until the last fd/reference closes

2. **No Android-specific gotcha: CONFIRMED**
   - Both mountify and hybrid_mount rely on this exact behavior
   - Mountify (context-mountify.md section 5): "Post-Mount Cleanup: The overlay mounts survive because the kernel already resolved the directory tree into its internal data structures at mount time"
   - Mountify's ext4 sparse mode: "The sparse image file is deleted from disk"
   - hybrid_mount: `nuke_path()` deletes backing files after mount for both ext4 and EROFS images
   - No reports of this behavior breaking on Android kernels -- it's a core kernel guarantee

3. **Sysfs entry hiding:**
   - Mountify uses an LKM (`nuke.c`) that calls `ext4_unregister_sysfs(sb)` to remove `/proc/fs/ext4/<device>` entries, then self-unloads via `-EAGAIN`
   - hybrid_mount uses KSU's `NukeExt4Sysfs` API (wraps similar functionality in ksud)
   - ZeroMount's decision to use SUSFS for this is a different mechanism but targets the same goal

4. **EROFS nuke: CONFIRMED by hybrid_mount**
   - hybrid_mount section 7: "Nuke the image file path (hides from filesystem via NukeExt4Sysfs)" -- used for both ext4 and EROFS

**Verdict: FULLY CORRECT** -- Standard Linux VFS behavior, confirmed by both reference implementations. No Android-specific gotchas. The inode-alive-after-unlink guarantee is a kernel-level invariant.

---

## Summary Table

| Decision | Verdict | Key Findings |
|----------|---------|--------------|
| **ME01** (Storage cascade) | CORRECT with caveat | EROFS and ext4 confirmed. tmpfs xattr NOT in stock GKI defconfig -- effective cascade may skip tmpfs on many devices. Runtime probing handles this. |
| **ME02** (New mount API) | CORRECT | fsopen/fsmount/move_mount present in 5.10 kernel source. Legacy fallback essential for 4.14 edge cases. |
| **ME05** (BFS planner) | CORRECT | Current metamount.sh uses VFS (no overlay), so BFS is new. Pattern matches both mountify and hybrid_mount reference implementations. |
| **ME06** (SAR handling) | CORRECT | Both reference implementations handle legacy-symlink vs modern-separate-mount SAR layouts. |
| **ME08** (Whiteout formats) | FULLY CORRECT | All 3 formats + opaque dirs verified in metamount.sh:171-207. Line reference accurate. |
| **ME09** (Source="KSU") | PARTIALLY CORRECT | Source name is reasonable, but rationale is wrong: try_umount uses path registration, not source name matching. Source name matters for zygisk tools instead. |
| **ME11** (/mnt/ safety) | CORRECT | /mnt/ is writable tmpfs on Android, used by both reference projects. No SELinux issues at post-fs-data stage. |
| **ME12** (Nuke after mount) | FULLY CORRECT | Standard Linux VFS inode behavior. Confirmed by both reference implementations. No Android-specific issues. |

---

## Decisions Not Individually Verified (Covered by Cross-References)

| Decision | Status | Notes |
|----------|--------|-------|
| **ME03** (Magic mount fallback) | Design-level -- matches hybrid_mount's Phase 2 magic mount fallback | No code to verify against, but hybrid_mount proves the concept works |
| **ME04** (Per-module overlay-to-magic fallback) | Design-level -- matches hybrid_mount's executor auto-revert logic | hybrid_mount: "On failure -> move affected module IDs to magic_ids" |
| **ME07** (Atomic rename for sync) | Design-level -- matches hybrid_mount's `sync.rs` pattern | hybrid_mount: `.tmp_<id>` -> rename -> `.backup_<id>` (exactly as described) |
| **ME10** (KSU try_umount integration) | Design-level -- both mountify (via `ksu_susfs add_try_umount` / `ksud kernel umount add`) and hybrid_mount (via `umount_mgr.rs`) implement this | Pattern is well-established |

---

## Recommendations

1. **ME01:** Add a note that `CONFIG_TMPFS_XATTR` is NOT set in stock GKI defconfig. The cascade should be documented as effectively EROFS -> ext4 on stock GKI, with tmpfs as a bonus when OEMs enable xattr support.

2. **ME09:** Correct the rationale. KSU's `try_umount` works via explicit mount path registration (SUSFS `add_try_umount` or `ksud kernel umount add`), not source name matching. The source name matters for third-party zygisk unmount tools. Both mechanisms should be documented as separate.

3. **ME05:** Clarify that BFS is new functionality for the overlay fallback path. The current metamount.sh VFS approach injects per-file rules and doesn't need mount-point planning.
