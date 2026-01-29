# Design

## Approach

**VFS path redirection at the namei layer** — instead of mounting overlays (detectable via /proc/mounts), ZeroMount intercepts path resolution in the kernel's Virtual File System to transparently swap file paths.

## Solution Components

### 1. Kernel Patch (zeromount.c + injection scripts)
- 4 core VFS hooks: getname_flags, inode_permission, d_path, getdents64
- 4 additional hooks (from clone analysis): stat, statfs, xattr, overlayfs detection
- Hash table storage: rules (path→path), dirs (parent→children), UIDs (excluded apps)
- RCU concurrency for lock-free hot-path reads
- Start-disabled pattern with explicit ENABLE/DISABLE ioctls

### 2. zm Binary (Freestanding C CLI)
- No libc dependency — runs before Android runtime is available
- Commands (current): add, del, clear, ver, list, blk, unb | Commands (v2 additions): enable, disable
- Communicates via ioctl to /dev/zeromount
- Targets: aarch64 + arm32

### 3. Metamodule Scripts
- metamount.sh: iterate modules → inject rules via zm binary (v2 will add: clear at start, enable after injection, notify-module-mounted at end)
- service.sh: UID exclusions + SUSFS integration (~30 lines)
- customize.sh: Module installation hooks
- metainstall.sh: Hook for regular module installation (receives MODPATH, ZIPFILE)
- metauninstall.sh: Cleanup on regular module removal
- Follows KernelSU metamodule contract (notify-module-mounted, skip_mount, disable, metainstall, metauninstall)

### 4. SUSFS Integration (~200 lines)
- sus_path: Hide module storage paths from readdir
- kstat_redirect: Spoof stat() to show original file metadata
- sus_map: Hide module paths from /proc/self/maps
- open_redirect: (if needed) Redirect opens for SUSFS-managed paths

## Data Flow

```
App calls open("/etc/fonts/custom.conf")
  → getname_flags() in namei.c
    → zeromount hook: hash path, lookup rules table
      → MATCH: swap to "/data/adb/modules/fonts/custom.conf"
        → inode_permission(): bypass check for injected file
          → File opened successfully
            → App reads font data (thinks it came from /etc/fonts/)

App calls readlink(/proc/self/fd/N)
  → d_path() in d_path.c
    → zeromount hook: lookup inode in reverse table
      → MATCH: return "/etc/fonts/custom.conf" (not real path)

App calls ls /etc/fonts/
  → getdents64() in readdir.c
    → Real entries first, then MAGIC_POS sentinel
      → zeromount hook: inject "custom.conf" as virtual entry
```

## Key Design Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | VFS redirection over mounts | Zero evidence in /proc/mounts |
| 2 | metamount.sh over service.sh | Correct boot timing (step 6) |
| 3 | Discard universal mount hijacker | ZeroMount IS the metamodule; no mounts to hijack |
| 4 | Rename to ZeroMount | Clean identity separation from prototype |
| 5 | Shell injection scripts | Cross-kernel-version compatibility |
| 6 | Start disabled + ENABLE ioctl | Prevents early-boot deadlock |
| 7 | Add stat/statfs/xattr/overlayfs hooks | Closes 4 detection vectors |
| 8 | Discard procmounts + base-hide-stuff | Scope creep; mount hiding irrelevant |

## Error Handling

- **Boot deadlock prevention:** Start disabled; metamount.sh enables after fs mounted
- **Dcache staleness:** Invalidate dentry cache after every rule addition
- **Permission mismatch:** Bypass inode_permission for injected files + traversal dirs
- **Memory safety:** RCU for reads, spinlock for writes, call_rcu for deferred frees
