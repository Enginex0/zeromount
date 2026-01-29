# Goal
**Project: ZeroMount** (renamed from NoMount, 2026-01-28)

## One-Sentence Summary

A KernelSU metamodule that makes module files accessible via VFS path redirection with zero filesystem mounts.

---

## Success Criteria

- [ ] Modules function correctly (files accessible at expected system paths)
- [ ] Zero entries in /proc/mounts from module activity
- [ ] /dev/zeromount device hidden from non-root detection
- [ ] SUSFS integration: kstat spoofed, paths hidden, maps hidden
- [ ] Kernel patch passes all bug fixes from audit (RCU, compat getdents, inode collision)
- [ ] metamount.sh follows metamodule contract (notify-module-mounted, skip_mount, disable)

---

## Explicitly Out of Scope

- NOT replacing SUSFS (we couple with it, not replace it)
- NOT supporting Magisk (KernelSU metamodule only for v1)
- NOT handling overlay/bind mount hijacking (we ARE the metamodule, no mounts to hijack)
- NOT building a WebUI (CLI-only for v1)

---

## Why This Matters

Overlay/bind mounts are detectable by banking apps via /proc/mounts and /proc/self/mountinfo. VFS path redirection operates below the mount layer, leaving zero mount evidence. Combined with SUSFS kstat spoofing, this creates an undetectable module system.
