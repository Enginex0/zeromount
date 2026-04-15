<p align="center">
  <h1 align="center">👻 ZeroMount</h1>
  <p align="center"><b>Mountless Module Loading for Rooted Android</b></p>
  <p align="center">Your modules. Stock mount tables. Zero traces.</p>
  <p align="center">
    <img src="https://img.shields.io/badge/status-beta-orange?style=for-the-badge" alt="Beta">
    <img src="https://img.shields.io/badge/KernelSU-supported-green?style=for-the-badge&logo=android" alt="KernelSU">
    <img src="https://img.shields.io/badge/Telegram-community-blue?style=for-the-badge&logo=telegram" alt="Telegram">
  </p>
  <p align="center">
    English | 简体中文 | 繁體中文 | Türkçe | Português (Brasil) | 한국어 | Français | Bahasa Indonesia | Русский | Українська | ภาษาไทย | Tiếng Việt | Italiano | Polski | Български | 日本語 | Español | العربية | हिन्दी | Deutsch | Nederlands | Ελληνικά | Svenska | Norsk | Dansk
  </p>
</p>

---

> [!WARNING]
> **ZeroMount is in beta.** Core functionality is tested end-to-end, but edge cases are expected across different devices, ROMs, and kernels. If something breaks, [open an issue](https://github.com/Enginex0/zeromount/issues).

---

## What is ZeroMount?

ZeroMount is a **mount orchestration engine** for rooted Android. It takes over your entire module loading pipeline — detecting kernel capabilities, scanning modules, planning mount strategies, executing with automatic fallback, applying stealth protections, and monitoring system health — all in a single coordinated boot sequence. The goal: load every module with zero trace.

Traditional root modules use bind mounts or OverlayFS, which leave entries in `/proc/mounts` and `/proc/self/mountinfo` that detection apps find. ZeroMount's primary engine works at a lower level: it intercepts the kernel's VFS layer at `getname()`, redirecting file paths *before* the filesystem even processes them. Module files appear at stock system paths. Mount tables stay clean. Detection apps see a stock device.

When the VFS driver isn't available, ZeroMount doesn't stop — it cascades through OverlayFS and MagicMount automatically, applying SUSFS stealth layers on top to hide whatever traces remain. It coordinates with peer modules, reconciles external SUSFS configurations, and guards against bootloops with multi-stage health monitoring. Every phase is orchestrated, every fallback is planned.

> **This is not a port of NoMount.** ZeroMount shares the same goal — kernel-level VFS redirection without mount pollution — but the architecture is entirely different in every layer: a custom kernel driver, a Rust userspace binary, SUSFS integration, a WebUI, and a multi-phase boot pipeline. Built from scratch.

---

## Features

### Mount Engine
- **VFS path redirection** — module files load at stock system paths, zero mount table entries
- **3 mount strategies** — VFS (primary) → OverlayFS (fallback) → MagicMount (last resort), auto-selected based on kernel capabilities
- **Per-module override** — force a specific mount strategy for individual modules
- **Directory entry injection** — module files appear in `ls` and `readdir` as stock
- **SELinux context injection** — redirected files carry correct labels, no AVC denials
- **statfs spoofing** — system partitions report expected EROFS/ext4 magic

### Stealth & Anti-Detection
- **SUSFS integration** — path hiding, kstat spoofing, mount hiding, `/proc/maps` hiding, AVC log spoofing — all toggleable from the WebUI
- **Property spoofing** — nukes or spoofs `ro.debuggable`, verified boot state, build fingerprint, serial number, and custom ROM markers via stealth `resetprop`
- **Uname & cmdline spoofing** — kernel release, version, and `/proc/cmdline` can match stock values
- **Process camouflage** — the ZeroMount binary appears as `[kworker/0:2]` in process listings
- **d_path & mmap clean** — `/proc/PID/maps` and fd symlinks show unmodified metadata

### Module Management
- **Auto-scan** — discovers and loads all active modules from `/data/adb/modules/` at boot
- **Hot load / unload** — add or remove module VFS rules at runtime without rebooting
- **Module exclusions** — blacklist specific modules from loading entirely
- **App exclusions** — exclude specific apps (by UID) from seeing redirected files
- **Conflict resolution** — detects overlapping files across modules, last-installed wins
- **Peer orchestration** — intercepts installs and uninstalls of other modules to maintain VFS compatibility

### Safety & Recovery
- **Bootloop guard** — monitors boot timeout, zygote stability, and SystemUI health; auto-disables the module and reboots on crash loops
- **Volume-key safe mode** — hold both volume keys during boot for immediate recovery
- **Config backup & rollback** — config is backed up before every boot pipeline and auto-restored on failure
- **Recovery lockout** — after a guard trigger, ZeroMount stays disabled until explicitly re-enabled

### WebUI
- **Dashboard** — engine status, active rule count, hidden paths/maps count, capabilities, activity log
- **Module manager** — scan, hot-load, and hot-unload modules
- **App exclusions** — searchable per-app VFS bypass with one-tap exclude/include
- **Module exclusions** — prevent specific modules from loading
- **SUSFS panel** — 20+ toggles for path hiding, kstat, maps, mounts, uname, cmdline, and more
- **Settings** — mount strategy, storage backend, property spoofing, guard thresholds, performance tuner
- **Theming** — dark, light, AMOLED, system-auto; 6 accent colors or system color; adjustable glass effect
- **Config export / import** — full backup and restore of your configuration

### Extras
- **Custom emoji fonts** — replace system emoji with NotoColorEmoji, with per-app injection for Facebook, GBoard, and GMS
- **Performance tuner** — optional CPU governor tuning with input-boost daemon
- **ADB root** — root shell in ADB via Axon injection, no global property changes
- **OTA updates** — in-manager updates via `updateJson`

---

## How It Works

ZeroMount runs a multi-phase pipeline on every boot:

1. **Detect** — probes the kernel for VFS driver, SUSFS capabilities, OverlayFS support, and storage backends. Writes a capability snapshot used by every phase that follows.
2. **Scan & Plan** — discovers all modules in `/data/adb/modules/`, classifies every file, resolves cross-module conflicts, and generates a per-partition mount plan.
3. **Execute** — selects the optimal mount strategy based on detected capabilities and applies it. Falls back automatically if a strategy fails — even per-module.
4. **Stealth** — applies SUSFS protections: path hiding, kstat spoofing, maps hiding, mount hiding, AVC log spoofing, property spoofing, uname/cmdline spoofing. Layers stack based on what the kernel supports.
5. **Guard** — spawns health monitors for boot timeout, zygote stability, and SystemUI. If any crash-loops, ZeroMount auto-disables itself and reboots to a safe state.

### Mount Strategies

| Strategy | Method | Mount table trace | Requires |
|---|---|---|---|
| **VFS** | Per-file redirection rules injected into a custom kernel driver (`/dev/zeromount`). Paths resolve before the filesystem sees them. | **None** | ZeroMount kernel driver |
| **OverlayFS** | Per-partition overlay mounts with staged lower directories. | Visible (SUSFS can hide) | OverlayFS kernel support |
| **MagicMount** | Individual bind mounts per file. | Visible | Nothing (always available) |

The cascade is automatic: VFS if the driver exists → OverlayFS if not → MagicMount as last resort. You can override the strategy globally or per-module from the WebUI.

---

## Requirements

> [!IMPORTANT]
> The VFS engine requires a **custom kernel** with the ZeroMount driver and SUSFS patches. Without it, ZeroMount still works via OverlayFS or MagicMount — but you won't get mountless redirection.

1. Rooted Android device with an unlocked bootloader
2. A supported root manager (KernelSU, APatch, or Magisk)
3. A kernel with ZeroMount + SUSFS patches → **[Super-Builders](https://github.com/Enginex0/Super-Builders)**

---

## Compatibility

### Tested Kernels

| Android Version | Kernel | Status |
|---|---|---|
| Android 12 | 5.10.209 | ✅ Tested |
| Android 15 | 6.6.66 | ✅ Tested |

### Root Managers

| Manager | Status | Notes |
|---|---|---|
| KernelSU | ✅ Tested | Full metamodule support |
| APatch | ⚠️ Untested | Metamodule hooks present but not verified |
| Magisk | ⚠️ Untested | Fallback pipeline exists but not verified on device |

> Tested on an unlisted combo? Let us know.

---

## Quick Start

1. **Build or download a kernel** with ZeroMount + SUSFS patches from [Super-Builders](https://github.com/Enginex0/Super-Builders)
2. **Flash the kernel** to your device
3. **Install ZeroMount** — download the ZIP and install via your root manager
4. **Reboot**
5. **Open the WebUI** — root manager → ZeroMount → ⚙️

The dashboard shows engine status, detected capabilities, loaded modules, and everything is configurable from the Settings tab.

---

## Community

```bash
$ zeromount --connect

 ██████╗ ██████╗ ███╗   ██╗███╗   ██╗███████╗ ██████╗████████╗
██╔════╝██╔═══██╗████╗  ██║████╗  ██║██╔════╝██╔════╝╚══██╔══╝
██║     ██║   ██║██╔██╗ ██║██╔██╗ ██║█████╗  ██║        ██║
██║     ██║   ██║██║╚██╗██║██║╚██╗██║██╔══╝  ██║        ██║
╚██████╗╚██████╔╝██║ ╚████║██║ ╚████║███████╗╚██████╗   ██║
 ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝╚═╝  ╚═══╝╚══════╝ ╚═════╝   ╚═╝

 [✓] SIGNAL    ──→  t.me/superpowers9
 [✓] CHANNEL   ──→  t.me/superpowers99
 [✓] UPLINK    ──→  kernel builds · bug triage · feature drops
 [✓] STATUS    ──→  OPEN — all operators welcome
```

<p align="center">
  <a href="https://t.me/superpowers9">
    <img src="https://img.shields.io/badge/⚡_GROUP-SuperPowers_Telegram-black?style=for-the-badge&logo=telegram&logoColor=cyan&labelColor=0d1117&color=00d4ff" alt="Telegram Group">
  </a>
  &nbsp;
  <a href="https://t.me/superpowers99">
    <img src="https://img.shields.io/badge/📢_CHANNEL-SuperPowers_Updates-black?style=for-the-badge&logo=telegram&logoColor=cyan&labelColor=0d1117&color=00d4ff" alt="Telegram Channel">
  </a>
</p>

---

## 🙏 Credits

- **[NoMount](https://github.com/maxsteeel/nomount)** — the project that inspired ZeroMount's approach to mountless module loading
- **[BRENE](https://github.com/rrr333nnn333/BRENE)** — SUSFS automation
- **[Hybrid Mount](https://github.com/Hybrid-Mount/meta-hybrid_mount)** by Hybrid Mount Org — metamodule architecture and frontend/backend structural design
- **[HymoFS](https://github.com/Anatdx/HymoFS)** by Anatdx — hybrid mounting and kernel-level path manipulation
- **[Mountify](https://github.com/backslashxx/mountify)** by backslashxx — mount solution and module management
- **[Magisk](https://github.com/topjohnwu/Magisk)** by topjohnwu — the root solution that started it all
- **[KernelSU](https://github.com/tiann/KernelSU)** by tiann — next-gen kernel root and module framework

---

## 📄 License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

---

<p align="center">
  <b>👻 GHOST mode — because the best mounts are the ones nobody can find.</b>
</p>
