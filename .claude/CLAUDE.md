# Zero-Mount Project Instructions

## Session Start Protocol

**READ THE PROJECT DATABASE FIRST:**

```
.claude/project-database/
├── index.md              ← START HERE (navigation, comparison, tasks)
├── susfs-reference.md    ← SUSFS kernel hiding mechanisms
├── zeromount-reference.md← ZeroMount VFS redirection
├── function-index.json   ← 129 functions indexed (kernel, shell, WebUI)
└── integration-map.md    ← Cross-project integration
```

**Quick Start:**
```bash
# Navigate the database
cat .claude/project-database/index.md

# Function lookup
jq '.zeromount_kernel.zeromount_is_uid_blocked' .claude/project-database/function-index.json
```

---

## Project Identity

| Property | Value |
|----------|-------|
| Name | Zero-Mount |
| Binary | `zm` |
| Device | `/dev/zeromount` |
| Config | `CONFIG_ZEROMOUNT` |
| Version | v3.2.0 |

**Two Projects Documented:**
- **Zero-Mount** at `/home/claudetest/zero-mount/nomount/`
- **SUSFS** at `/home/claudetest/gki-build/susfs4ksu-new/`

---

## Core Concept (Memorize This)

```
SUSFS  = HIDING     → Files return ENOENT (not found)
ZeroMount = REDIRECTION → Files return DIFFERENT CONTENT
```

**Unified Exclusion:** `zm blk <uid>` excludes UID from BOTH systems.

---

## UID Exclusion Semantics (INVERTED)

| State | VFS Redirection | SUSFS Hiding | App Sees |
|-------|-----------------|--------------|----------|
| `zm blk <uid>` | Bypassed | Bypassed | REAL files + artifacts |
| Normal | Active | Active | MODULE files, hidden artifacts |
| Root | Bypassed | Bypassed | REAL files (by design) |

**"Excluded" = sees everything real** (for detector apps)

---

## Key Paths

```
Module scripts:  module/
WebUI source:    webui-v2-beta/src/
Kernel patches:  patches/
SUSFS source:    /home/claudetest/gki-build/susfs4ksu-new/
Project database:.claude/project-database/
```

---

## Quick Commands

```bash
# zm CLI
zm add <vpath> <rpath>   # Add redirection rule
zm del <vpath>           # Delete rule
zm blk <uid>             # Exclude UID (sees real)
zm unb <uid>             # Include UID
zm list                  # List all rules
zm enable / zm disable   # Toggle engine

# Build WebUI
cd webui-v2-beta && pnpm build

# Push to device
adb push module/webroot-beta/. /data/local/tmp/webroot/
adb shell "su -c 'cp -r /data/local/tmp/webroot/* /data/adb/modules/zeromount/webroot/'"
```

---

## Rules

1. **Read database first** — Don't guess, look it up
2. **SUSFS ≠ ZeroMount** — Hiding vs redirection
3. **Root sees REAL** — Cannot test redirection as root
4. **Exclusion is inverted** — "Excluded" = sees real files
5. **Test on device** — Kernel integration requires real hardware

---

## When Confused

1. Check `index.md` for common task workflows
2. Search `function-index.json` for specific functions
3. Read reference docs for architecture details
4. Ask user if still unclear

---

## Integration Architecture

```
zm blk <uid>
    │
    ├─► ZeroMount: zeromount_is_uid_blocked() → bypass VFS redirect
    │
    └─► SUSFS: susfs_is_uid_zeromount_excluded() → bypass ALL hiding
        ├─ Path hiding bypassed
        ├─ Mount hiding bypassed
        └─ Stat spoofing bypassed
```

**Kernel Integration:** `zeromount_is_uid_blocked()` exported via `EXPORT_SYMBOL`, called by SUSFS at 3 check points in `fs/susfs.c`.

---

## Session Tracking

- `.claude/progress.json` — Current phase
- `.claude/features.json` — Feature status
- `.claude/session-roundup.md` — Session summaries
- `.claude/plans/iridescent-sniffing-cook.md` — Active cherry-pick plan

**Current State:** v3.2.0 + upstream cherry-pick in progress (Phase A complete, Phase B pending).

---

## Upstream Cherry-Pick Status (IN PROGRESS)

**Upstream remote:** `upstream` → `https://github.com/maxsteeel/nomount.git`
- `upstream/master` — 3 commits since fork (refresh ioctl, wildcard CLI, CI)
- `upstream/experimental` — 10 commits (stat/statfs spoofing, workqueue, nm_enter/nm_exit, mega update c82104c)

**Checkpoint commit:** `22ba297` (pre-cherry-pick state)
**Phase A commit:** `01d55a0` (foundation safety improvements)

### Completed (Phase A)
| Task | What Changed |
|------|-------------|
| #1 Fix recursion guard | `zm_enter()`/`zm_exit()`/`zm_is_recursive()` in zeromount.h, EXPORT_PER_CPU_SYMBOL, replaced racy this_cpu_inc_return in getname_hook |
| #3 flush_parent() | New function: inode_lock + lookup_one_len + d_invalidate + d_drop for targeted parent dentry invalidation |
| #5 WRITE_ONCE/READ_ONCE | zm_ino_adb/zm_ino_modules, rule->is_new data race fix |

### Pending (Phase B — next session)
| Task | What To Do |
|------|-----------|
| #2 zeromount_should_skip() | Unified safety check replacing ZEROMOUNT_DISABLED(). **DO NOT use in is_uid_blocked** (SUSFS API contract). Adds: PF_KTHREAD, PF_EXITING, !current->mm, !nsproxy, PF_MEMALLOC_NOFS, in_interrupt/nmi, oops_in_progress |
| #4 Enhance flush_dcache | Add d_drop before d_invalidate, ENOENT fallback via flush_parent, zm_enter/zm_exit wrapping (fixes latent bug: kern_path redirects and flushes wrong dentry) |
| #6 force_refresh_all() | Safe copy-paths-then-flush pattern. **NOT upstream's list_cut_position** (has concurrency bug) |
| #7 IOC_REFRESH ioctl | `_IO(ZEROMOUNT_IOC_MAGIC, 10)` — number 10 because 8=ENABLE, 9=DISABLE |
| #8 metamount.sh refresh | `"$LOADER" refresh >/dev/null 2>&1 &` after zm enable |
| #9 Validation | 2 parallel review agents (safety + correctness) |

### Upstream Bugs — DO NOT IMPORT
1. `nomount_ioctl_del_uid`: `kfree()` instead of `kfree_rcu()` (use-after-free)
2. `nomount_resolve_path`: Returns interior RCU pointer (use-after-free)
3. Missing `capable(CAP_SYS_ADMIN)` in ioctl dispatcher (security regression)
4. `nomount_force_refresh_all`: `list_cut_position` removes rules from live list (breaks concurrent access)

### Not Cherry-Picked (Intentional)
- stat spoofing → SUSFS kstat_redirect covers (passes both vpath+rpath)
- mmap metadata → SUSFS sus_map hides entries entirely
- nm_enter/nm_exit everywhere → Only getname_hook + flush_dcache have reentry risk
- Delayed workqueue → Our explicit zm enable is more deterministic
- Named critical process list → PF_KTHREAD + !current->mm covers reliably
- static_vpath no-alloc → Premature optimization without stat hooks

### Detection Gaps to Investigate (Future)
- `/proc/misc` leaks "zeromount" device entry
- SUSFS kstat_redirect timing: verify redirected inode gets kstat coverage on-device
- SUSFS sus_map timing: verify redirected library mmaps are hidden
- Missing do_proc_readlink hook (upstream has it, we rely on d_path hook)
