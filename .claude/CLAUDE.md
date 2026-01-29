## Project Documentation Protocol

**This project uses structured documentation. Follow these instructions.**

---

### Session Start — READ THESE FIRST

```
1. .claude/handoff-2026-01-29-detection-bypass.md → LATEST: Kernel artifact hiding fix
2. .claude/handoff-2026-01-29-sucompat.md         → Sucompat stat hook fix (kernel)
3. docs/LEARNINGS.md                              → Key technical insights
4. docs/FOCUS.md                                  → Project status
```

**Current state:** Detection bypass COMPLETE. All 5 detection apps pass. Module v1.0.1 deployed.

---

### Project Context (ZeroMount v2)

This is a KernelSU metamodule that provides VFS-level path redirection **without any mounts**.

**Key identity:**
- Name: ZeroMount (renamed from NoMount)
- Binary: `zm` (was `nm`)
- Device: `/dev/zeromount` (was `/dev/nomount`)
- Config: `CONFIG_ZEROMOUNT` (was `CONFIG_NOMOUNT`)

**MVP Status:** All features completed. Working on detection bypass improvements.

---

### Critical Warnings (UPDATED 2026-01-29)

**DO NOT:**
1. Modify kernel susfs.c decode blocks (encode/decode is CORRECT and symmetric)
2. Test SUSFS spoofing from root shell (only works for umounted apps)
3. Assume file-based fixes will solve daemon detection (different vector)
4. Re-implement overlayfs hooks (I6 DISCARDED — caused detection flood)
5. Use VFS-level stat hooks (conflicts with SUSFS)
6. Check for string existence with file-wide grep (check within function scope!)

**DO:**
- Read handoff-2026-01-29-detection-bypass.md before starting
- Test with actual detection apps, not root shell
- Check docs/LEARNINGS.md for technical insights
- Verify SUSFS hiding: `dmesg | grep sus_path_loop`
- Remember: adb shell can still see hidden paths (not in umount list)

---

### Current Issue (This Session)

**Sucompat stat() hook was missing umounted check**

- `ksu_handle_execveat_sucompat()` — has check ✓
- `ksu_handle_faccessat()` — has check ✓
- `ksu_handle_stat()` — **WAS MISSING** ✗ → Fixed

**Build pending:** 21467986635
**Watch:** https://github.com/Enginex0/kernelsu-next-vanilla/actions/runs/21467986635

---

### Detection Status

| Detector | Before Fix | After Fix |
|----------|------------|-----------|
| Native Detector | ✓ PASS | ✓ PASS |
| Native Test | ✓ PASS | ✓ PASS |
| Holmes | ✓ PASS | ✓ PASS |
| Disclosure | ✗ sucompat SCA | ? (pending) |
| Garuda Defender | ✗ Found su binary | ? (pending) |

---

### File Reference

| File | Purpose |
|------|---------|
| `.claude/handoff-2026-01-29-sucompat.md` | **START HERE** — Sucompat fix details |
| `.claude/handoff-2026-01-29.md` | Previous handoff (daemonScan issue) |
| `docs/LEARNINGS.md` | Technical insights (sucompat entry added) |
| `docs/DECISION-GATE.md` | I3 stat hook decisions |
| `docs/FOCUS.md` | Project status |

---

### Key Paths

```
Module:        /home/claudetest/zero-mount/nomount/module/
Device:        /data/adb/modules/zeromount/
Build Workflow:/home/claudetest/gki-build/kernelsu-next-vanilla/.github/workflows/build.yml
SUSFS:         /data/adb/ksu/bin/ksu_susfs
Old Patches:   /home/claudetest/gki-build/nomount-vfs-clone/patches/
```

---

### Key Commits (kernelsu-next-vanilla)

| Commit | Description |
|--------|-------------|
| `22938fe` | First fix attempt (buggy — checked file-wide) |
| `b4d27bb` | Fixed detection (checks within function scope) |

---

### Rules

1. **Read handoff first** — Contains critical context and warnings
2. **Evidence over assumptions** — Trace full data paths before "fixing"
3. **Test with real apps** — Root shell won't show spoofed values
4. **Document learnings** — Update LEARNINGS.md with insights
5. **Check function scope** — Don't grep for string existence file-wide
