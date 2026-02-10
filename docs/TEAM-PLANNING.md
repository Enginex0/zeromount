# ZeroMount v2 -- Team Planning Document

> **For:** 12 implementation agents (6 pairs)
> **Date:** 2026-02-08
> **Status:** Implementation complete — serves as single source of truth for the codebase

---

## 1. Project Overview

ZeroMount v2 is a Rust-based KernelSU/APatch metamodule that replaces ~2500 lines of shell scripts with a single binary. It mounts modules via VFS redirection when kernel patches are present, falling back to OverlayFS or magic mount when they're not. The binary uses a typestate pipeline pattern to enforce operation ordering at compile time, eliminating race conditions from the shell implementation. A 5-scenario detection engine (Full, SusfsFrontend, KernelOnly, SusfsOnly, None) drives the mount strategy cascade. SUSFS integration moves from a maintained fork to 10 build-time injection patches on upstream. An ext4 sysfs nuke (dual-path: ksud or one-shot LKM) removes loop-mount evidence from /proc/fs/ext4/. A unified verbose toggle syncs kernel sysfs, userspace marker, and config.toml in one operation. A SolidJS WebUI provides device management through the KSU WebView. 81 architecture decisions are locked in DECISIONS.md. All 27 features are implemented.

---

## 2. Team Structure

| Pair | Agents | Domain | Primary Focus |
|------|--------|--------|---------------|
| **1-2** | Rust Core | Core infrastructure | Binary scaffold, CLI, config, logging, pipeline controller, process camouflage |
| **3-4** | VFS Engine | VFS driver interface | /dev/zeromount ioctl interface, VFS mount executor |
| **5-6** | Mount Engine | Module mounting | Module scanner, BFS planner, OverlayFS, magic mount, storage cascade |
| **7-8** | SUSFS Client | SUSFS integration | SUSFS supercall interface, custom commands, font redirect, BRENE features |
| **9-10** | WebUI | Frontend | Dead code cleanup, scenario display, settings/BRENE toggles, glass toggle |
| **11-12** | Detection + Platform | Platform + CI | Detection engine, KSU/APatch abstraction, shell launchers, CI, ZIP packaging |

Within each pair: one agent implements, the other validates. Both read the same docs and cross-check.

---

## 3. Document Map

### Every agent MUST read:

| Document | Path | What It Contains |
|----------|------|------------------|
| DECISIONS.md | `/home/claudetest/metamodule-experiment/DECISIONS.md` | All 81 architecture decisions with rationale and status |
| DESIGN.md | `/home/claudetest/metamodule-experiment/docs/DESIGN.md` | Component architecture, file structure, data flow, CLI interface |
| GOAL.md | `/home/claudetest/metamodule-experiment/docs/GOAL.md` | Success criteria, scope boundaries |
| features.json | `/home/claudetest/metamodule-experiment/.claude/features.json` | Feature backlog with dependencies and acceptance criteria |
| OUTSTANDING-ISSUES.md | `/home/claudetest/metamodule-experiment/docs/OUTSTANDING-ISSUES.md` | Post-MVP issues, remaining work items |

### Per-pair additional reading:

| Pair | Required Additional Documents |
|------|------------------------------|
| 1-2 | CONTEXT.md sections 4-5 (binary, scripts) |
| 3-4 | CONTEXT.md section 3 (kernel implementation); `/home/claudetest/metamodule-experiment/docs/verification/kernel-verification.md` |
| 5-6 | `/home/claudetest/metamodule-experiment/docs/verification/mount-verification.md`; CONTEXT.md section 5.2 (boot sequence); `nuke_ext4_lkm/` (LKM source) |
| 7-8 | `/home/claudetest/metamodule-experiment/docs/verification/susfs-verification.md`; CONTEXT.md sections 3.6-3.7 (SUSFS coupling) |
| 9-10 | `/home/claudetest/metamodule-experiment/docs/verification/webui-verification.md`; CONTEXT.md sections 6, 9 (WebUI, dead code) |
| 11-12 | `/home/claudetest/metamodule-experiment/docs/verification/ksu-verification.md`; CONTEXT.md sections 5.2-5.3 (boot sequence, dependencies); `patches/susfs/` (injection scripts); `scripts/package.sh` |

---

## 4. Shared Types Contract

Pair 1-2 MUST define these types first. All other pairs depend on them.

### Scenario Enum
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    Full,           // /dev/zeromount present + full SUSFS capabilities
    SusfsFrontend,  // /dev/zeromount present + partial SUSFS capabilities
    KernelOnly,     // /dev/zeromount present + no SUSFS binary
    SusfsOnly,      // No /dev/zeromount -- SUSFS available without VFS driver
    None,           // No /dev/zeromount -- use OverlayFS/magic mount
}
```

### CapabilityFlags
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityFlags {
    pub vfs_driver: bool,
    pub vfs_version: Option<u32>,
    pub vfs_status_ioctl: bool,     // GET_STATUS (0x80045A0B) available
    pub susfs_available: bool,
    pub susfs_version: Option<String>,
    pub susfs_kstat: bool,
    pub susfs_path: bool,
    pub susfs_maps: bool,
    pub susfs_open_redirect: bool,
    pub susfs_kstat_redirect: bool,     // custom 0x55573
    pub susfs_open_redirect_all: bool,  // custom 0x555c1
    pub overlay_supported: bool,
    pub erofs_supported: bool,
    pub tmpfs_xattr: bool,
}
```

### MountPlan and MountResult
```rust
pub struct MountPlan {
    pub scenario: Scenario,
    pub modules: Vec<PlannedModule>,
    pub partition_mounts: Vec<PartitionMount>,
}

pub struct MountResult {
    pub module_id: String,
    pub strategy_used: MountStrategy,
    pub success: bool,
    pub rules_applied: u32,
    pub rules_failed: u32,
    pub error: Option<String>,
    pub mount_paths: Vec<String>,
}

pub enum MountStrategy { Vfs, Overlay, MagicMount }
```

### ScannedModule
```rust
pub struct ScannedModule {
    pub id: String,
    pub path: PathBuf,
    pub files: Vec<ModuleFile>,
    pub has_service_sh: bool,
    pub has_post_fs_data_sh: bool,
    pub prop: ModuleProp,
}

pub enum ModuleFileType {
    Regular, Directory, Symlink,
    WhiteoutCharDev,   // mknod c 0 0
    WhiteoutXattr,     // trusted.overlay.whiteout=y
    WhiteoutAufs,      // .wh.* prefix
    OpaqueDir,         // trusted.overlay.opaque=y
    RedirectXattr,     // trusted.overlay.redirect
}
```

### RuntimeState (status JSON)
```rust
pub struct RuntimeState {
    pub scenario: Scenario,
    pub capabilities: CapabilityFlags,
    pub engine_active: Option<bool>,
    pub driver_version: Option<u32>,
    pub rule_count: u32,
    pub excluded_uid_count: u32,
    pub hidden_path_count: u32,
    pub susfs_version: Option<String>,
    pub modules: Vec<ModuleStatus>,
    pub timestamp: u64,
    pub degraded: bool,
    pub degradation_reason: Option<String>,
}
```

### SusfsCommand Enum
```rust
#[repr(u32)]
pub enum SusfsCommand {
    AddSusPath = 0x55550,
    SetAndroidDataRootPath = 0x55551,
    SetSdcardRootPath = 0x55552,
    AddSusPathLoop = 0x55553,
    HideSusMntsForNonSuProcs = 0x55561, // mount hiding (EXCLUDED per S05)
    AddSusKstat = 0x55570,
    UpdateSusKstat = 0x55571,
    AddSusKstatStatically = 0x55572,
    AddSusKstatRedirect = 0x55573,      // custom
    SetUname = 0x55590,
    EnableLog = 0x555a0,
    SetCmdline = 0x555b0,
    AddOpenRedirect = 0x555c0,
    AddOpenRedirectAll = 0x555c1,        // custom, NO CLI handler in upstream
    ShowVersion = 0x555e1,
    ShowEnabledFeatures = 0x555e2,
    ShowVariant = 0x555e3,
    EnableAvcLogSpoofing = 0x60010,
    AddSusMap = 0x60020,
}
```

### DetectionResult (serialized between detect and mount)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionResult {
    pub scenario: Scenario,
    pub capabilities: CapabilityFlags,
    pub driver_version: Option<u32>,
    pub timestamp: u64,
}
```

### Supporting Types (referenced above)
```rust
pub struct PlannedModule {
    pub id: String,
    pub source_path: PathBuf,
    pub target_partitions: Vec<String>,
    pub file_count: usize,
}

pub struct PartitionMount {
    pub partition: String,
    pub mount_point: PathBuf,       // e.g., /system/bin (never /system)
    pub contributing_modules: Vec<String>,
}

pub struct ModuleFile {
    pub relative_path: PathBuf,     // relative to module root, e.g., system/bin/foo
    pub file_type: ModuleFileType,
    pub source_module: String,
}

pub struct ModuleProp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub version_code: u32,
    pub author: String,
    pub description: String,
}

pub struct ModuleStatus {
    pub id: String,
    pub strategy: MountStrategy,
    pub rules_applied: u32,
    pub rules_failed: u32,
    pub errors: Vec<String>,
    pub mount_paths: Vec<String>,
}
```

### RootManager Trait
```rust
pub trait RootManager {
    fn name(&self) -> &str;                    // "KernelSU" or "APatch"
    fn base_dir(&self) -> &Path;               // /data/adb/ksu/ or /data/adb/ap/
    fn busybox_path(&self) -> PathBuf;
    fn susfs_binary_paths(&self) -> Vec<PathBuf>;
    fn update_description(&self, text: &str) -> Result<()>;
    fn notify_module_mounted(&self) -> Result<()>;
}
```

---

## 5. Feature Backlog

Features defined in `.claude/features.json`. Ordered by phase:

| Phase | Features | Description | Gate | Status |
|-------|----------|-------------|------|--------|
| **1: Foundation** | F01, F02, F03, F04 | Scaffold, CLI, config, logging | Scaffold compiles and runs on Android | **DONE** |
| **2: Core** | F05, F06, F07, F08 | VFS ioctls, detection (5 scenarios), scanner, planner | Ioctls work, modules scanned, plans generated | **DONE** |
| **3: Strategies** | F09, F10, F11, F12 | OverlayFS, magic mount, VFS executor, pipeline | All 3 strategies functional, pipeline compiles | **DONE** |
| **4: SUSFS** | F13, F14, F15, F16 | SUSFS supercalls, custom cmds, fonts, BRENE | Supercalls work, font redirect works | **DONE** |
| **5: Platform** | F17, F18, F19, F20 | Shell launchers, platform trait, camouflage, watcher | Boot sequence works end-to-end | **DONE** |
| **6: WebUI** | F21, F22, F23, F24, F25 | Cleanup, scenario, settings, toggle, status JSON | Dead code removed, all tabs functional | **DONE** |
| **7: Build** | F26, F27 | CI pipeline, ZIP packaging | CI builds all targets, ZIP installs correctly | **DONE** |
| **8: Post-MVP** | — | LKM CI for GKI builds, nuke_ext4 Kconfig, outstanding issues | See `docs/OUTSTANDING-ISSUES.md` | Pending |

All 27 MVP features completed 2026-02-08. Post-MVP items tracked in `docs/OUTSTANDING-ISSUES.md`.

---

## 6. Per-Pair Task Briefs

### Pair 1-2: Rust Core

**Domain:** Binary scaffold, CLI, config, logging, pipeline controller, process camouflage
**Decision IDs:** R01-R10, ME13, ME15, CO03, KSU09
**Features:** F01, F02, F03, F04, F12, F19

**Files created:**
```
src/main.rs, src/cli/mod.rs, src/cli/handlers.rs
src/core/mod.rs, src/core/config.rs, src/core/pipeline.rs, src/core/state.rs, src/core/types.rs
src/logging/mod.rs, src/logging/kmsg.rs, src/logging/rotating.rs, src/logging/sysfs.rs
src/utils/process.rs
Cargo.toml, Cargo.lock, .cargo/config.toml, .gitignore
```

**Reference docs:** DECISIONS.md R01-R10; DESIGN.md (full file structure, data flow); CONTEXT.md section 4 (current zm binary), section 5 (scripts being replaced)

**Acceptance criteria per feature:**
- F01: `cargo ndk -t arm64-v8a build --release` succeeds
- F02: All subcommands listed in `zeromount --help`
- F03: Config get/set works, bootloop backup/restore tested
- F04: Logs appear in kmsg and file, verbose toggle works
- F12: Pipeline compiles only in valid state order, notify-module-mounted at correct point
- F19: Both `/proc/self/comm` and `/proc/self/cmdline` camouflaged

**Implementation notes:**
- F04 expanded beyond original plan: unified verbose toggle syncs three layers — sysfs kernel debug level (`/sys/kernel/zeromount/debug`), `.verbose` marker file, `config.toml` `logging.verbose` — via `src/logging/sysfs.rs`
- CLI subcommands added: `zeromount logging enable|disable|set-level|status`
- Logging subsystem: `src/logging/` with dual subscribers (kmsg + rotating file) and sysfs control

**Cross-domain dependencies:**
- All other pairs import shared types FROM this pair
- F12 is the integration point -- start LAST after F06-F11 are ready
- Pair 3-4 provides VFS ioctl interface (consumed by F12)
- Pair 5-6 provides scanner and planner (consumed by F12)
- Pair 7-8 provides SUSFS client (consumed by F12)
- Pair 11-12 provides detection engine and RootManager (consumed by F12)

---

### Pair 3-4: VFS Engine

**Domain:** /dev/zeromount ioctl interface, VFS mount executor
**Decision IDs:** VFS01-VFS07, CO01, CO03
**Features:** F05, F11

**Files created:**
```
src/vfs/mod.rs, src/vfs/ioctls.rs, src/vfs/types.rs, src/vfs/executor.rs
```

**Implementation note:** `executor.rs` (266 lines) separates the VFS mount pipeline (inject→SUSFS→enable→refresh) from the raw ioctl interface in `ioctls.rs`.

**Reference docs:** DECISIONS.md VFS01-VFS07; CONTEXT.md section 3 (kernel ioctl table); kernel-verification.md; `/home/claudetest/zero-mount/zeromount/src/zm.c` (current C implementation)

**Acceptance criteria:**
- F05: All 10 ioctls work on ARM64. ARM32 numbers correct at compile time. GET_STATUS fallback works.
- F11: VFS pipeline: inject -> SUSFS -> enable -> refresh ordering enforced. Hot-reload uses CLEAR_ALL + re-inject.

**Cross-domain dependencies:**
- Depends on Pair 1-2 for shared types (ScannedModule, MountPlan)
- F11 depends on Pair 7-8 for SUSFS client (kstat, path hide after inject)
- F11 depends on Pair 5-6 for ScannedModule data
- F05 is independent -- start immediately after F01

**Key technical details:**
- Ioctl magic: `0x5A` (ASCII 'Z')
- ARM64 struct: 24 bytes (two 8B pointers + 4B flags + 4B padding) -> `0x40185A01`
- ARM32 struct: 12 bytes (two 4B pointers + 4B flags) -> `0x400C5A01`
- GET_VERSION is the ONLY ioctl NOT requiring CAP_SYS_ADMIN
- REFRESH (0x5A0A) exists in kernel but was missing from zm.c -- implement it
- CO01: Both `del_rule` AND `clear_all` leak `dirs_ht` entries

---

### Pair 5-6: Mount Engine

**Domain:** Module scanner, BFS planner, OverlayFS mounting, magic mount, storage cascade
**Decision IDs:** ME01-ME15, CO02
**Features:** F07, F08, F09, F10

**Files created:**
```
src/modules/mod.rs, src/modules/scanner.rs, src/modules/model.rs, src/modules/rules.rs
src/mount/mod.rs, src/mount/planner.rs, src/mount/executor.rs, src/mount/overlay.rs
src/mount/magic.rs, src/mount/storage.rs, src/mount/umount.rs
nuke_ext4_lkm/nuke.c, nuke_ext4_lkm/Makefile
module/lkm/.gitkeep
```

**Reference docs:** DECISIONS.md ME01-ME15; mount-verification.md; CONTEXT.md section 5; `/home/claudetest/zero-mount/zeromount/module/metamount.sh` (current 427-line implementation)

**Acceptance criteria:**
- F07: Scanner finds modules, resolves all 3 whiteout formats + opaque dirs + redirect xattr. Uses 23-partition constant.
- F08: BFS planner never mounts at partition roots. SAR detection works.
- F09: OverlayFS works with EROFS/tmpfs/ext4. New mount API on 5.2+. Backing files nuked.
- F10: Magic mount works. Per-module overlay-to-magic fallback.

**Cross-domain dependencies:**
- Depends on Pair 1-2 for shared types and config (F03)
- Pair 3-4 consumes scanner output in F11
- Pair 1-2 consumes planner output in F12

**Key technical details:**
- 23 unique partitions: `system vendor product system_ext odm oem my_bigball my_carrier my_company my_engineering my_heytap my_manifest my_preload my_product my_region my_stock mi_ext cust optics prism oem_dlkm system_dlkm vendor_dlkm`
- Whiteout: char device `mknod c 0 0`, xattr `trusted.overlay.whiteout=y`, AUFS `.wh.*`
- SAR: `/product` may be symlink to `/system/product` (legacy) or separate mount (modern)
- Storage: EROFS preferred; tmpfs xattr NOT in stock GKI defconfig; ext4 loopback fallback
- New mount API: `fsopen("overlay")` -> `fsconfig()` -> `fsmount()` -> `move_mount()`
- Legacy: `mount(source, dest, "overlay", 0, "lowerdir=...")`
- Mount source: `"KSU"` (for zygisk unmount tools, NOT for KSU try_umount which uses path registration)

**Storage improvements (post-plan additions):**
- Command timeout: `run_command_with_timeout()` wraps mkfs.erofs, dd, mkfs.ext4 with 30s deadline (poll-based via `try_wait()` at 100ms intervals)
- Dynamic ext4 sizing: `calculate_ext4_image_size_mb()` scans `/data/adb/modules/` recursively, applies 1.5× headroom, enforces 64MB minimum (replaces hardcoded 2GB sparse image)
- ext4 sysfs nuke: `nuke_ext4_sysfs()` removes `/proc/fs/ext4/<device>` evidence after ext4 loop mount — dual-path: (1) `ksud kernel nuke-ext4-sysfs` for KSU/APatch 22105+, (2) LKM fallback via `insmod nuke.ko mount_point=<path> symaddr=<addr>` (one-shot, returns -EAGAIN to auto-unload)
- LKM selection: `select_nuke_ko()` reads `/proc/version`, extracts kernel major.minor, matches against `module/lkm/nuke-android<ver>-<kernel>.ko`
- Symbol resolution: `read_kallsyms_address()` parses `/proc/kallsyms` for `ext4_unregister_sysfs` address
- New constants: `CMD_TIMEOUT` (30s), `CMD_POLL_INTERVAL` (100ms), `MODULES_DIR_PATH`, `MIN_EXT4_SIZE_MB` (64), `LKM_DIR`

---

### Pair 7-8: SUSFS Client

**Domain:** SUSFS supercall interface, custom commands, font redirect, BRENE features
**Decision IDs:** S01-S13
**Features:** F13, F14, F15, F16

**Files created:**
```
src/susfs/mod.rs, src/susfs/ffi.rs, src/susfs/kstat.rs
src/susfs/paths.rs, src/susfs/fonts.rs, src/susfs/brene.rs
```

**Reference docs:** DECISIONS.md S01-S13; susfs-verification.md (complete command map, supercall mechanism); `/home/claudetest/gki-build/susfs4ksu-new/kernel_patches/include/linux/susfs_def.h` (struct definitions); `/home/claudetest/gki-build/susfs4ksu-new/ksu_susfs/jni/main.c` (userspace reference); `/home/claudetest/zero-mount/zeromount/module/susfs_integration.sh` (978 lines being replaced)

**Acceptance criteria:**
- F13: Supercall works for standard commands. FFI structs match kernel. Version query works.
- F14: Custom commands work via supercall. Graceful fallback when absent.
- F15: Font redirect works. OverlayFS fallback for fonts available.
- F16: BRENE toggles read from config.toml. Mount hiding NEVER invoked.

**Cross-domain dependencies:**
- Depends on Pair 1-2 for config (F03) -- BRENE toggles stored in config.toml
- Pair 3-4 consumes SUSFS client in F11 (kstat/path hide after VFS inject)
- Pair 9-10 needs SUSFS capability info for WebUI toggle state
- Pair 11-12 uses SUSFS detection in F06

**Key technical details:**
- Dual invocation: standard commands (`add_sus_path`, `add_sus_map`, etc.) invoke upstream `ksu_susfs` binary; custom commands use direct supercalls
- Supercall: `syscall(SYS_reboot, 0xDEADBEEF, 0xFAFAFAFA, cmd, &info_struct)`
- `0xDEADBEEF` = KSU_INSTALL_MAGIC1, `0xFAFAFAFA` = SUSFS_MAGIC
- Custom: `kstat_redirect` (0x55573), `open_redirect_all` (0x555c1)
- `open_redirect_all` has NO CLI handler in upstream `ksu_susfs` -- supercall only
- FFI structs: `#[repr(C)]` with exact layout matching `susfs_def.h`
- 20 functions from `susfs_integration.sh` absorbed into Rust
- Mount hiding EXCLUDED -- root cause of LSPosed bug
- Property spoofing uses `resetprop`, NOT SUSFS
- SUSFS kernel version is "v2.0.0" -- verify with `CMD_SUSFS_SHOW_VERSION`

---

### Pair 9-10: WebUI

**Domain:** Dead code cleanup, scenario display, settings/BRENE toggles, glass toggle
**Decision IDs:** W01-W09, CO04
**Features:** F21, F22, F23, F24, F25

**Files created:**
```
webui/src/App.tsx, webui/src/app.css, webui/src/index.tsx
webui/src/routes/StatusTab.tsx, webui/src/routes/SettingsTab.tsx
webui/src/routes/ConfigTab.tsx, webui/src/routes/ModulesTab.tsx
webui/src/components/core/Badge.tsx, webui/src/components/core/Button.tsx
webui/src/components/core/Card.tsx, webui/src/components/core/Input.tsx
webui/src/components/core/ScenarioIndicator.tsx, webui/src/components/core/Skeleton.tsx
webui/src/components/core/Toggle.tsx, webui/src/components/core/glass-toggle.css
webui/src/components/layout/Header.tsx, webui/src/components/layout/Modal.tsx
webui/src/components/layout/NavBar.tsx, webui/src/components/layout/Toast.tsx
webui/src/lib/api.ts, webui/src/lib/api.mock.ts, webui/src/lib/store.ts
webui/src/lib/types.ts, webui/src/lib/ksuApi.ts, webui/src/lib/theme.ts
webui/src/lib/icons.ts, webui/src/lib/constants.ts, webui/src/lib/ksu.d.ts
webui/package.json, webui/pnpm-lock.yaml, webui/vite.config.ts
webui/tsconfig.json, webui/tsconfig.app.json, webui/tsconfig.node.json
```

**Reference docs:** DECISIONS.md W01-W09, CO04; webui-verification.md; CONTEXT.md sections 6, 9; `/home/claudetest/zero-mount/zeromount/webui/` (current source); `/home/president/Git-repo-success/glass-toggle.css`

**Acceptance criteria:**
- F21: All 15 dead code items removed. Build outputs to webroot/. No @material/web.
- F22: Scenario indicator with color coding. Engine state from GET_STATUS.
- F23: SUSFS hierarchical toggles. BRENE toggles persist to config.toml.
- F24: Glass toggle renders in WebView. Uses `--text-accent`.
- F25: Single `zeromount status` call for bulk data. No `.status_cache.json`.

**Cross-domain dependencies:**
- Depends on Pair 1-2 for status JSON format (RuntimeState struct)
- F21 has NO dependencies -- start immediately
- F22-F25 need the Rust binary's `status --json` output format defined

**Key technical details:**
- Stack: SolidJS 1.9 + Vite 7 + TypeScript 5.9
- `@material/web` is phantom -- not imported anywhere; remove from package.json
- Glass toggle: use `var(--text-accent)` not `var(--accent)` (3 occurrences)
- Remove `@media (prefers-color-scheme: light)` from glass toggle (redundant with JS theme)
- `inset: 0` is fine -- already used in Toggle.tsx:58, StatusTab.css:50, app.css
- BRENE toggles MUST persist to `config.toml` via `zeromount config set`, NOT localStorage
- Property spoofing is NOT SUSFS -- separate UI section (uses resetprop)
- `ksu.exec()` pattern: `ksu.exec(cmd, '{}', callbackName)` with global callback

**Implementation additions beyond plan:**
- ConfigTab.tsx (331 lines): mount engine storage mode, random paths, ext4 sizing config
- ModulesTab.tsx (244 lines): module list with enable/disable/remove actions
- ksuApi.ts (161 lines): KSU WebView `exec()` callback wrapper with promise API
- theme.ts (214 lines): dynamic accent color theming from KSU manager
- api.mock.ts (321 lines): mock API for offline/browser development
- Component library split: `components/core/` (UI primitives) and `components/layout/` (structural)
- pnpm used as package manager

---

### Pair 11-12: Detection + Platform

**Domain:** Detection engine, platform abstraction, shell launchers, CI, ZIP packaging
**Decision IDs:** DET01-DET07, KSU01-KSU11, CO01-CO04, B01-B05, S01-S02, S06, S11
**Features:** F06, F17, F18, F20, F26, F27

**Files created:**
```
src/detect/mod.rs, src/detect/kernel.rs, src/detect/susfs.rs, src/detect/watcher.rs
src/utils/mod.rs, src/utils/platform.rs
module/metamount.sh, module/post-fs-data.sh, module/service.sh
module/uninstall.sh, module/metainstall.sh, module/metauninstall.sh
module/customize.sh, module/module.prop
scripts/package.sh
patches/susfs/fix-susfs-safety.sh, patches/susfs/inject-susfs-custom-handlers.sh
patches/susfs/inject-susfs-kstat-redirect.sh, patches/susfs/inject-susfs-mount-display.sh
patches/susfs/inject-susfs-open-redirect-all.sh, patches/susfs/inject-susfs-supercall-dispatch.sh
patches/susfs/inject-susfs-unicode-filter-func.sh, patches/susfs/inject-susfs-vfs-open-redirect-all.sh
patches/susfs/inject-susfs-zeromount-coupling.sh, patches/susfs/unicode_filter.sh
```

**Reference docs:** DECISIONS.md DET01-DET07, KSU01-KSU10, B01-B05; ksu-verification.md; `/home/claudetest/zero-mount/reference/kernelsu-module-guide.md`; `/home/claudetest/gki-build/METAMODULE_COMPLETE_GUIDE.md`

**Acceptance criteria:**
- F06: 5 scenarios detected correctly (Full, SusfsFrontend, KernelOnly, SusfsOnly, None). Three-layer SUSFS probe works.
- F17: Shell scripts under 30 lines. Boot sequence: detect -> mount -> notify.
- F18: RootManager trait with KSU + APatch implementations.
- F20: inotify watcher detects changes within 1s. Fallback polling works.
- F26: CI builds 4 Rust binaries + WebUI. SUSFS patch step integrated.
- F27: ZIP installs correctly. 5 eliminated scripts absent.

**Cross-domain dependencies:**
- F06 depends on Pair 3-4 for VFS ioctl interface (GET_VERSION probe)
- F17 depends on Pair 1-2 for CLI being functional
- F18 consumed by Pair 1-2 (pipeline uses RootManager for description updates)
- F26 depends on all pairs having compilable code
- F27 depends on F26

**Key technical details:**
- `post-fs-data.sh` has 10-second BLOCKING timeout -- detect must complete in <2 seconds
- `metainstall.sh` runs for OTHER module installs, not self (self = `customize.sh`)
- `metauninstall.sh` runs for OTHER module uninstalls, not self (self = `uninstall.sh`)
- `ksud kernel notify-module-mounted` is CRITICAL -- must be called after full pipeline
- KSU04: Do NOT declare `manage.kernel_umount`
- KSU05: `override.description` is KSU-only; APatch uses direct module.prop sed
- DET02: Use `/sys/kernel/zeromount/` NOT `/proc/filesystems` (miscdevice, not registered fs)
- APatch is ARM64-only; x86/x86_64 binaries serve KSU users (emulators, Chromebooks)
- KSU11: `post-mount.sh` and `boot-completed.sh` are intentionally NOT used -- document in shell headers

**Implementation additions beyond plan:**
- Detection uses `(vfs_driver, susfs_available)` tuple match for 5 scenarios; `SusfsOnly` = `(false, true)`
- Package script (`scripts/package.sh`, 205 lines) requires all 4 ABI binaries + webroot; auto-includes `module/lkm/*.ko`
- Version sourced from `Cargo.toml` as single source of truth
- SUSFS patches: 10 kernel injection scripts in `patches/susfs/` (1,526 total lines) for build-time patching
- `customize.sh` (70 lines): KSU/APatch installation with arch detection and binary placement

---

## 7. Dependency Graph

```
Phase 1 (Foundation -- Pair 1-2 starts):
  F01 ──> F02 ──> F03
  F01 ──> F04

Phase 2 (Core -- after F01):
  F01 ──> F05 (VFS ioctls, Pair 3-4)
  F03 ──> F07 (Scanner, Pair 5-6)
  F05 ──> F06 (Detection — 5 scenarios, Pair 11-12)
  F07 ──> F08 (BFS planner, Pair 5-6)

Phase 3 (Strategies -- after Phase 2):
  F08 ──> F09 (OverlayFS, Pair 5-6)
  F08 ──> F10 (Magic mount, Pair 5-6)
  F05 + F07 ──> F11 (VFS executor, Pair 3-4)
  F06 + F07-F11 ──> F12 (Pipeline, Pair 1-2)

Phase 4 (SUSFS -- after F01, parallel with Phases 2-3):
  F01 ──> F13 (SUSFS client, Pair 7-8)
  F13 ──> F14 (Custom commands, Pair 7-8)
  F14 ──> F15 (Font redirect, Pair 7-8)
  F13 + F03 ──> F16 (BRENE, Pair 7-8)

Phase 5 (Platform -- after F02):
  F02 ──> F17 (Shell launchers, Pair 11-12)
  F02 ──> F18 (Platform abstraction, Pair 11-12)
  F01 ──> F19 (Process camouflage, Pair 1-2)
  F05 + F12 ──> F20 (inotify watcher, Pair 11-12)

Phase 6 (WebUI -- independent of Rust):
  F21 (cleanup, Pair 9-10) -- NO deps, start immediately
  F21 ──> F22, F23, F24 (parallel, Pair 9-10)
  F22 ──> F25 (status JSON, Pair 9-10)

Phase 7 (Build -- after everything):
  F01 + F21 ──> F26 (CI, Pair 11-12)
  F26 ──> F27 (ZIP packaging, Pair 11-12)
```

**Critical path:** F01 -> F02 -> F03 -> F07 -> F08 -> F09/F10/F11 -> F12

**Immediate parallel starts after F01:**
- F04 (logging, Pair 1-2)
- F05 (VFS ioctls, Pair 3-4)
- F13 (SUSFS supercall, Pair 7-8)
- F19 (process camouflage, Pair 1-2)
- F21 (WebUI cleanup, Pair 9-10) -- NO deps at all

---

## 8. Pending Items

All 9 previously-pending decisions have been resolved and implemented:

| Decision | Topic | Resolution | Implementation |
|----------|-------|------------|----------------|
| S07 | Font OverlayFS fallback | open_redirect primary + OverlayFS fallback | `src/susfs/fonts.rs` |
| S08 | BRENE feature list | All toggles, default ON for APK/zygisk/font hide | `src/susfs/brene.rs` |
| S09 | kstat_redirect (0x55573) | Supercall with fallback to add_sus_kstat_statically | `src/susfs/kstat.rs` |
| S10 | open_redirect_all (0x555c1) | Supercall with fallback to per-UID open_redirect | `src/susfs/mod.rs` |
| S11 | SUSFS unicode filter | Build-time patch chain | `patches/susfs/inject-susfs-unicode-filter-func.sh` |
| S12 | SUSFS config files | Rust binary calls SUSFS directly, no config files | `src/susfs/mod.rs` |
| S13 | SUSFS fork diff | 10 injection scripts for build-time patching | `patches/susfs/` |
| W08 | Glass toggle CSS | `--text-accent`, removed `prefers-color-scheme` | `webui/src/components/core/glass-toggle.css` |
| W09 | BRENE WebUI toggles | Persists to config.toml, property spoofing separate | `webui/src/routes/SettingsTab.tsx` |

---

## 9. Reference Paths

### Source Code
| Path | Contents |
|------|----------|
| `/home/claudetest/zero-mount/zeromount/` | ZeroMount v1 module (current production) |
| `/home/claudetest/zero-mount/zeromount/src/zm.c` | Current C binary (304 lines) |
| `/home/claudetest/zero-mount/zeromount/module/metamount.sh` | Current boot orchestrator (427 lines) |
| `/home/claudetest/zero-mount/zeromount/module/susfs_integration.sh` | Current SUSFS integration (978 lines) |
| `/home/claudetest/zero-mount/zeromount/webui/` | Current WebUI source |
| `/home/claudetest/gki-build/susfs4ksu-new/` | Custom SUSFS fork |
| `/home/claudetest/gki-build/susfs4ksu-new/kernel_patches/include/linux/susfs_def.h` | SUSFS struct definitions (102 lines) |
| `/home/claudetest/gki-build/susfs4ksu-new/ksu_susfs/jni/main.c` | SUSFS userspace tool (787 lines) |
| `/home/claudetest/zero-mount/susfs-module/` | SUSFS flashable module |
| `/home/president/Git-repo-success/glass-toggle.css` | Glass morphism toggle CSS |
| `nuke_ext4_lkm/` | One-shot LKM source for ext4 sysfs evidence removal (nuke.c + Makefile) |
| `patches/susfs/` | 10 SUSFS kernel injection scripts for build-time patching |
| `scripts/package.sh` | ZIP packaging script (205 lines) |
| `module/lkm/` | Drop zone for pre-compiled GKI .ko files |

### Reference Documentation
| Path | Contents |
|------|----------|
| `/home/claudetest/zero-mount/reference/kernelsu-module-guide.md` | KernelSU module development guide |
| `/home/claudetest/zero-mount/reference/kernelsu-module-config.md` | KernelSU module config API |
| `/home/claudetest/zero-mount/reference/kernelsu-module-webui.md` | KernelSU WebUI integration |
| `/home/claudetest/zero-mount/reference/kernelsu-additional-docs.md` | Additional KernelSU docs |
| `/home/claudetest/gki-build/METAMODULE_COMPLETE_GUIDE.md` | Metamodule complete guide |

### Kernel
| Path | Contents |
|------|----------|
| `/home/claudetest/gki-build/kernel-test/android12-5.10-2024-05/` | Kernel source (android12-5.10) |

### Analysis
| Path | Contents |
|------|----------|
| `/home/claudetest/zero-mount/context-gathering/output/` | Full context analysis output |

### Project Documentation
| Path | Contents |
|------|----------|
| `docs/OUTSTANDING-ISSUES.md` | Post-MVP issues, remaining work items (466 lines) |

---

## 10. Communication Protocol

1. **Within pair:** Message your partner FIRST for any design question, code review, or blocker.
2. **Cross-domain issues:** Message the team lead for issues that affect other pairs.
3. **Type contract changes:** If you need to change a shared type (Section 4), message Pair 1-2 AND all consuming pairs BEFORE making the change.
4. **Dependency completion:** When you complete a feature, message all pairs that depend on it with a summary of the API.
5. **Blockers:** If blocked on another pair's output, message them directly with specifics: feature ID, what you need, what you're blocked on.
6. **PENDING decisions:** If a PENDING decision blocks your work, implement the primary approach from Section 8 and flag it for user review.

---

## 11. Quality Gates

Each feature must pass before marking complete:

1. **Compiles:** `cargo ndk -t arm64-v8a build --release` succeeds (Rust) or `npm run build` succeeds (WebUI)
2. **Unit tests:** Core logic has tests -- mount planning, ioctl encoding, config resolution, SUSFS struct layout verification
3. **Matches decisions:** Implementation matches the DECISIONS.md description for ALL referenced decision IDs
4. **Acceptance criteria:** All criteria from features.json satisfied
5. **No regressions:** Feature doesn't break previously-passing features
6. **No `unwrap()` in production paths** (test-only OK)
7. **All FFI structs have `#[repr(C)]`** and match the C header layout
8. **Partner validates** by reading every line of the diff

---

*Generated from verified analysis of 6 domain reports covering 81 architecture decisions and 27 features. Updated 2026-02-09 to reflect completed implementation.*
