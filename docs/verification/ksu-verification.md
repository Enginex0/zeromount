# KSU Platform Decisions Verification Report

> **Primary Analyst:** ksu-analyst-1
> **Cross-Validator:** ksu-analyst-2
> **Scope:** KSU01-KSU10 from DECISIONS.md
> **Reference Docs:**
> - `kernelsu-module-guide.md` (official KernelSU module + metamodule guide)
> - `kernelsu-module-config.md` (module configuration system)
> - `kernelsu-module-webui.md` (WebUI + API reference)
> - `kernelsu-additional-docs.md` (cross-manager comparison, APatch docs)
> - `METAMODULE_COMPLETE_GUIDE.md` (compiled metamodule reference)
> **Date:** 2026-02-08

---

## Verification Legend

- **CONFIRMED** -- Claim matches documentation exactly
- **PARTIALLY CORRECT** -- Core claim is right but details need correction
- **NEEDS CORRECTION** -- Claim contradicts documentation
- **UNVERIFIABLE** -- No documentation source available to confirm or deny

---

## KSU01: Target KernelSU + APatch

**Decision:** Metamodule mode on both. Not Magisk (no metamodule concept). APatch adopted KernelSU's metamodule system.

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-guide.md` lines 514-560: Metamodule system is a KernelSU invention. Magisk uses built-in magic mount with no plugin architecture.
- `kernelsu-additional-docs.md` section 15: "APatch has adopted the metamodule system from KernelSU." Lists available metamodules for APatch (overlayfs, mountify, magic mount, hybrid mount).
- `kernelsu-additional-docs.md` section 20 (Cross-Manager Comparison): Magisk mounting = "core built-in magic mount (bind mounts)", KernelSU = "metamodule system (pluggable)", APatch = "kernel OverlayFS + metamodule system (adopted from KernelSU)".

**No corrections needed.**

---

## KSU02: Root Manager Detection

**Decision:** Check `$KSU` and `$APATCH` environment variables. Filesystem fallback: `/data/adb/ksu/` or `/data/adb/ap/`. Rust binary abstracts behind a `RootManager` trait for path differences (BusyBox, SUSFS binary, config dirs).

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-guide.md` line 184: "You can use the environment variable `KSU` to determine if a script is running in KernelSU or Magisk. If running in KernelSU, this value will be set to `true`."
- `kernelsu-module-guide.md` line 279: "`KSU` (bool): a variable to mark that the script is running in the KernelSU environment, and the value of this variable will always be true."
- `kernelsu-additional-docs.md` section 14 (APatch): "`APATCH` (bool): always `true`"
- `kernelsu-additional-docs.md` section 20 (Cross-Manager Comparison): Confirms `KSU=true` for KernelSU, `APATCH=true` for APatch, and different BusyBox paths (`/data/adb/ksu/bin/busybox` vs `/data/adb/ap/bin/busybox`).
- `kernelsu-additional-docs.md` section 20: Provides a detect_root_manager() function confirming the pattern.

**Notes:**
- `$KSU` is a bool (`true`), not a path or version string. The decision correctly describes it as an env var check.
- Filesystem paths confirmed: KSU uses `/data/adb/ksu/`, APatch uses `/data/adb/ap/`.

---

## KSU03: Config Storage Abstraction

**Decision:** KernelSU has `ksud module config` (32 keys, 1MB values). APatch does not. The Rust binary uses file-based config (TOML) as the universal approach. `ksud module config` used only for `override.description` (KSU05) and `manage.kernel_umount` when on KSU.

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-config.md` lines 63-76: Validation limits confirmed:
  - Maximum key length: 256 bytes
  - Maximum value length: 1MB (1,048,576 bytes)
  - Maximum config entries: **32 per module**
  - Key format: `^[a-zA-Z][a-zA-Z0-9._-]+$`
- `kernelsu-additional-docs.md` section 20: "Module config: KernelSU has `ksud module config` built-in key-value store. Magisk/APatch do not have this -- use file-based config."
- `kernelsu-module-config.md` lines 99-148: Advanced features include `override.description` and `manage.<feature>` keys.

**Notes:**
- The "32 keys" claim maps to "32 entries per module" in the docs. CONFIRMED.
- The "1MB values" claim maps to "1MB (1048576 bytes)" max value length. CONFIRMED.
- APatch lacking `ksud module config` is CONFIRMED. File-based TOML is a sound universal approach.

---

## KSU04: No `manage.kernel_umount` Declaration

**Decision:** ZeroMount uses VFS redirection, not mounts. Declaring `kernel_umount` would cause ksud to try unmounting nonexistent mount points. In overlay fallback mode, ZeroMount manages its own try_umount registration (ME10) rather than delegating to KSU's automatic system.

**Verdict: PARTIALLY CORRECT -- mechanism description needs correction**

**Evidence:**
- `kernelsu-module-config.md` lines 117-148: `manage.kernel_umount` is a **module config key** (not a module.prop field). Setting it declares that the module is "managing" this KernelSU feature.
  - `ksud module config set manage.kernel_umount false` = module manages kernel unmount and DISABLES it
  - `ksud module config set manage.kernel_umount true` = module manages kernel unmount and ENABLES it
  - Deleting the key = module no longer controls this feature
- `kernelsu-additional-docs.md` section 7 (App Profile): "Umount Modules" is a per-app feature in KernelSU that unmounts overlays for specific apps. On kernel 5.10+, this is done natively in the kernel.

**Corrections needed:**
1. The decision says "Declaring kernel_umount would cause ksud to try unmounting nonexistent mount points." This is **imprecise**. `manage.kernel_umount` is a config key that controls whether KernelSU's per-app umount feature is enabled or disabled. Per `kernelsu-additional-docs.md` section 7 (App Profile), `kernel_umount` is specifically KSU's per-app module unmounting feature ("Umount Modules" in App Profiles). On kernel 5.10+, the kernel performs unmounting natively.
2. For VFS mode (no mounts exist): Not setting `manage.kernel_umount` is fine -- there's nothing to unmount. The decision's conclusion is correct even if the rationale is slightly off.
3. For overlay fallback mode: ME10 explicitly states "If SUSFS available, use `add_try_umount`. Otherwise use KSU native `kernel_umount` feature." This means ZeroMount WANTS KSU's native kernel_umount to apply to its overlay mounts (source="KSU" per ME09) when SUSFS is unavailable. NOT declaring `manage.kernel_umount` (letting KSU's default apply) is actually the CORRECT behavior -- it allows KSU's per-app unmount to work on ZeroMount's overlays for apps configured with "umount modules" in their profile.

**Cross-validated with ksu-analyst-2:** Both analysts independently reached the same conclusion. The decision's OUTCOME (don't declare manage.kernel_umount) is correct, but the RATIONALE needs rewriting. It should say: "We don't declare manage.kernel_umount because we want KSU's default per-app unmount behavior to apply to our overlay mounts in fallback mode. In VFS mode, there are no mounts, so kernel_umount is irrelevant."

**Recommendation:** Rewrite the rationale, not the conclusion. The decision outcome is correct for both VFS mode (irrelevant) and overlay fallback mode (desired behavior).

---

## KSU05: Dynamic Description via `override.description`

**Decision:** Update module.prop description after pipeline completion. Cross-platform (both KSU and APatch display module.prop description).

**Verdict: PARTIALLY CORRECT -- cross-platform claim is misleading**

**Evidence:**
- `kernelsu-module-config.md` lines 103-115: `ksud module config set override.description "text"` is the documented mechanism. CONFIRMED.
- `kernelsu-additional-docs.md` section 20: "Module config: KernelSU has ksud module config built-in key-value store. Magisk/APatch do not have this."
- `kernelsu-additional-docs.md` section 17 (APatch FAQ): "APatch WebUI implementation is completely the same as KernelSU" -- but this refers to WebUI, not the config system.

**Corrections needed:**
1. `override.description` is a KSU-only feature (uses `ksud module config`). APatch does NOT have this API.
2. The decision's claim that this is "Cross-platform (both KSU and APatch display module.prop description)" conflates two things:
   - Both platforms display the `description` field from `module.prop` -- TRUE
   - Both platforms support `override.description` -- FALSE (KSU only)
3. For APatch, the fallback must be direct `sed` modification of `module.prop` or some other mechanism.

**Recommendation:** Clarify that `override.description` is KSU-specific. On APatch, fall back to direct module.prop modification. The decision should read: "On KSU, use `ksud module config set override.description`. On APatch, modify `module.prop` directly."

---

## KSU06: Thin `metamount.sh` Launcher

**Decision:** Under 30 lines. Detects architecture, selects correct binary, executes `zeromount mount`, handles bootloop counter, calls `ksud kernel notify-module-mounted` on success. All logic from the current 427-line `metamount.sh` moves into the Rust binary's `mount` subcommand.

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-guide.md` lines 672-724: metamount.sh is the mount handler hook. It receives `MODDIR` and standard KernelSU env vars. Its job is to mount all enabled modules.
- `METAMODULE_COMPLETE_GUIDE.md` lines 795-811: hybrid_mount example shows the exact pattern -- a thin shell launcher that calls a Rust binary then notifies:
  ```bash
  "$MODDIR/meta-hybrid"  # Run the Rust binary
  /data/adb/ksud kernel notify-module-mounted
  ```
- `METAMODULE_COMPLETE_GUIDE.md` lines 826-834: "The 5 Things Every Metamodule Must Do" -- #5 is "Call `/data/adb/ksud kernel notify-module-mounted` when done."

**Boot lifecycle confirmed:**
- `kernelsu-module-guide.md` lines 789-812: metamount.sh runs at step 6 of post-fs-data stage:
  1. Common post-fs-data.d scripts
  2. Prune modules, restorecon, load sepolicy.rule
  3. Metamodule's post-fs-data.sh
  4. Regular modules' post-fs-data.sh
  5. Load system.prop
  6. **Metamodule's metamount.sh** <-- HERE
  7. post-mount.d stage

**Notes:**
- `ksud kernel notify-module-mounted` is NOT mentioned in the official `kernelsu-module-guide.md` but IS listed as critical in `METAMODULE_COMPLETE_GUIDE.md` (which is a compiled reference). The official guide implies metamount.sh completes and then post-mount.d runs, suggesting ksud tracks the script exit. The explicit `notify-module-mounted` call may be a best practice from meta-overlayfs/hybrid_mount implementations rather than a strict API requirement. Recommend keeping the call as a safety measure.
- **Documentation emphasis note (cross-validated with ksu-analyst-2):** The METAMODULE_COMPLETE_GUIDE marks `notify-module-mounted` as THE most critical requirement (section 10: "If you forget this, KernelSU doesn't know mounting is complete!"). The decision captures this call but treats it as an afterthought in the description. Given its criticality, it deserves more prominent emphasis in the decision text.
- The thin launcher pattern matches hybrid_mount's approach exactly. Good design.
- **Architectural validation:** METAMODULE_COMPLETE_GUIDE section 20 describes "NoMount" as attempting exactly what ZeroMount does (VFS-based metamodule, no visible mounts, kernel-level redirection) but "got overcomplicated with 1500+ lines of shell scripts." The Rust rewrite (R01) directly solves this validated problem.

---

## KSU07: `metainstall.sh` -- Partition Normalization at Install

**Decision:** Detect which partitions exist on the device at install time. Write `partitions.conf` for the Rust binary.

**Verdict: CONFIRMED (valid use of the hook)**

**Evidence:**
- `kernelsu-module-guide.md` lines 726-752: metainstall.sh is sourced during module installation, after files are extracted. It inherits: `MODPATH`, `TMPDIR`, `ZIPFILE`, `ARCH`, `API`, `IS64BIT`, `KSU`, `KSU_VER`, `KSU_VER_CODE`, `BOOTMODE`, and functions `ui_print`, `abort`, `set_perm`, `set_perm_recursive`, `install_module`.
- `kernelsu-module-guide.md` line 752: "This script is NOT called when installing the metamodule itself."

**Notes:**
- metainstall.sh's purpose per docs is to "Customize how regular modules are installed." Using it for partition detection at install time is a creative but valid use.
- Important caveat: metainstall.sh is called when OTHER modules are installed, not when the metamodule itself is installed. For ZeroMount's own install-time partition detection, use `customize.sh` instead. The decision may be confusing these two hooks.
- **Cross-validated detail (ksu-analyst-2):** metainstall.sh is SOURCED (not executed) and has access to the `install_module` function (`kernelsu-module-guide.md` line 743) which must be called to trigger the built-in installation process. Partition detection would run AFTER `install_module` completes, so module files are already in place. This is a valid sequencing for the "detect partitions when other modules install" scenario.

**Correction needed:** If the intent is to detect partitions when ZeroMount ITSELF is installed, this should happen in `customize.sh`, not `metainstall.sh`. `metainstall.sh` runs when OTHER modules are installed through ZeroMount. Clarify which scenario is intended.

---

## KSU08: `metauninstall.sh` -- Cleanup

**Decision:** Clear VFS rules, disable engine, remove `/data/adb/zeromount/` data directory, clean SUSFS entries tagged `[ZeroMount]`. Current 17-line script is appropriate with SUSFS cleanup addition.

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-guide.md` lines 754-783: metauninstall.sh runs during module uninstallation, before the module directory is removed. It receives `MODULE_ID`.
- `METAMODULE_COMPLETE_GUIDE.md` lines 278-300: Confirms purpose is to "Clean up resources when regular modules are uninstalled."

**Notes:**
- Again, note the distinction: `metauninstall.sh` is called when OTHER modules are uninstalled. For cleaning up when ZeroMount ITSELF is uninstalled, use `uninstall.sh`. The decision's cleanup actions (VFS rules, engine, SUSFS entries) make more sense in `uninstall.sh`.
- The 17-line script size is reasonable for cleanup operations.

---

## KSU09: `notify-module-mounted` After Full Pipeline

**Decision:** Call AFTER: rules injected, engine enabled, SUSFS applied, kstat pass complete, module description updated. Fixes BUG-M3 race.

**Verdict: CONFIRMED**

**Evidence:**
- `METAMODULE_COMPLETE_GUIDE.md` lines 668-678: Section 10 "The notify-module-mounted command (CRITICAL!)":
  ```
  After mounting is done, you MUST run:
  /data/adb/ksud kernel notify-module-mounted
  This tells KernelSU: "Hey, I finished mounting everything. You can continue booting now."
  ```
- `METAMODULE_COMPLETE_GUIDE.md` lines 826-832: Summary lists it as one of "The 5 Things Every Metamodule Must Do."
- The exact API call is: `/data/adb/ksud kernel notify-module-mounted` (note the `kernel` subcommand).

**Notes:**
- The decision's ordering (rules -> SUSFS -> enable -> refresh -> description -> notify) is a ZeroMount-specific pipeline. The docs just say "call it when mounting is done" without prescribing internal pipeline order. ZeroMount's ordering is reasonable and fixes BUG-M3.
- The `notify-module-mounted` call is documented in the compiled guide but not in the official kernelsu.org metamodule page. It may be implicitly handled by ksud tracking metamount.sh exit, but the explicit call is safer.

---

## KSU10: `post-fs-data.sh` for Detection, `metamount.sh` for Mounting

**Decision:** Split: `post-fs-data.sh` runs the Rust binary's `detect` subcommand (kernel probe, SUSFS probe, writes detection result JSON). `metamount.sh` reads detection result and runs the `mount` pipeline. Separates lightweight probing from heavy I/O.

**Verdict: CONFIRMED**

**Evidence:**
- `kernelsu-module-guide.md` lines 789-812: Boot execution order confirms:
  - Step 3: Metamodule's `post-fs-data.sh` executes
  - Step 4: Regular modules' `post-fs-data.sh` execute
  - Step 5: Load system.prop
  - Step 6: Metamodule's `metamount.sh` executes
- `kernelsu-module-guide.md` lines 327-333: post-fs-data mode:
  - BLOCKING with 10-second timeout
  - Runs before any modules are mounted
  - Runs before Zygote starts
  - Using `setprop` will deadlock -- use `resetprop -n` instead

**Notes:**
- The split is architecturally sound. post-fs-data.sh runs before metamount.sh with system.prop loading in between.
- CRITICAL CAVEAT: post-fs-data has a **10-second timeout** and is BLOCKING. The `detect` subcommand must complete well within this window. Kernel probe + SUSFS probe should be fast (file existence checks + ioctl calls), but writing JSON output should be minimal.
- The 10-second timeout is shared with ALL post-fs-data.sh scripts (metamodule + regular modules). ZeroMount's detect should target <2 seconds to leave room for other modules.

---

## Summary Table

| Decision | Verdict | Key Finding |
|----------|---------|-------------|
| KSU01 | CONFIRMED | Correct -- Magisk has no metamodule, APatch adopted KSU's system |
| KSU02 | CONFIRMED | `$KSU=true`, `$APATCH=true`, filesystem fallback paths correct |
| KSU03 | CONFIRMED | 32 entries, 1MB values, APatch lacks ksud config -- all verified |
| KSU04 | PARTIALLY CORRECT | Conclusion right (don't declare), but rationale wrong. `manage.kernel_umount` is a per-app umount config key. Not declaring lets KSU's default apply to overlay mounts in fallback mode, which ME10 explicitly wants |
| KSU05 | PARTIALLY CORRECT | `override.description` is KSU-only. APatch does NOT have this API. Cross-platform claim misleading |
| KSU06 | CONFIRMED | Thin launcher pattern matches hybrid_mount. `ksud kernel notify-module-mounted` confirmed |
| KSU07 | CONFIRMED (with caveat) | Valid hook, but metainstall.sh runs for OTHER module installs, not ZeroMount's own install. May need `customize.sh` for self-install partition detection |
| KSU08 | CONFIRMED (with caveat) | Valid hook, but metauninstall.sh runs for OTHER module uninstalls. ZeroMount self-cleanup should use `uninstall.sh` |
| KSU09 | CONFIRMED | API is `/data/adb/ksud kernel notify-module-mounted`. Pipeline ordering is sound |
| KSU10 | CONFIRMED | Boot order validates the split. 10-second timeout on post-fs-data is critical constraint |

---

## Actionable Corrections

1. **KSU04:** Rewrite the rationale (not the conclusion). The decision should say: "We don't declare manage.kernel_umount because we want KSU's default per-app unmount behavior to apply to our overlay mounts in fallback mode. In VFS mode, there are no mounts, so kernel_umount is irrelevant." Remove the incorrect claim about "causing ksud to try unmounting nonexistent mount points." (Cross-validated: both analysts agree the outcome is correct, only the rationale needs fixing.)

2. **KSU05:** Add platform-specific notes: `override.description` via `ksud module config` on KSU; direct `module.prop` modification on APatch. Remove the "cross-platform" claim for override.description itself. The `RootManager` trait from KSU02 is the right abstraction point for this platform difference.

3. **KSU06:** Elevate the emphasis on `notify-module-mounted`. The METAMODULE_COMPLETE_GUIDE marks it as THE most critical requirement. The decision text currently treats it as an afterthought.

4. **KSU07:** Clarify which install scenario is intended. If ZeroMount's own install -- use `customize.sh`. If detecting partitions when other modules install through ZeroMount -- `metainstall.sh` is correct.

5. **KSU08:** Same clarification. Self-cleanup = `uninstall.sh`. Other-module-cleanup = `metauninstall.sh`.

6. **KSU10:** Add explicit note about the 10-second post-fs-data timeout. Recommend detect subcommand targets <2 seconds.

---

## Missing Decisions (Gaps Identified)

The following KernelSU hooks and features are available but not addressed by any KSU decision:

1. **post-mount.sh hook:** Runs after metamount.sh completes (step 7 of boot order). ZeroMount could use this for:
   - Post-mount verification (confirm all overlays are correctly applied)
   - Status cache generation
   - Deferred SUSFS operations that require mounts to be in place

2. **boot-completed.sh hook:** Runs after ACTION_BOOT_COMPLETED. ZeroMount could use this for:
   - WebUI state initialization
   - Non-critical status reporting
   - Deferred tasks that don't need to run during boot

3. **service.sh usage:** Mentioned in B04 (module ZIP contents) but not discussed as a design decision. The monitor/status polling from v1's `monitor.sh` could use this stage (NON-BLOCKING, runs during boot animation).

4. **KSU_MODULE environment variable:** Available in all module scripts, set to the module ID. Could simplify `ksud module config` calls (no need to specify module ID explicitly).

5. **APatch ARM64-only constraint:** `kernelsu-additional-docs.md` section 16 (line 841) states APatch is "ARM64 architecture only." B02 builds four ABIs (arm64, arm, x86_64, x86). The extra ABIs (arm, x86_64, x86) are only relevant for KernelSU (emulators, Chromebooks). Not a problem, but implementation should note that APatch testing is ARM64-only.

**Recommendation:** Add a KSU11 decision covering lifecycle hook allocation: which Rust binary subcommands map to which boot stage scripts.

---

## Cross-Validation Notes

This report was cross-validated with ksu-analyst-2. Key agreements:
- Both analysts independently identified the KSU04 rationale issue
- Both analysts independently confirmed the `notify-module-mounted` documentation gap (official guide vs compiled guide)
- ksu-analyst-2 identified the METAMODULE_COMPLETE_GUIDE section 20 (NoMount) as architectural validation of ZeroMount's approach
- ksu-analyst-2 flagged the missing post-mount.sh/boot-completed.sh decisions as a gap
- No contradictions between independent analyses

---

## Cross-Validation by ksu-analyst-2

> **Validator:** ksu-analyst-2
> **Date:** 2026-02-08
> **Method:** Independent analysis of the same reference docs, then cross-validation of ksu-analyst-1's findings

### Alignment Summary

All findings independently confirmed. No contradictions between analyst-1 and analyst-2 analyses. Both analysts independently flagged the same issues with KSU04 rationale and KSU05 cross-platform claim before comparing notes.

### Confirmed Findings (no changes needed)

- **KSU01, KSU02, KSU03, KSU06, KSU09, KSU10**: Full agreement with analyst-1's CONFIRMED verdicts. Evidence citations verified against source documents.
- **KSU04**: Full agreement on PARTIALLY CORRECT verdict. Both analysts independently identified that `manage.kernel_umount` is a config key mechanism, not a mount trigger. Analyst-2 adds: per `kernelsu-additional-docs.md` section 7 (App Profile, lines 574-580), `kernel_umount` is specifically KSU's per-app "Umount Modules" feature that unmounts overlays for apps configured in App Profiles. On kernel 5.10+, this happens natively in the kernel. This further clarifies the scenario-dependent recommendation: in VFS mode no mounts exist so the feature is moot; in overlay fallback mode, ME10 says "use KSU native kernel_umount feature" which implies ZeroMount WANTS the default behavior to apply, making NOT declaring `manage.kernel_umount` potentially correct -- but for a different reason than stated.
- **KSU05**: Full agreement on PARTIALLY CORRECT verdict. Confirmed via `kernelsu-additional-docs.md` line 1060.
- **KSU07, KSU08**: Agree with caveats about self-install vs other-module-install distinction.

### Additional Findings (not in analyst-1's report)

#### 1. Unused Boot Stage Hooks -- Missing Decision Item

Neither `post-mount.sh` nor `boot-completed.sh` appear anywhere in DECISIONS.md (grep confirmed zero matches). Both hooks are available to metamodules per the boot execution order:

- **`post-mount.sh`**: Runs AFTER metamount.sh completes (boot step 7). Potential uses for ZeroMount:
  - Post-mount verification (confirm overlays mounted correctly)
  - Status cache generation (`.status_cache.json`)
  - Operations requiring mounts to be in place before Zygote starts
- **`boot-completed.sh`**: Runs after ACTION_BOOT_COMPLETED. Potential uses:
  - inotify watcher startup (DET04 currently implies service.sh but doesn't specify)
  - Deferred status reporting
  - WebUI state initialization

**Recommendation:** Add a new decision (e.g., KSU11) documenting which boot stage hooks are used and why. Current implicit mapping:
- `post-fs-data.sh` -- detection (KSU10)
- `metamount.sh` -- mounting pipeline (KSU06)
- `service.sh` -- mentioned in B04 ZIP contents but no design decision
- `post-mount.sh` -- not used (should document why or plan usage)
- `boot-completed.sh` -- not used (should document why or plan usage)

#### 2. APatch ARM64-Only Constraint

`kernelsu-additional-docs.md` section 16 (line 841): APatch supports "ARM64 architecture only." Decision B02 builds four ABIs (arm64, arm, x86_64, x86). The additional ABIs serve KernelSU users (emulators, Chromebooks) and are not wasted effort, but the verification report should note this platform difference. On APatch, only the `zm-arm64` binary is relevant.

#### 3. NoMount Architectural Validation

`METAMODULE_COMPLETE_GUIDE.md` section 20 (lines 815-824) describes "NoMount" as a metamodule that "doesn't use visible mounts at all, uses a custom VFS driver, redirects file access at the kernel level" but "got overcomplicated with 1500+ lines of shell scripts." This is essentially ZeroMount v1. The Rust rewrite (R01) directly addresses this identified problem. This provides external validation that the metamodule guide authors recognized VFS-based mounting as a valid strategy, and that shell-based implementation was the bottleneck -- not the approach itself.

#### 4. KSU03 Minor Clarification

The decision text says `manage.kernel_umount` is used "when on KSU" -- but KSU04 says NOT to declare it. These two decisions appear to contradict each other on whether `manage.kernel_umount` is used. KSU03 should be updated to remove the `manage.kernel_umount` reference if KSU04's conclusion (don't declare) holds, OR both should be updated to reflect the scenario-dependent approach (declare with `false` value in overlay fallback mode only).

### Final Verdict

The KSU decision set is **sound overall**. The 10 decisions correctly map ZeroMount's metamodule architecture to KernelSU's platform APIs. The two PARTIALLY CORRECT items (KSU04, KSU05) need rationale/detail corrections but don't change the implementation direction. The missing hook utilization (post-mount.sh, boot-completed.sh) is a gap worth addressing but not blocking.
