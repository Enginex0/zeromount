# Changelog

## v2.0.161-dev

- Adopt resetprop-rs library for prop spoofing — eliminates subprocess forks per enforcement cycle, reads/writes property areas via direct mmap
- Add resetprop-rs as git submodule under `external/resetprop-rs`
- Add `resetprop-rs` CLI binary to module for shell-level hexpatch operations
- Remove `command -v resetprop` PATH guard from service.sh — prop-watch no longer depends on Magisk's resetprop binary
- Add explicit `[profile.dev]` to preserve full debug info and symbols in debug builds
- Add GitHub Actions build workflow with debug + release variants across all 4 Android ABIs
- Add Makefile for local build convenience
- Add `.gitignore`

## v2.0.160-dev

- Fix SAR partition promotion for bind-mounted partitions (product, system_ext, vendor, odm now visible in overlay and magic mount modes on devices where /system/product is a bind mount instead of a symlink)
- Fix kernel `auto_inject_parent` duplicate directory entries — skip registration when path already exists on real filesystem (patched across all KernelSU-Next, ReSukiSU, SukiSU-Ultra, WildKSU variants)
- Add VFS rule injection for novel directories and opaque-dir replacements (previously skipped, relying only on kernel-side readdir injection)
- Add hybrid overlay+VFS fallback — when overlay mounts fail on novel targets, VFS rules fill the gap
- Detect `.replace` sentinel files for opaque directory classification (previously only xattr-based detection)
- Add `module unload <id>` CLI command for hot-unloading modules without reboot
- Add `sync-description` CLI command for live KSU/APatch Manager description updates
- WebUI module unload now routes through CLI backend instead of manual VFS rule deletion
- Auto-resume normal guard operation after clean boot post-recovery

## v2.0.146-dev

- Detect and uninstall conflicting metamodules during installation

## v2.0.144-dev

- Change safe mode trigger from volume-down to volume-up + volume-down combo (fixes fastboot key conflict)
- Add persistent recovery lockout so guard recovery survives boot-completed cleanup
- Add `guard clear-lockout` CLI subcommand and WebUI lockout banner with clear button
- Add diagnostic kmsg logging when guard marker recording fails

## v2.0.143-dev

- Add i18n support for 36 languages in WebUI and daemon description strings

## v2.0.142-dev

- Initial release on zeromount repository
