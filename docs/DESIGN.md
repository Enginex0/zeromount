# Design

## Approach

Replace shell-based orchestration with a single Rust binary that owns the full mount pipeline. The binary uses a typestate pattern to enforce pipeline ordering at compile time, eliminating the race conditions and ordering bugs present in the shell implementation. At boot, it probes kernel capabilities to select one of four scenarios, then processes all modules through the selected strategy (VFS redirection, OverlayFS, or magic mount). Shell scripts shrink to thin launchers (<30 lines each).

---

## Components

### 1. Pipeline Controller (`core/pipeline.rs`)

- **Purpose:** Typestate-enforced orchestration of the full mount sequence
- **States:** `Init → Detected → Planned → Mounted → Finalized`
- **Inputs:** Config, module directory path
- **Outputs:** Status JSON, mount results, module description update
- **Key invariant:** Each state transition consumes `self`, preventing out-of-order calls at compile time

### 2. Detection Engine (`detect/`)

- **Purpose:** Probe kernel and userspace capabilities, select scenario
- **Inputs:** `/dev/zeromount` device, SUSFS binary paths, `/sys/kernel/zeromount/` sysfs
- **Outputs:** `Scenario` enum (FULL, SUSFS_FRONTEND, KERNEL_ONLY, NONE) + capability flags
- **Dependencies:** VFS ioctl interface (for `GET_VERSION`), sysfs probing
- **Three-layer SUSFS probe:** (1) Module state — check `.disabled` marker in SUSFS module dir, (2) Binary availability — search standard paths for `ksu_susfs`, probe standard capabilities, (3) Custom kernel ioctls — Rust binary probes kernel directly for build-time patched commands (`kstat_redirect`, `open_redirect_all`). Separation of concern: SUSFS binary is upstream/untouched; custom capabilities are our Rust binary + our kernel patches.

### 3. Module Scanner (`modules/scanner.rs`)

- **Purpose:** Discover enabled modules and resolve mount rules
- **Inputs:** `/data/adb/modules/` directory
- **Outputs:** `Vec<ScannedModule>` with per-module file trees and rules
- **Dependencies:** rayon for parallel scanning, config for rule resolution
- **Filters:** `disable`, `remove`, `skip_mount` sentinels; self-name blacklist

### 4. Mount Planner (`mount/planner.rs`)

- **Purpose:** BFS walk of module file trees to determine minimal mount point set
- **Inputs:** Scanned modules, target partitions, scenario
- **Outputs:** `MountPlan` — list of mount operations grouped by partition
- **Key constraint:** Never mount at partition root — always descend one level

### 5. Mount Executor (`mount/executor.rs`)

- **Purpose:** Execute the mount plan via the selected strategy
- **Inputs:** `MountPlan`, storage backend handle
- **Outputs:** `MountResult` per module (strategy used, success/failure, mount paths)
- **Strategies:**
  - **VFS:** Inject rules via `/dev/zeromount` ioctls, enable engine, refresh dcache
  - **OverlayFS:** New mount API (5.2+) with legacy fallback, source="KSU"
  - **Magic Mount:** Bind mounts per file, tmpfs skeleton for directories
- **Fallback:** Per-module overlay failure → magic mount for that module only

### 6. Storage Backend (`mount/storage.rs`)

- **Purpose:** Prepare staging area for overlay lower layers
- **Inputs:** Storage mode (auto-detected), module content
- **Outputs:** `StorageHandle` with mount point and mode
- **Cascade:** EROFS (compressed read-only) → tmpfs (RAM, xattr check) → ext4 (loopback image)
- **Cleanup:** Nuke backing files after mount (EROFS image deleted, ext4 superblock zeroed)

### 7. VFS Engine (`vfs/`)

- **Purpose:** Rust interface to the `/dev/zeromount` kernel driver
- **Inputs:** Virtual path + real path pairs, UIDs
- **Outputs:** Ioctl results
- **Commands:** add_rule, del_rule, clear_all, get_version, add_uid, del_uid, get_list, enable, disable, refresh, query_status
- **Key fix:** ARM32 ioctl numbers derived at compile time via `_IOW` macros

### 8. SUSFS Client (`susfs/`)

- **Purpose:** Interface to SUSFS kernel commands for metadata spoofing and path hiding
- **Inputs:** File paths, kstat metadata, capability flags
- **Outputs:** Success/failure per operation
- **Capabilities:** kstat spoofing, path hiding, maps hiding, font redirect, BRENE automation features (auto-hide APKs, zygisk maps, font maps; uname/AVC spoofing)
- **Explicitly excluded:** mount hiding (root cause of LSPosed bug)
- **Communication:** Dual invocation pattern. Standard commands (`add_sus_path`, `add_sus_map`, `add_sus_kstat_statically`, `add_open_redirect`) invoke the upstream `ksu_susfs` binary. Custom commands (`kstat_redirect` 0x55573, `open_redirect_all` 0x555c1) use direct `SYS_reboot` supercalls — `syscall(SYS_reboot, 0xDEADBEEF, 0xFAFAFAFA, CMD_SUSFS_xxx, &info_struct)` — since the upstream binary has no CLI handlers for them.
- **FFI structs:** 5 key structs from `susfs_def.h` must have matching Rust `#[repr(C)]` layouts: `st_susfs_sus_path`, `st_susfs_sus_kstat`, `st_susfs_sus_kstat_redirect`, `st_susfs_open_redirect`, `st_susfs_sus_map`

### 9. CLI (`cli/`)

- **Purpose:** clap-based subcommand dispatch for boot pipeline and WebUI queries
- **Key commands:** `mount`, `detect`, `status`, `module list/scan`, `config get/set`, `vfs *`, `uid *`, `susfs *`, `diag`, `version`
- **WebUI pattern:** WebUI calls `ksu.exec("zeromount status")` → parses JSON stdout

---

## Data Flow

### Boot Sequence

```
post-fs-data.sh             # BLOCKING, 10s timeout shared across modules
  │
  ▼
zeromount detect ──► writes .detection_result.json
  │                   (scenario, capabilities, driver version)
  │                   MUST complete in <2s (10s blocking timeout)
  │
metamount.sh
  │
  ▼
zeromount mount ──► reads .detection_result.json
  │
  ├─ 1. Load config.toml
  ├─ 2. Init storage backend (EROFS/tmpfs/ext4 cascade)
  ├─ 3. Scan modules (rayon parallel)
  ├─ 4. Generate mount plan (BFS planner)
  ├─ 5. Execute strategy:
  │     ├─ FULL/SUSFS_FE/KERNEL_ONLY → VFS inject + enable + refresh
  │     └─ NONE → OverlayFS (per-module magic mount fallback)
  ├─ 6. Apply SUSFS protections (7 commands across 4 domains)
  ├─ 7. Register mounts for try_umount (overlay mode only)
  ├─ 8. Update module.prop description
  ├─ 9. Write .status_cache.json
  └─ 10. Exit 0
  │
metamount.sh
  │
  ▼
ksud kernel notify-module-mounted
```

### WebUI Communication

```
SolidJS WebUI
  │
  ▼ ksu.exec("zeromount status")
Rust binary ──► JSON stdout ──► WebUI parses
  │
  ▼ ksu.exec("zeromount config set key value")
Rust binary ──► writes config.toml ──► stdout "ok"
```

---

## Error Handling

| Failure | Behavior | Recovery |
|---------|----------|----------|
| `/dev/zeromount` missing | Scenario = NONE, use overlay mode | Fully functional without VFS |
| SUSFS binary not found | Scenario downgrades, skip SUSFS ops | VFS still works, no metadata spoofing |
| EROFS unsupported | Fall through to tmpfs | Automatic via storage cascade |
| tmpfs xattr unsupported | Fall through to ext4 | Automatic via storage cascade |
| OverlayFS fails for module X | Magic mount for module X only | Per-module fallback, others unaffected |
| Ioctl ADD_RULE fails | Log error, skip file, continue | Module partially mounted |
| Bootloop detected (3 strikes) | Skip mount pipeline, disable module | User re-enables via KSU manager |
| SUSFS capability missing | Skip that capability, log warning | Partial protection vs full |

---

## File Structure

```
src/
├── main.rs              # Entry point, CLI dispatch
├── cli/
│   ├── mod.rs           # clap command definitions
│   └── handlers.rs      # Subcommand implementations
├── core/
│   ├── config.rs        # TOML config loading + 3-layer resolution
│   ├── pipeline.rs      # MountController<S> typestate machine
│   └── state.rs         # RuntimeState, status JSON serialization
├── detect/
│   ├── mod.rs           # Scenario detection orchestrator
│   ├── kernel.rs        # /dev/zeromount + sysfs probing
│   └── susfs.rs         # SUSFS binary discovery + capability probe
├── modules/
│   ├── scanner.rs       # Parallel module discovery (rayon)
│   ├── model.rs         # Module, ScannedModule, ModuleRules structs
│   └── rules.rs         # 3-layer rule resolution
├── mount/
│   ├── planner.rs       # BFS mount point planning
│   ├── executor.rs      # Strategy dispatch (VFS/overlay/magic)
│   ├── overlay.rs       # OverlayFS mounting (new API + legacy)
│   ├── magic.rs         # Magic mount (bind mount tree)
│   ├── storage.rs       # EROFS/tmpfs/ext4 cascade
│   └── umount.rs        # try_umount registration
├── vfs/
│   ├── mod.rs           # /dev/zeromount ioctl interface
│   ├── ioctls.rs        # Ioctl number definitions (_IOW/_IOR macros)
│   └── types.rs         # ZeromountRule, kernel struct FFI
├── susfs/
│   ├── mod.rs           # SUSFS client (dual: binary + supercall)
│   ├── ffi.rs           # SYS_reboot supercall, #[repr(C)] struct layouts
│   ├── kstat.rs         # Kstat spoofing logic
│   ├── paths.rs         # Path hiding + maps hiding
│   └── fonts.rs         # Font redirect (open_redirect + kstat)
└── utils/
    ├── fs.rs            # Atomic rename, SELinux context, xattr helpers
    ├── process.rs       # Process camouflage (kworker)
    └── platform.rs      # RootManager trait (KSU vs APatch abstraction)

module/                  # Shipped in ZIP
├── module.prop          # metamodule=1
├── customize.sh         # Install hook (arch detect, binary copy)
├── metamount.sh         # Thin launcher → zeromount mount
├── metainstall.sh       # Module install hook (runs for OTHER module installs)
├── metauninstall.sh     # Cleanup hook (runs for OTHER module uninstalls)
├── uninstall.sh         # ZeroMount self-cleanup
├── post-fs-data.sh      # Detection probe → zeromount detect
├── service.sh           # Post-boot setup
├── zm-arm64             # Rust binary (aarch64)
├── zm-arm               # Rust binary (armv7)
├── zm-x86_64            # Rust binary (x86_64, emulators/Chromebooks)
├── zm-x86               # Rust binary (i686, emulators)
├── bin/                 # aapt binary for module metadata
├── config.toml          # Default configuration template
└── webroot/             # Built WebUI output (from webui/ build)

webui/                   # SolidJS WebUI (custom components, no Material Web)
├── src/
│   ├── App.tsx
│   ├── lib/
│   │   ├── api.ts       # ksu.exec calls to Rust binary
│   │   ├── store.ts     # SolidJS signals
│   │   ├── theme.ts     # JS-driven theme switching (dark/light/amoled)
│   │   └── types.ts     # TypeScript interfaces
│   ├── routes/
│   │   ├── StatusTab.tsx # Scenario display, engine status
│   │   ├── ModulesTab.tsx
│   │   ├── ConfigTab.tsx
│   │   └── SettingsTab.tsx # SUSFS toggles, BRENE features, glass morphism toggle
│   └── components/
│       └── Toggle.tsx   # Glass morphism toggle (accent-adaptive via --accent-rgb)
└── vite.config.ts       # Output → module/webroot/
```

---

## CLI Interface

```
zeromount mount                    # Full pipeline (called by metamount.sh)
zeromount detect                   # Probe capabilities, write detection JSON
zeromount status [--json]          # Engine state, modules, scenario
zeromount module list              # Module list with mount status
zeromount module scan              # Force rescan
zeromount config get <key>         # Read config value
zeromount config set <key> <val>   # Write config value
zeromount vfs add <vp> <rp>        # Add VFS redirection rule
zeromount vfs del <vp>             # Delete VFS rule
zeromount vfs clear                # Clear all rules
zeromount vfs enable               # Enable VFS engine
zeromount vfs disable              # Disable VFS engine
zeromount vfs refresh              # Flush dcache
zeromount vfs list                 # List active rules
zeromount vfs query-status         # Engine enabled state
zeromount uid block <uid>          # Exclude UID from redirection
zeromount uid unblock <uid>        # Include UID in redirection
zeromount susfs <feature> <on|off> # Toggle BRENE automation features
zeromount diag                     # Diagnostic dump
zeromount version                  # Version from module.prop
```
