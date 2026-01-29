# Handoff: Detection Bypass Hardening Complete

**Date:** 2026-01-29
**Session:** Detection bypass investigation and fix

---

## What Was Done

### 1. Identified ZeroMount Kernel Artifact Exposure

**Problem:** Detection apps (crackme) could identify ZeroMount presence even with no rules configured.

**Root cause:** Two kernel artifacts were visible to all processes:
- `/dev/zeromount` (char device, mode 0600)
- `/sys/kernel/zeromount/` (sysfs kobject, mode 0755)

Detection apps used `stat()` on these paths — success = ZeroMount present.

### 2. Implemented Fix

**Solution:** Use SUSFS `sus_path_loop` to hide both paths from non-root apps.

**Changes:**
- `module/service.sh` — Added SUSFS sus_path_loop calls on boot
- `releases/zeromount-v1.0.1.zip` — Updated module package

**Code added to service.sh:**
```bash
if [ -x "$SUSFS_BIN" ]; then
    "$SUSFS_BIN" add_sus_path_loop /dev/zeromount 2>/dev/null
    "$SUSFS_BIN" add_sus_path_loop /sys/kernel/zeromount 2>/dev/null
fi
```

### 3. Verified Fix

**dmesg confirmation:**
```
susfs:[0][13709][susfs_run_sus_path_loop] re-flag '/dev/zeromount' as SUS_PATH for uid: 99039
susfs:[0][13709][susfs_run_sus_path_loop] re-flag '/sys/kernel/zeromount' as SUS_PATH for uid: 99039
```

**Detection apps now pass:** crackme, Disclosure, Holmes, Native Test, Native Detector

---

## Previous Session Issues (Also Investigated)

### Sucompat stat() Side-Channel

**Problem:** `ksu_handle_stat()` missing `susfs_is_current_proc_umounted()` check.

**Fix attempted:** Added check to build.yml workflow, but awk patterns didn't match actual code structure.

**Current status:** Fix script corrected in build.yml, awaiting verification in next kernel build.

**Build with corrected fix:** 21469051200 (check status)

---

## Current State

| Component | Status |
|-----------|--------|
| ZeroMount module | v1.0.1 with artifact hiding |
| Detection bypass | ✓ All 5 apps pass |
| Kernel sucompat fix | Pending build verification |

---

## Next Session Actions

1. **Verify sucompat kernel fix** — Check build 21469051200 logs
2. **Flash new kernel** — If sucompat fix applied correctly
3. **Test without umount list** — Verify sucompat fix works independently

---

## Key Files

| File | Purpose |
|------|---------|
| `module/service.sh` | Hides kernel artifacts on boot |
| `.github/workflows/build.yml` | Sucompat stat() fix (in kernelsu-next-vanilla repo) |
| `docs/LEARNINGS.md` | Technical insights documented |
| `docs/FOCUS.md` | Project status updated |

---

## Commits This Session

| Repo | Commit | Description |
|------|--------|-------------|
| Enginex0/zeromount | 6209300 | Fix ZeroMount detection: hide kernel artifacts via SUSFS |
| Enginex0/kernelsu-next-vanilla | 62d5333 | Fix sucompat stat hook patch - correct awk patterns |
