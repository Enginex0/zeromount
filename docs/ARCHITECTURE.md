# Architecture

## System Overview

ZeroMount operates at three layers to achieve zero-mount file redirection on Android:

```
Layer 3: KernelSU Core
├── Loads metamodule at boot step 6
├── Calls metamount.sh (not service.sh)
└── Manages module lifecycle

Layer 2: Metamodule (ZeroMount)
├── metamount.sh      → Boot-time orchestrator (clear → iterate → inject → notify)
├── service.sh        → Post-boot UID exclusions + SUSFS integration
├── zm binary         → Freestanding C CLI, talks to /dev/zeromount via ioctl
├── customize.sh      → Module installation hooks
├── metainstall.sh    → Hook for regular module installation
└── metauninstall.sh  → Cleanup on regular module removal

Layer 1: Kernel Patch (zeromount.c)
├── VFS hooks: namei (path swap), d_path (reverse translate), readdir (inject entries)
├── Extended hooks: stat (metadata spoof), statfs (fs spoof), xattr (SELinux), overlayfs (re-spoof)
├── Permission bypass: inode_permission + generic_permission for injected files
├── Data: 3 hash tables (rules, dirs, UIDs) + RCU concurrency
└── Control: /dev/zeromount misc device, 7 ioctl commands (enable/disable to be added in v2)
```

## Boot Sequence

```
1. Kernel boots → fs_initcall() registers /dev/zeromount (starts ENABLED — v2 will change to start DISABLED via ATOMIC_INIT(0))
2. KernelSU loads → detects metamodule
3. post-fs-data stage begins
4. KernelSU processes regular modules (overlay mounts for non-metamodule modules)
5. Load system.prop
6. KernelSU calls metamount.sh for ZeroMount ← CRITICAL MOMENT
   a. Clear all previous rules
   b. Iterate /data/adb/modules/*/
   c. For each module: check skip_mount, check disable, inject rules via zm binary
   d. Enable ZeroMount via zm enable
   e. Call notify-module-mounted once (signals KernelSU that all mounting is complete)
7. service.sh runs → UID exclusions + SUSFS integration
8. Boot complete → all apps see redirected paths
```

## Key Components

| Component | Location | Purpose |
|-----------|----------|---------|
| zeromount.c | fs/zeromount.c (kernel) | Core VFS redirection engine |
| zeromount.h | include/linux/zeromount.h | Data structures + API |
| zm | /data/adb/zeromount/zm | Freestanding CLI binary |
| metamount.sh | Module root | Boot orchestrator |
| service.sh | Module root | Post-boot SUSFS + UID config |
| metainstall.sh | Module root | Hook for regular module installation |
| metauninstall.sh | Module root | Cleanup on regular module removal |

## Integration Points

| System | Interface | Direction |
|--------|-----------|-----------|
| KernelSU | metamount.sh callback | KernelSU → ZeroMount |
| SUSFS | sus_path, kstat_redirect, sus_map, open_redirect | ZeroMount → SUSFS |
| VFS | namei.c, d_path.c, readdir.c hooks | ZeroMount → Kernel VFS |
| Userspace | /dev/zeromount ioctl | zm binary → Kernel |

## Constraints

- Kernel patch is compiled-in (not loadable module) — no unload capability
- Must start DISABLED to prevent early-boot deadlock (kern_path before fs mounted)
- RCU required for lock-free reads on hot path (path resolution)
- /dev/zeromount is mode 0600 — root only
- Android /system prefix must be handled (dual mount at / and /system)
