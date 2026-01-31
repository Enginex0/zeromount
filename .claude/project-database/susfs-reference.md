# SUSFS Reference

**Version:** v2.0.0 GKI
**Source Path:** `/home/claudetest/gki-build/susfs4ksu-new/`
**Variant:** GKI (kernel >= 5.0.0) or NON-GKI (kernel < 5.0.0)

## Overview

SUSFS (SUS FileSystem) is a kernel-level hiding framework for Android that integrates with KernelSU. It provides 6 distinct hiding mechanisms:

| Mechanism | Effect | Scope |
|-----------|--------|-------|
| sus_path | Makes paths invisible (ENOENT) | Zygote-spawned apps |
| sus_mount | Hides mount entries from /proc | All processes (configurable) |
| sus_kstat | Spoofs file stat information | Zygote-spawned apps |
| sus_map | Hides mmapped files from /proc/maps | Umounted apps |
| open_redirect | Redirects file opens (root only) | UID < 2000 |
| spoof_uname | Spoofs uname syscall output | All processes |

SUSFS hides artifacts by making them invisible. ZeroMount redirects paths to different content. These are complementary systems.

## Architecture

### VFS Hook Mechanism

SUSFS hooks VFS operations at multiple points in the kernel:

```
fs/namei.c
  __lookup_hash()      -> sus_path hiding (dcache)
  lookup_fast()        -> sus_path hiding (RCU lookup)
  __lookup_slow()      -> sus_path hiding (slow path)
  lookup_open()        -> sus_path hiding (open ops)
  link_path_walk()     -> sus_path hiding (path walk)
  do_filp_open()       -> open_redirect

fs/namespace.c
  mnt_alloc_id()       -> sus_mount ID spoofing
  mnt_alloc_group_id() -> sus_mount group ID
  show_vfsmnt()        -> mount entry hiding

fs/stat.c
  generic_fillattr()   -> sus_kstat spoofing

fs/exec.c
  do_execveat_common() -> su compatibility hooks

drivers/input/input.c
  input_handle_event() -> volume key detection

fs/proc/task_mmu.c
  show_map_vma()       -> sus_map hiding
```

### Thread-Local State

SUSFS uses thread-local flags in `task_struct->thread_info.flags`:

```c
// susfs_def.h:57
#define TIF_PROC_UMOUNTED 33
```

Check if current process is "umounted" (hiding active):
```c
// susfs_def.h:83-85
static inline bool susfs_is_current_proc_umounted(void) {
    return test_ti_thread_flag(&current->thread_info, TIF_PROC_UMOUNTED);
}
```

Set process as "umounted":
```c
// susfs_def.h:87-89
static inline void susfs_set_current_proc_umounted(void) {
    set_ti_thread_flag(&current->thread_info, TIF_PROC_UMOUNTED);
}
```

### Inode Mapping Flags

SUSFS marks inodes with flags in `inode->i_mapping->flags`:

| Flag | Bit | Purpose |
|------|-----|---------|
| AS_FLAGS_SUS_PATH | 33 | Path hiding |
| AS_FLAGS_SUS_MOUNT | 34 | Mount hiding |
| AS_FLAGS_SUS_KSTAT | 35 | Stat spoofing |
| AS_FLAGS_OPEN_REDIRECT | 36 | Open redirect (root) |
| AS_FLAGS_ANDROID_DATA_ROOT_DIR | 37 | Android data root marker |
| AS_FLAGS_SDCARD_ROOT_DIR | 38 | SDCard root marker |
| AS_FLAGS_SUS_MAP | 39 | Map hiding |
| AS_FLAGS_OPEN_REDIRECT_ALL | 40 | Open redirect (all UIDs) |

Source: `susfs_def.h:59-74`

## Data Structures

### sus_path Structures

```c
// susfs.h:29-34
struct st_susfs_sus_path {
    unsigned long   target_ino;
    char            target_pathname[SUSFS_MAX_LEN_PATHNAME];  // 256 bytes
    unsigned int    i_uid;
    int             err;
};

// susfs.h:36-41 - Linked list node
struct st_susfs_sus_path_list {
    struct list_head          list;
    struct st_susfs_sus_path  info;
    char                      target_pathname[SUSFS_MAX_LEN_PATHNAME];
    size_t                    path_len;
};

// susfs.h:43-48 - External directory marker
struct st_external_dir {
    char    target_pathname[SUSFS_MAX_LEN_PATHNAME];
    bool    is_inited;
    int     cmd;
    int     err;
};
```

### sus_kstat Structures

```c
// susfs.h:61-78
struct st_susfs_sus_kstat {
    int             is_statically;
    unsigned long   target_ino;
    char            target_pathname[SUSFS_MAX_LEN_PATHNAME];
    unsigned long   spoofed_ino;
    unsigned long   spoofed_dev;
    unsigned int    spoofed_nlink;
    long long       spoofed_size;
    long            spoofed_atime_tv_sec;
    long            spoofed_mtime_tv_sec;
    long            spoofed_ctime_tv_sec;
    long            spoofed_atime_tv_nsec;
    long            spoofed_mtime_tv_nsec;
    long            spoofed_ctime_tv_nsec;
    unsigned long   spoofed_blksize;
    unsigned long long spoofed_blocks;
    int             err;
};

// susfs.h:80-84 - Hash table node
struct st_susfs_sus_kstat_hlist {
    unsigned long               target_ino;
    struct st_susfs_sus_kstat   info;
    struct hlist_node           node;
};

// susfs.h:86-102 - Redirect structure
struct st_susfs_sus_kstat_redirect {
    char            virtual_pathname[SUSFS_MAX_LEN_PATHNAME];
    char            real_pathname[SUSFS_MAX_LEN_PATHNAME];
    unsigned long   spoofed_ino;
    // ... (same stat fields as sus_kstat)
    int             err;
};
```

### open_redirect Structures

```c
// susfs.h:132-137
struct st_susfs_open_redirect {
    unsigned long   target_ino;
    char            target_pathname[SUSFS_MAX_LEN_PATHNAME];
    char            redirected_pathname[SUSFS_MAX_LEN_PATHNAME];
    int             err;
};

// susfs.h:139-144 - Hash table node
struct st_susfs_open_redirect_hlist {
    unsigned long       target_ino;
    char                target_pathname[SUSFS_MAX_LEN_PATHNAME];
    char                redirected_pathname[SUSFS_MAX_LEN_PATHNAME];
    struct hlist_node   node;
};
```

### Global Data Structures

```c
// susfs.c - Linked Lists (3 total)
static LIST_HEAD(LH_SUS_PATH_LOOP);        // Line 110
static LIST_HEAD(LH_SUS_PATH_ANDROID_DATA); // Line 111
static LIST_HEAD(LH_SUS_PATH_SDCARD);      // Line 112

// susfs.c - Hash Tables (3 total)
static DEFINE_HASHTABLE(SUS_KSTAT_HLIST, 10);          // Line 479 (1024 buckets)
static DEFINE_HASHTABLE(OPEN_REDIRECT_HLIST, 10);      // Line 976 (1024 buckets)
static DEFINE_HASHTABLE(OPEN_REDIRECT_ALL_HLIST, 10);  // Line 978 (1024 buckets)

// susfs.c - Spinlocks
static DEFINE_SPINLOCK(susfs_spin_lock_sus_path);      // Line 109
static DEFINE_SPINLOCK(susfs_spin_lock_sus_mount);     // Line 453
static DEFINE_SPINLOCK(susfs_spin_lock_sus_kstat);     // Line 478
static DEFINE_SPINLOCK(susfs_spin_lock_open_redirect); // Line 975
static DEFINE_SPINLOCK(susfs_spin_lock_open_redirect_all); // Line 977
```

## IOCTL Reference

SUSFS uses a custom syscall interface via `SYS_reboot`:

**Magic Numbers:**
- `SUSFS_MAGIC`: `0xFAFAFAFA` (susfs_def.h:10)
- `KSU_INSTALL_MAGIC1`: `0xDEADBEEF`

**Invocation Pattern:**
```c
syscall(SYS_reboot, KSU_INSTALL_MAGIC1, SUSFS_MAGIC, CMD_SUSFS_*, &info);
```

| Command | Hex | Input Structure | Handler Function | Effect |
|---------|-----|-----------------|------------------|--------|
| CMD_SUSFS_ADD_SUS_PATH | 0x55550 | st_susfs_sus_path | susfs_add_sus_path() | Add path to hiding list |
| CMD_SUSFS_SET_ANDROID_DATA_ROOT_PATH | 0x55551 | st_external_dir | susfs_set_i_state_on_external_dir() | Set Android data root |
| CMD_SUSFS_SET_SDCARD_ROOT_PATH | 0x55552 | st_external_dir | susfs_set_i_state_on_external_dir() | Set SDCard root |
| CMD_SUSFS_ADD_SUS_PATH_LOOP | 0x55553 | st_susfs_sus_path | susfs_add_sus_path_loop() | Add path with re-flagging |
| CMD_SUSFS_ADD_SUS_MOUNT | 0x55560 | - | (deprecated) | - |
| CMD_SUSFS_HIDE_SUS_MNTS_FOR_NON_SU_PROCS | 0x55561 | st_susfs_hide_sus_mnts_for_non_su_procs | susfs_set_hide_sus_mnts_for_non_su_procs() | Toggle mount hiding |
| CMD_SUSFS_ADD_SUS_KSTAT | 0x55570 | st_susfs_sus_kstat | susfs_add_sus_kstat() | Add kstat spoofing (dynamic) |
| CMD_SUSFS_UPDATE_SUS_KSTAT | 0x55571 | st_susfs_sus_kstat | susfs_update_sus_kstat() | Update kstat entry |
| CMD_SUSFS_ADD_SUS_KSTAT_STATICALLY | 0x55572 | st_susfs_sus_kstat | susfs_add_sus_kstat() | Add kstat spoofing (static) |
| CMD_SUSFS_ADD_SUS_KSTAT_REDIRECT | 0x55573 | st_susfs_sus_kstat_redirect | susfs_add_sus_kstat_redirect() | Add kstat redirect |
| CMD_SUSFS_SET_UNAME | 0x55590 | st_susfs_uname | susfs_set_uname() | Set spoofed uname |
| CMD_SUSFS_ENABLE_LOG | 0x555a0 | st_susfs_log | susfs_enable_log() | Toggle kernel logging |
| CMD_SUSFS_SET_CMDLINE_OR_BOOTCONFIG | 0x555b0 | st_susfs_spoof_cmdline_or_bootconfig | susfs_set_cmdline_or_bootconfig() | Spoof /proc/cmdline |
| CMD_SUSFS_ADD_OPEN_REDIRECT | 0x555c0 | st_susfs_open_redirect | susfs_add_open_redirect() | Add redirect (UID < 2000) |
| CMD_SUSFS_ADD_OPEN_REDIRECT_ALL | 0x555c1 | st_susfs_open_redirect | susfs_add_open_redirect_all() | Add redirect (all UIDs) |
| CMD_SUSFS_SHOW_VERSION | 0x555e1 | st_susfs_version | susfs_show_version() | Get SUSFS version |
| CMD_SUSFS_SHOW_ENABLED_FEATURES | 0x555e2 | st_susfs_enabled_features | susfs_get_enabled_features() | List enabled configs |
| CMD_SUSFS_SHOW_VARIANT | 0x555e3 | st_susfs_variant | susfs_show_variant() | Get GKI/NON-GKI variant |
| CMD_SUSFS_ENABLE_AVC_LOG_SPOOFING | 0x60010 | st_susfs_avc_log_spoofing | susfs_set_avc_log_spoofing() | Toggle AVC log spoofing |
| CMD_SUSFS_ADD_SUS_MAP | 0x60020 | st_susfs_sus_map | susfs_add_sus_map() | Add map hiding |

Source: `susfs_def.h:11-35`

## Kernel Config Options

All options defined in `10_enable_susfs_for_ksu.patch`:

| Config Option | Default | Description |
|---------------|---------|-------------|
| CONFIG_KSU_SUSFS | y | Master enable for SUSFS (requires THREAD_INFO_IN_TASK) |
| CONFIG_KSU_SUSFS_SUS_PATH | y | Hide paths from syscalls |
| CONFIG_KSU_SUSFS_SUS_MOUNT | y | Hide mounts from /proc |
| CONFIG_KSU_SUSFS_SUS_KSTAT | y | Spoof file stat info |
| CONFIG_KSU_SUSFS_SPOOF_UNAME | y | Spoof uname syscall |
| CONFIG_KSU_SUSFS_ENABLE_LOG | y | Enable kernel logging |
| CONFIG_KSU_SUSFS_HIDE_KSU_SUSFS_SYMBOLS | y | Hide symbols from /proc/kallsyms |
| CONFIG_KSU_SUSFS_SPOOF_CMDLINE_OR_BOOTCONFIG | y | Spoof /proc/cmdline or /proc/bootconfig |
| CONFIG_KSU_SUSFS_OPEN_REDIRECT | y | Redirect file opens (experimental) |
| CONFIG_KSU_SUSFS_SUS_MAP | y | Hide mmapped files from /proc/maps |

**Dependencies:**
- All features require `CONFIG_KSU_SUSFS`
- `CONFIG_KSU_SUSFS` requires `CONFIG_KSU` and `CONFIG_THREAD_INFO_IN_TASK`

## Key Functions

### Path Hiding (sus_path)

| Function | File:Line | Purpose |
|----------|-----------|---------|
| susfs_add_sus_path() | susfs.c:178 | Add path to hiding list |
| susfs_add_sus_path_loop() | susfs.c:269 | Add path with per-zygote re-flagging |
| susfs_run_sus_path_loop() | susfs.c:332 | Re-flag paths for new process |
| susfs_is_inode_sus_path() | susfs.c:418-446 | Check if inode should be hidden |
| susfs_is_sus_android_data_d_name_found() | susfs.c:377 | Check Android/data path |
| susfs_is_sus_sdcard_d_name_found() | susfs.c:399 | Check SDCard path |

### Stat Spoofing (sus_kstat)

| Function | File:Line | Purpose |
|----------|-----------|---------|
| susfs_add_sus_kstat() | susfs.c:507 | Add kstat spoofing rule |
| susfs_add_sus_kstat_redirect() | susfs.c:578 | Add kstat redirect rule |
| susfs_update_sus_kstat() | susfs.c:738 | Update existing kstat entry |
| susfs_sus_ino_for_generic_fillattr() | susfs.c:794 | Spoof stat in generic_fillattr |
| susfs_sus_ino_for_show_map_vma() | susfs.c:820 | Spoof ino/dev in /proc/maps |

### Open Redirect

| Function | File:Line | Purpose |
|----------|-----------|---------|
| susfs_add_open_redirect() | susfs.c:1034 | Add redirect rule (UID < 2000) |
| susfs_add_open_redirect_all() | susfs.c:1074 | Add redirect rule (all UIDs) |
| susfs_get_redirected_path() | susfs.c:1114 | Get redirect path by inode |
| susfs_get_redirected_path_all() | susfs.c:1130 | Get redirect path (all UIDs) |

### Mount Hiding (sus_mount)

| Function | File:Line | Purpose |
|----------|-----------|---------|
| susfs_set_hide_sus_mnts_for_non_su_procs() | susfs.c:456 | Toggle global mount hiding |
| susfs_alloc_sus_vfsmnt() | namespace.c patch | Allocate fake mount ID |
| susfs_reuse_sus_vfsmnt() | namespace.c patch | Reuse mount ID on copy |
| susfs_reorder_mnt_id() | setuid_hook.c | Reorder mount IDs after umount |

### Utility Functions

| Function | File:Line | Purpose |
|----------|-----------|---------|
| susfs_init() | susfs.c:1335 | Initialize SUSFS subsystem |
| susfs_is_current_proc_umounted() | susfs_def.h:83 | Check if hiding is active |
| susfs_set_current_proc_umounted() | susfs_def.h:87 | Enable hiding for process |
| susfs_is_uid_zeromount_excluded() | susfs_def.h:94-97 | Check ZeroMount exclusion |
| susfs_check_unicode_bypass() | susfs.c:43 | Block Unicode path attacks |

## Integration Points

### ZeroMount Integration

SUSFS integrates with ZeroMount for unified UID exclusion:

```c
// susfs_def.h:91-101
#ifdef CONFIG_ZEROMOUNT
extern bool zeromount_is_uid_blocked(uid_t uid);
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) {
    bool result = zeromount_is_uid_blocked(uid);
    printk(KERN_INFO "susfs: zeromount_check uid=%u result=%d\n", uid, result);
    return result;
}
#else
static inline bool susfs_is_uid_zeromount_excluded(uid_t uid) { return false; }
#endif
```

**Integration Usage (3 locations in susfs.c):**

```c
// susfs.c:349-354 - is_i_uid_in_android_data_not_allowed()
if (susfs_is_uid_zeromount_excluded(current_uid().val))
    return false;

// susfs.c:356-360 - is_i_uid_in_sdcard_not_allowed()
if (susfs_is_uid_zeromount_excluded(current_uid().val))
    return false;

// susfs.c:362-367 - is_i_uid_not_allowed()
if (susfs_is_uid_zeromount_excluded(current_uid().val))
    return false;
```

### KernelSU Integration

SUSFS integrates with KernelSU through:

1. **SELinux SID Caching** (selinux.c patch):
   - `susfs_set_ksu_sid()` / `susfs_is_current_ksu_domain()`
   - `susfs_set_zygote_sid()` / `susfs_is_current_zygote_domain()`
   - `susfs_set_init_sid()` / `susfs_is_current_init_domain()`

2. **Setuid Hook** (setuid_hook.c):
   - Modified `ksu_handle_setresuid()` to set `TIF_PROC_UMOUNTED`
   - Calls `susfs_run_sus_path_loop()` and `susfs_reorder_mnt_id()`

3. **Supercalls** (supercalls.c):
   - SUSFS commands routed through `ksu_handle_sys_reboot()`
   - Checks `magic2 == SUSFS_MAGIC` before dispatching

## Build Guide

### Patch Application Order

1. Apply ZeroMount patch first (if using ZeroMount integration)
2. Apply KernelSU patches:
   ```
   KernelSU/10_enable_susfs_for_ksu.patch
   KernelSU/fix_sucompat_stat_bypass.patch (if needed)
   ```
3. Apply SUSFS GKI patch:
   ```
   50_add_susfs_in_gki-android12-5.10.patch
   ```
4. Copy source files:
   ```
   cp fs/susfs.c <kernel>/fs/
   cp include/linux/susfs.h <kernel>/include/linux/
   cp include/linux/susfs_def.h <kernel>/include/linux/
   ```

### Required CONFIG Options

```
CONFIG_KSU=y
CONFIG_KSU_SUSFS=y
CONFIG_THREAD_INFO_IN_TASK=y

# Optional features (all default y)
CONFIG_KSU_SUSFS_SUS_PATH=y
CONFIG_KSU_SUSFS_SUS_MOUNT=y
CONFIG_KSU_SUSFS_SUS_KSTAT=y
CONFIG_KSU_SUSFS_SPOOF_UNAME=y
CONFIG_KSU_SUSFS_ENABLE_LOG=y
CONFIG_KSU_SUSFS_HIDE_KSU_SUSFS_SYMBOLS=y
CONFIG_KSU_SUSFS_SPOOF_CMDLINE_OR_BOOTCONFIG=y
CONFIG_KSU_SUSFS_OPEN_REDIRECT=y
CONFIG_KSU_SUSFS_SUS_MAP=y

# For ZeroMount integration
CONFIG_ZEROMOUNT=y
```

### GKI Build Commands

```bash
# Example for Android 12 / 5.10
gh workflow run build.yml --repo Enginex0/kernelsu-next-vanilla \
  -r main -f android_version=android12 -f kernel_version=5.10 \
  -f sub_level=209 -f os_patch_level=2024-05 -f device_codename=lake
```

## Quick Reference

### Userspace CLI (ksu_susfs)

```bash
# Path hiding
ksu_susfs add_sus_path /path/to/hide
ksu_susfs add_sus_path_loop /path/to/hide  # Re-flags per zygote spawn
ksu_susfs set_android_data_root_path /sdcard/Android/data
ksu_susfs set_sdcard_root_path /sdcard

# Mount hiding
ksu_susfs hide_sus_mnts_for_non_su_procs 1  # Enable
ksu_susfs hide_sus_mnts_for_non_su_procs 0  # Disable

# Stat spoofing
ksu_susfs add_sus_kstat /path/to/spoof
ksu_susfs update_sus_kstat /path/to/spoof
ksu_susfs add_sus_kstat_statically /path ino dev nlink size atime atime_ns mtime mtime_ns ctime ctime_ns blocks blksize
ksu_susfs add_sus_kstat_redirect /virtual/path /real/path ino dev nlink size atime atime_ns mtime mtime_ns ctime ctime_ns blocks blksize

# Open redirect (UID < 2000 only)
ksu_susfs add_open_redirect /target/path /redirected/path

# Map hiding
ksu_susfs add_sus_map /path/to/library.so

# Uname spoofing
ksu_susfs set_uname "5.10.123-gki" "default"

# Logging
ksu_susfs enable_log 1  # Enable
ksu_susfs enable_log 0  # Disable

# AVC log spoofing
ksu_susfs enable_avc_log_spoofing 1

# Info commands
ksu_susfs show version          # e.g., "v2.0.0"
ksu_susfs show variant          # "GKI" or "NON-GKI"
ksu_susfs show enabled_features
```

### Key File Locations

```
Kernel Source:
  fs/susfs.c                    -> Main implementation
  include/linux/susfs.h         -> Struct definitions, function declarations
  include/linux/susfs_def.h     -> Constants, thread flags, inode flags

Patches:
  kernel_patches/50_add_susfs_in_gki-android12-5.10.patch
  kernel_patches/KernelSU/10_enable_susfs_for_ksu.patch

Userspace Tool:
  ksu_susfs/jni/main.c          -> CLI implementation
```

### Thread State Flow

```
Zygote
  fork() -> setresuid() -> ksu_handle_setresuid()
    |
    +-- ksu_handle_umount() -> umount KSU mounts
    +-- susfs_reorder_mnt_id() -> fix mount IDs
    +-- susfs_run_sus_path_loop() -> re-flag paths
    +-- susfs_set_current_proc_umounted()
              |
              v
         TIF_PROC_UMOUNTED = 1
              |
App Runtime   v
  +-- VFS lookup -> susfs_is_current_proc_umounted() -> true
  |                    +-> Apply hiding logic
  +-- /proc access -> Check TIF_PROC_UMOUNTED
                         +-> Hide mounts/maps
```

---

*Generated for MIT Graduate Security Research*
*Last Updated: 2026-01-31*
