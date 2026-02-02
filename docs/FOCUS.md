# Focus Protocol

> **Rule: ONE thing until DONE or explicitly ABANDONED with reason.**

---

## CURRENT FOCUS

**Phase: WEBUI V2 DEVELOPMENT** 🔄

### Active Task

WebUI v2 iteration — Enhance Beta (winner) with Gamma features.

### Latest Achievement (2026-01-29)
WebUI v2 Competition completed. Three architects (Alpha, Beta, Gamma) built competing designs.
**Winner: BETA — "The Bold Expressionist"** with Electric Sunrise gradients and spring physics.

### Next Steps
- Cherry-pick glassmorphism features from Gamma
- Polish animations and interactions
- Integrate with real zm backend (remove mocks)

---

## IDEA CAPTURE

| Date | Idea | After 7 Days? |
|------|------|---------------|
| 2026-01-28 | Batch mode in zm binary (process file list instead of one-at-a-time) | Waiting |
| 2026-01-28 | SELinux context copying for redirected files | Waiting |
| 2026-01-28 | Font/APK special handling via readdir injection | Waiting |
| 2026-01-29 | WebUI integration — Add ZeroMount status to KernelSU/SUSFS WebUI | Waiting |
| 2026-01-29 | Multi-version kernel support — Test on 5.15, 6.1, 6.6 kernels | Waiting |

---

## GRAVEYARD

### NoMount v1
- **Started:** 2026-01-27
- **Abandoned:** 2026-01-28
- **Days invested:** 1
- **Why abandoned:** Built without understanding metamodule architecture. service.sh grew to 1081 lines fighting the framework. Wrong execution timing forced 4-phase workaround. Universal mount hijacker was unnecessary complexity.
- **Pattern:** Started building before reading documentation.
- **Lesson:** /learn mode exists for a reason. Understanding saves 10x the debugging time.

### Kernel dev_t decode "fix"
- **Started:** 2026-01-29
- **Abandoned:** 2026-01-29
- **Days invested:** 0
- **Why abandoned:** Initial hypothesis was wrong. Kernel encode/decode roundtrip is symmetric and correct. Removing decode calls would have broken stat return values.
- **Pattern:** Assumed bug without tracing full data path.
- **Lesson:** Trace the COMPLETE flow before "fixing" suspicious code. The roundtrip was designed to be symmetric.

---

## COMPLETED

| Feature | Started | Completed | Days |
|---------|---------|-----------|------|
| Understanding phase (kernel + metamodule + userspace + SUSFS) | 2026-01-28 | 2026-01-28 | 1 |
| kernel-patch-v2 (Bug Fixes + Rename) | 2026-01-28 | 2026-01-28 | 1 |
| zm-binary (Freestanding C CLI) | 2026-01-28 | 2026-01-28 | 1 |
| metamodule-scripts (Shell Scripts) | 2026-01-28 | 2026-01-28 | 1 |
| susfs-integration (SUSFS Integration Module) | 2026-01-28 | 2026-01-28 | 1 |
| susfs-kernel-enhancements (kstat_redirect) | 2026-01-28 | 2026-01-29 | 2 |
| build-and-test (Build Pipeline + Testing) | 2026-01-28 | 2026-01-29 | 2 |
| script-extraction (Injection Scripts) | 2026-01-28 | 2026-01-28 | 1 |
| SUSFS fix investigation + deployment | 2026-01-29 | 2026-01-29 | 1 |
| ZeroMount kernel artifact hiding (detection bypass) | 2026-01-29 | 2026-01-29 | 1 |
| **WebUI v2 Competition (3 architects)** | 2026-01-29 | 2026-01-29 | 1 |

---

## STATS

**Completed:** 11 features
**Abandoned:** 2 (NoMount v1, kernel decode "fix")
**Completion Rate:** 85%

**Goal:** 80%+ completion rate ✓ **ACHIEVED**

---

## VERIFICATION

All 5 detection apps pass:
- Native Detector (Reveny): ✓ "The Environment is normal"
- Native Test (icu.nullptr): ✓ "Normal"
- Holmes: ✓ "Normal"
- Disclosure (milltina): ✓ "No traces were found"
- Crackme (kikyps): ✓ Pass (after kernel artifact hiding)

**ZeroMount + SUSFS integration verified working.**

### Detection Vectors Closed (2026-01-29)
| Vector | Fix |
|--------|-----|
| `/dev/zeromount` visible | SUSFS sus_path_loop hides from umounted apps |
| `/sys/kernel/zeromount/` visible | SUSFS sus_path_loop hides from umounted apps |
| Sucompat stat() side-channel | Kernel fix: `susfs_is_current_proc_umounted()` check |
