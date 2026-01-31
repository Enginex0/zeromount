# Zero-Mount Project Instructions

## Session Start Protocol

**READ THE PROJECT DATABASE FIRST:**

```
/home/claudetest/.claude/project-database/
├── index.md              ← START HERE (navigation, comparison, tasks)
├── susfs-reference.md    ← SUSFS kernel hiding mechanisms
├── zeromount-reference.md← ZeroMount VFS redirection
├── function-index.json   ← 129 functions indexed (kernel, shell, WebUI)
└── integration-map.md    ← Cross-project integration
```

**Quick Start:**
```bash
# Navigate the database
cat /home/claudetest/.claude/project-database/index.md

# Function lookup
jq '.zeromount_kernel.zeromount_is_uid_blocked' /home/claudetest/.claude/project-database/function-index.json
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
Project database:/home/claudetest/.claude/project-database/
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

**Current State:** v3.2.0 shipped, WebUI instant load via daemon cache.
