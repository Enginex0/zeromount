# ZeroMount Decision Matrix — Final Synthesis

## Identity

| Item | Value |
|------|-------|
| Project | ZeroMount |
| Module ID | zeromount |
| Binary | zm |
| Kernel config | CONFIG_ZEROMOUNT |
| Data dir | /data/adb/zeromount |
| Device | /dev/zeromount |
| Ioctl magic | current 'N' (0x4E) → v2 target 'Z' (0x5A) |

---

## Layer 1: Kernel Patch — Bugs to Fix

| # | Bug | Severity | Location | Fix |
|---|-----|----------|----------|-----|
| K1 | RCU use-after-free in UID deletion | CRITICAL | nomount.c:737 | Use call_rcu() instead of kfree() |
| K2 | Compat getdents return value | MEDIUM | readdir.c:890 | Change buf.count to count |
| K3 | Inode collision — no dev check | MEDIUM | nomount.c:228,279 | Compare (ino, dev) pair |
| K4 | O(n) scan on permission check | PERF | nomount.c:279 | Add inode-keyed hash table |
| K5 | O(n) scan on d_path | PERF | nomount.c:227 | Same inode-keyed hash table |
| K6 | Static hash tables in header | WASTE | nomount.h:972-977 | Move to .c, use extern in .h |
| K7 | Overly broad permission bypass | SECURITY | namei.c:361-370 | Only bypass needed mask |
| K8 | No SELinux context handling | DETECTION | — | DECIDED: Add xattr hook patch (see Decision #7 in DECISIONS.md) |
| K9 | EXPORT_SYMBOL on kill switch | DETECTION | nomount.c | Remove export or add access control |
| K10 | Rename all identifiers | IDENTITY | All files | nomount → zeromount, magic N → Z |
| K11 | Add enable/disable ioctls | FEATURE | nomount.c | New IOC commands 8 and 9 |
| K12 | Add runtime logging toggle | FEATURE | New file | ZM_LOG() macro + sysfs at /sys/kernel/zeromount/debug |

---

## Layer 2: zm Binary (src/zm.c) — Bugs to Fix

| # | Bug | Severity | Location | Fix |
|---|-----|----------|----------|-----|
| B1 | FD leak — never closes /dev/zeromount | CRITICAL | nm.c:146 | Add sys1(SYS_CLOSE, fd) before exit |
| B2 | Stack buffer overflow in path resolution | HIGH | nm.c:186 | Add bounds check: cwd + src < PATH_MAX |
| B3 | STAT_MODE_IDX value is correct (idx 4 = byte offset 16 = st_mode on aarch64) | VERIFIED | nm.c:14 | No fix needed — previously misidentified as bug |
| B4 | UID parsing overflow | MEDIUM | nm.c:213-214 | Add overflow check and digit validation |
| B5 | Version display limited to single digit | LOW | nm.c:233 | Use itoa or multi-char output |
| B6 | No error output | MEDIUM | All | Add print_error() helper using SYS_WRITE to stderr |
| B7 | Silent failure on bad argc | LOW | nm.c:161 | Print usage on bad argc |
| B8 | Unknown command silently exits 0 | LOW | nm.c dispatch | Default case should exit 1 with error |
| B9 | Rename device path | IDENTITY | nm.c:146 | /dev/nomount → /dev/zeromount |
| B10 | Recalculate ioctl codes | IDENTITY | nm.c:122-128 | Magic 'N'→'Z': all codes change |
| B11 | Add enable/disable commands | FEATURE | New code | 'e' = enable, 'x' = disable (avoid 'd' conflict with del) |

---

## Layer 3: Metamodule Scripts — Fixes Required

| # | Issue | Severity | File | Fix |
|---|-------|----------|------|-----|
| S1 | Missing notify-module-mounted | CRITICAL | metamount.sh | Add: /data/adb/ksud kernel notify-module-mounted |
| S2 | Missing skip_mount flag check | CRITICAL | metamount.sh | Add: [ -f "$mod_path/skip_mount" ] && continue |
| S3 | Missing metauninstall.sh | MEDIUM | New file | Create cleanup script (~12 lines) |
| S4 | Empty versionCode in module.prop | LOW | module.prop | Set to 1 |
| S5 | Non-standard 'remove' flag check | LOW | metamount.sh:35 | Remove, only check 'disable' |
| S6 | No APEX partition in TARGET_PARTITIONS | LOW | metamount.sh:7 | Add 'apex' to list |
| S7 | Rename all identifiers | IDENTITY | All scripts | nomount→zeromount, nm→zm |
| S8 | monitor.sh race condition | LOW | monitor.sh | Add sleep 1 before sed |
| S9 | service.sh no error checking on UID block | LOW | service.sh | Add error logging |

---

## Layer 4: SUSFS Integration — Keep/Discard/Simplify

### KEEP (Valuable patterns for ZeroMount)

| Function | Purpose | Target File |
|----------|---------|-------------|
| susfs_init() | Detect SUSFS binary + capabilities | susfs_integration.sh |
| susfs_classify_path() | Route file types to correct SUSFS actions | susfs_integration.sh |
| susfs_apply_kstat() with fallback | kstat_redirect → kstat_statically cascade | susfs_integration.sh |
| susfs_apply_path() | Hide paths from detection | susfs_integration.sh |
| susfs_apply_maps() | Hide .so from /proc/maps | susfs_integration.sh |
| apply_font_redirect() | open_redirect_all + kstat for fonts | susfs_integration.sh |
| Device ID override from parent | Prevent stat() device mismatch detection | susfs_apply_kstat() |
| Device hiding pattern | Hide /dev/zeromount + sysfs entries | service.sh |
| nm_register_rule_with_susfs() | Main orchestration API | susfs_integration.sh |

### DISCARD (Bloat from old architecture)

| Component | Lines | Why Discard |
|-----------|-------|-------------|
| Universal mount hijacker | ~200 | ZeroMount IS the metamodule, no mounts to hijack |
| Overlay engine integration | ~150 | No overlays in ZeroMount |
| Rule cache system | ~65 | Premature optimization |
| 4-phase enable/disable dance | ~80 | Wrong execution timing workaround |
| Config file system | ~25 | Deterministic > configurable |
| Monitor daemon spawning | ~10 | Boot stages catch all changes |
| Excessive logging (5 levels + enter/exit) | ~250 | Simplify to 3 levels |
| Legacy compatibility wrappers | ~20 | Dead code |

### SIMPLIFY (Keep core, remove bloat)

| Function | Change | Reason |
|----------|--------|--------|
| susfs_apply_path() deferral | Remove defer if no overlays at Step 6 | Verify overlay timing |
| susfs_clean_module_entries() | Keep core, remove tracking | Simpler cleanup |
| register_module_vfs() | Keep VFS loop, remove overlay decisions | Always VFS |

---

## Size Targets

| Component | Old Lines | ZeroMount Target |
|-----------|-----------|------------------|
| Kernel patch (zeromount.c) | 643 | ~700 (with bug fixes + new features) |
| Kernel header (zeromount.h) | 97 | ~80 (move statics to .c) |
| zm.c binary | 247 | ~300 (with enable/disable + error output) |
| metamount.sh | 69 | ~75 (with contract compliance fixes) |
| service.sh | 6 | ~30 (with SUSFS + UID exclusions) |
| susfs_integration.sh | 1335 | ~200 (slim version) |
| customize.sh | 40 | ~40 (rename only) |
| metainstall.sh | 3 | ~5 (minimal) |
| metauninstall.sh | 0 | ~12 (new) |
| monitor.sh | 15 | ~18 (with race fix) |
| module.prop | 6 | 6 |
| **TOTAL** | **2461** | **~1466 (40% reduction)** |

---

## Build Order (Dependency Chain)

```
1. kernel-patch-v2 (K1-K12)
   ├── Fixes: RCU, compat, inode, performance
   ├── Rename: zeromount identity
   ├── New: enable/disable ioctls
   └── New: ZM_LOG() runtime logging

2. zm-binary (B1-B11) [depends on: kernel-patch-v2]
   ├── Fixes: FD leak, stack overflow, stat index, UID parse
   ├── Rename: zeromount identity, new ioctl codes
   └── New: enable/disable commands, error output

3. metamodule-scripts (S1-S9) [depends on: zm-binary]
   ├── Fixes: notify-module-mounted, skip_mount
   ├── Rename: zeromount identity
   └── New: metauninstall.sh

4. susfs-integration [depends on: metamodule-scripts]
   ├── Slim: 1335 → 200 lines
   └── Keep: init, classify, kstat, maps, font redirect

5. build-and-test [depends on: all above]
   └── GitHub Actions + device testing
```

---

## Open Question

**Overlay mount timing at Step 6:** Does KernelSU have overlays mounted at metamount.sh execution time (Step 6)? If NO overlays → sus_path deferred logic can be deleted entirely. If YES → keep simplified deferral. This determines ~150 lines of code.
