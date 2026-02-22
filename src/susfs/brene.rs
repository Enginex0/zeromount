use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use tracing::{debug, error, info, warn};

use crate::core::config::{UnameConfig, UnameMode, ZeroMountConfig};
use crate::core::types::SusfsMode;
use crate::utils::command::{run_command_with_timeout, CMD_TIMEOUT};
use super::SusfsClient;
use super::fonts;
use super::kstat;
use super::paths;

// S05: Mount hiding is NEVER invoked. It causes LSPosed module failure
// because hidden mounts become invisible to processes that need them.
// The correct approach is VFS redirection which avoids mounts entirely.

// Paths for auto-hide toggles
const ROOTED_FOLDER_PATHS: &[&str] = &[
    "/data/adb",
    "/data/adb/modules",
    "/data/adb/ksu",
    "/data/adb/ap",
    "/data/adb/magisk",
    "/sbin/.magisk",
    "/cache/magisk.log",
    "/data/cache/magisk.log",
];

const RECOVERY_PATHS: &[&str] = &[
    "/cache/recovery",
    "/data/cache/recovery",
];

const TMP_PATHS: &[&str] = &[
    "/data/local/tmp",
];

// Zygisk .so patterns injected into /proc/PID/maps
const ZYGISK_MAP_PATTERNS: &[&str] = &[
    "/data/adb/modules/zygisksu/lib",
    "/data/adb/modules/shamiko/lib",
    "/data/adb/ksu/bin/zygisk",
    "/data/adb/ap/bin/zygisk",
    "libzygisk",
    "zygisk.so",
];

const SYSTEM_FONTS_DIR: &str = "/system/fonts";
const MODULES_DIR: &str = "/data/adb/modules";
const FONT_STAGING_DIR: &str = "/data/adb/zeromount/fonts";

const DEX2OAT_UMOUNT_PATHS: &[&str] = &[
    "/system/apex/com.android.art/bin/dex2oat",
    "/system/apex/com.android.art/bin/dex2oat32",
    "/system/apex/com.android.art/bin/dex2oat64",
    "/apex/com.android.art/bin/dex2oat",
    "/apex/com.android.art/bin/dex2oat32",
    "/apex/com.android.art/bin/dex2oat64",
];

// Custom ROM artifacts that reveal recovery/OTA infrastructure
const ROM_SUS_PATHS: &[&str] = &[
    "/system/addon.d",
    "/vendor/bin/install-recovery.sh",
    "/system/bin/install-recovery.sh",
    "/system/vendor/bin/install-recovery.sh",
];

const SDCARD_ROOTED_APP_FOLDERS: &[&str] = &[
    "MT2",
    "OhMyFont",
    "AppManager",
    "DataBackup",
    "Android/fas-rs",
];

const SDCARD_RECOVERY_FOLDERS: &[&str] = &[
    "Fox",
    "PBRP",
    "TWRP",
];


/// Summary of all BRENE operations applied during a boot cycle.
#[derive(Debug, Default)]
pub struct BreneResult {
    pub paths_hidden: u32,
    pub maps_hidden: u32,
    pub font_modules: Vec<FontModuleInfo>,
    pub emoji_applied: bool,
    pub uname_spoofed: bool,
    pub avc_spoofed: bool,
    pub log_enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FontModuleInfo {
    pub id: String,
    pub redirect_count: u32,
}

pub fn apply_brene(client: &SusfsClient, config: &ZeroMountConfig, fonts_overlay_mounted: bool, _susfs_mode: SusfsMode) -> Result<BreneResult> {
    let mut result = BreneResult::default();

    if !client.is_available() {
        debug!("SUSFS unavailable, skipping BRENE application");
        return Ok(result);
    }

    let brene = &config.brene;
    let susfs_cfg = &config.susfs;

    // Effective feature flags: kernel capability AND user config sub-toggle
    let has_path = client.features().path && susfs_cfg.path_hide;
    let has_maps = client.features().maps && susfs_cfg.maps_hide;
    let _has_open_redirect = client.features().open_redirect && susfs_cfg.open_redirect;
    let _has_kstat = client.features().kstat && susfs_cfg.kstat;

    // ROM artifacts are hidden unconditionally — mirrors post-fs-data.sh behavior
    if has_path {
        let count = paths::hide_paths(client, ROM_SUS_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: ROM sus paths hidden ({count})");
    }

    if brene.auto_hide_rooted_folders && has_path {
        let count = paths::hide_paths(client, ROOTED_FOLDER_PATHS).unwrap_or(0);
        let sdcard_count = hide_sdcard_rooted_folders(client).unwrap_or(0);
        result.paths_hidden += count + sdcard_count;
        info!("BRENE: rooted folders hidden ({count} infra + {sdcard_count} sdcard)");
    }

    if brene.auto_hide_recovery && has_path {
        let count = paths::hide_paths(client, RECOVERY_PATHS).unwrap_or(0);
        let sdcard_count = hide_sdcard_recovery_folders(client).unwrap_or(0);
        result.paths_hidden += count + sdcard_count;
        info!("BRENE: recovery paths hidden ({count} cache + {sdcard_count} sdcard)");
    }

    if brene.auto_hide_tmp && has_path {
        let count = paths::hide_dir_children_loop(client, TMP_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: tmp children hidden via path_loop ({count})");
    }

    if brene.auto_hide_sdcard_data && has_path {
        let count = hide_sdcard_data(client).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: sdcard android/data hidden ({count})");
    }

    if brene.auto_hide_apk && has_path {
        let count = hide_apk_paths(client);
        result.paths_hidden += count;
        info!("BRENE: APK paths hidden ({count})");
    }

    // -- Maps hiding --

    if brene.auto_hide_zygisk && has_maps {
        let pattern_count = paths::hide_maps(client, ZYGISK_MAP_PATTERNS).unwrap_or(0);
        let dynamic_count = hide_zygisk_modules_dynamic(client).unwrap_or(0);
        result.maps_hidden += pattern_count + dynamic_count;
        info!("BRENE: zygisk hidden ({pattern_count} patterns + {dynamic_count} dynamic)");
    }

    if brene.auto_hide_injections && has_maps {
        let count = hide_module_injections(client).unwrap_or(0);
        result.maps_hidden += count;
        info!("BRENE: module injection files hidden ({count})");
    }

    // -- Font redirect --
    // Overlay mode: kstat + path_hide only (overlay serves files, open_redirect would conflict)
    // Non-overlay mode: bind mount + full SUSFS redirect

    if brene.auto_hide_fonts {
        let fonts = if fonts_overlay_mounted {
            hide_font_modules_overlay(client)
        } else {
            process_font_modules(client, &config.mount.overlay_source)
        };
        info!("BRENE: processed {} font modules (overlay={}): {:?}", fonts.len(), fonts_overlay_mounted, fonts);
        result.font_modules = fonts;
    }

    // -- Emoji font replacement --
    if config.emoji.enabled {
        info!("BRENE: emoji toggle ON, checking conflicts");
        if let Some(conflict_id) = super::emoji::check_emoji_font_conflict(&result.font_modules) {
            warn!("BRENE: emoji SKIPPED — conflicting font module: {conflict_id}");
        } else {
            match super::emoji::apply_emoji_fonts(client, &config.mount.overlay_source, fonts_overlay_mounted) {
                Ok(er) => {
                    info!("BRENE: emoji applied — strategy={}, mounts={}, redirects={}, vfs={}",
                          er.strategy, er.mounts, er.redirects, er.vfs_rules);
                    result.emoji_applied = true;
                }
                Err(e) => error!("BRENE: emoji mount failed: {e}"),
            }
        }
    } else {
        debug!("BRENE: emoji toggle OFF, skipping");
    }

    // -- Custom user-defined lists --

    if !brene.custom_sus_paths.is_empty() && has_path {
        let path_refs: Vec<&str> = brene.custom_sus_paths.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: custom sus_paths hidden ({count}/{})", path_refs.len());
    }

    if !brene.custom_sus_path_loops.is_empty() && has_path {
        let path_refs: Vec<&str> = brene.custom_sus_path_loops.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths_loop(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: custom sus_path_loops hidden ({count}/{})", path_refs.len());
    }

    if !brene.custom_sus_maps.is_empty() && has_maps {
        let path_refs: Vec<&str> = brene.custom_sus_maps.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_maps(client, &path_refs).unwrap_or(0);
        result.maps_hidden += count;
        info!("BRENE: custom sus_maps hidden ({count}/{})", path_refs.len());
    }

    // -- Hide sus mounts (kernel supercall — takes effect immediately) --

    match client.hide_sus_mounts(brene.hide_sus_mounts) {
        Ok(()) => info!("BRENE: hide_sus_mounts set to {}", brene.hide_sus_mounts),
        Err(e) => warn!("BRENE: hide_sus_mounts failed: {e}"),
    }

    // -- AVC log spoofing --

    if brene.avc_log_spoofing {
        match client.enable_avc_log_spoofing(true) {
            Ok(()) => {
                result.avc_spoofed = true;
                info!("BRENE: AVC log spoofing enabled");
            }
            Err(e) => warn!("BRENE: AVC log spoofing failed: {e}"),
        }
    }

    // -- SUSFS debug log toggle --

    if brene.susfs_log {
        match client.enable_log(true) {
            Ok(()) => {
                result.log_enabled = true;
                info!("BRENE: SUSFS logging enabled");
            }
            Err(e) => warn!("BRENE: SUSFS log enable failed: {e}"),
        }
    }

    // -- Cmdline spoofing (kernel supercall) --

    if brene.spoof_cmdline {
        apply_spoof_cmdline(client);
    }

    // -- Hide KSU loop devices --

    if brene.hide_ksu_loops && client.features().path {
        let count = hide_ksu_loop_devices(client);
        result.paths_hidden += count;
        if count > 0 {
            info!("BRENE: KSU loop devices hidden ({count})");
        }
    }

    // -- Force hide LSPosed (dex2oat umount via ksud) --

    if brene.force_hide_lsposed {
        apply_force_hide_lsposed();
    }

    // -- Uname spoofing --

    apply_uname(client, &config.uname, &mut result)?;

    info!(
        "BRENE complete: {} paths, {} maps, {} font modules, uname={}, avc={}",
        result.paths_hidden,
        result.maps_hidden,
        result.font_modules.len(),
        result.uname_spoofed,
        result.avc_spoofed,
    );

    Ok(result)
}

/// Discover and hide APK paths under /data/app/ for root-management packages.
/// These are the package directories that reveal a rooted device.
fn hide_apk_paths(client: &SusfsClient) -> u32 {
    let apk_dir = Path::new("/data/app");
    if !apk_dir.is_dir() {
        return 0;
    }

    // Known package prefixes that indicate root management
    let root_pkg_patterns = [
        "me.weishu.kernelsu",
        "io.github.vvb2060.magisk",
        "com.topjohnwu.magisk",
        "me.bmax.apatch",
        "org.lsposed.manager",
    ];

    let mut count = 0u32;
    let entries = match fs::read_dir(apk_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let matches = root_pkg_patterns.iter().any(|pat| name.contains(pat));
        if !matches {
            continue;
        }

        let path_str = entry.path().to_string_lossy().to_string();
        match client.add_sus_path(&path_str) {
            Ok(()) => count += 1,
            Err(e) => debug!("hide APK failed for {path_str}: {e}"),
        }
    }

    count
}

/// Enumerate children of /sdcard/Android/data and /storage/emulated/0/Android/data
/// and hide each with add_sus_path. Both mount paths need hiding — apps may resolve
/// either depending on whether they follow the /sdcard symlink or not.
fn hide_sdcard_data(client: &SusfsClient) -> Result<u32> {
    let paths = [
        "/sdcard/Android/data",
        "/storage/emulated/0/Android/data",
    ];
    let mut count = 0u32;
    for base in &paths {
        let dir = Path::new(base);
        if !dir.is_dir() {
            continue;
        }
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path_str = entry.path().to_string_lossy().to_string();
            match client.add_sus_path(&path_str) {
                Ok(()) => count += 1,
                Err(e) => debug!("sdcard data hide failed for {path_str}: {e}"),
            }
        }
    }
    Ok(count)
}

/// Walk /data/adb/modules/*/zygisk/ and hide all .so files dynamically.
/// Catches novel zygisk module names that ZYGISK_MAP_PATTERNS doesn't know about.
fn hide_zygisk_modules_dynamic(client: &SusfsClient) -> Result<u32> {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0u32;

    for module_entry in fs::read_dir(modules_dir)?.filter_map(|e| e.ok()) {
        let zygisk_dir = module_entry.path().join("zygisk");
        if !zygisk_dir.is_dir() {
            continue;
        }

        let dir_entries = match fs::read_dir(&zygisk_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in dir_entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_so = path.extension().and_then(|e| e.to_str()) == Some("so");
            if !is_so {
                continue;
            }
            let path_str = path.to_string_lossy().to_string();
            match client.add_sus_map(&path_str) {
                Ok(()) => count += 1,
                Err(e) => debug!("hide zygisk dynamic failed for {path_str}: {e}"),
            }
        }
    }

    Ok(count)
}

/// Walk /data/adb/modules/*/system/ and hide all files with a "." in the name.
/// Matches BRENE's `find -name "*.*"` filter — catches injected overlays in /proc/maps.
/// Skips our own module dir (meta-zeromount) to avoid self-hiding.
fn hide_module_injections(client: &SusfsClient) -> Result<u32> {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.is_dir() {
        return Ok(0);
    }

    let mut count = 0u32;

    for module_entry in fs::read_dir(modules_dir)?.filter_map(|e| e.ok()) {
        let module_path = module_entry.path();
        let module_name = match module_path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        // Don't self-hide — our own system/ files must remain visible to zeromount
        if module_name == "meta-zeromount" {
            continue;
        }

        let system_dir = module_path.join("system");
        if !system_dir.is_dir() {
            continue;
        }

        count += walk_system_dir_hide_dotfiles(client, &system_dir);
    }

    Ok(count)
}

fn walk_system_dir_hide_dotfiles(client: &SusfsClient, dir: &Path) -> u32 {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut count = 0u32;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            count += walk_system_dir_hide_dotfiles(client, &path);
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        // BRENE uses `-name "*.*"` — match any file with a dot in the name
        if !name.contains('.') {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();
        match client.add_sus_map(&path_str) {
            Ok(()) => count += 1,
            Err(e) => debug!("hide injection failed for {path_str}: {e}"),
        }
    }

    count
}

/// Scan /data/adb/modules/ for font modules. Bind-mount each font file
/// (kernel_umount=false keeps mounts in app namespaces, hide_sus_mounts
/// covers the traces), then layer SUSFS redirect + kstat on top.
fn process_font_modules(client: &SusfsClient, overlay_source: &str) -> Vec<FontModuleInfo> {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(modules_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut results = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let module_path = entry.path();
        if !module_path.is_dir() {
            continue;
        }

        if module_path.join("disable").exists() || module_path.join("remove").exists() {
            continue;
        }

        let font_dir = module_path.join("system/fonts");
        if !font_dir.is_dir() {
            continue;
        }

        let module_id = match module_path.file_name().and_then(|n| n.to_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        // KSU restorecon resets font module contexts to adb_data_file on
        // metamodule upgrade. Force system_file so bind mounts are readable
        // by system_server/zygote regardless of who creates them.
        fix_font_selinux_contexts(&font_dir);

        let bind_count = bind_mount_font_files(&font_dir, SYSTEM_FONTS_DIR);
        if bind_count > 0 {
            info!("font module '{module_id}': {bind_count} files bind-mounted");
        }

        match fonts::redirect_font_module(client, &module_id, &font_dir, SYSTEM_FONTS_DIR, overlay_source) {
            Ok(result) => {
                debug!(
                    "font module '{}': strategy={:?}, redirected={}",
                    module_id, result.strategy, result.redirect_count
                );
                results.push(FontModuleInfo {
                    id: module_id,
                    redirect_count: result.redirect_count as u32,
                });
            }
            Err(e) => warn!("font module '{module_id}' failed: {e}"),
        }
    }

    results
}

fn stage_font_file(source: &Path, filename: &str) -> Option<PathBuf> {
    let staging_dir = Path::new(FONT_STAGING_DIR);
    if !staging_dir.exists() {
        fs::create_dir_all(staging_dir).ok()?;
        fs::set_permissions(staging_dir, fs::Permissions::from_mode(0o700)).ok()?;
    }
    let dest = staging_dir.join(filename);
    fs::copy(source, &dest).ok()?;
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o644)).ok()?;
    Some(dest)
}

fn bind_mount_font_files(font_dir: &Path, system_font_dir: &str) -> u32 {
    let entries = match fs::read_dir(font_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut count = 0u32;
    for entry in entries.filter_map(|e| e.ok()) {
        let source = entry.path();
        if !source.is_file() {
            continue;
        }
        let filename = match source.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let target = format!("{}/{}", system_font_dir, filename);

        if !Path::new(&target).exists() {
            if let Some(staged) = stage_font_file(&source, filename) {
                crate::utils::selinux::set_selinux_context(
                    &staged, "u:object_r:system_file:s0"
                );
                if let Ok(driver) = crate::vfs::VfsDriver::open() {
                    match driver.add_rule(&staged, Path::new(&target), false) {
                        Ok(()) => debug!("font VFS rule: {filename} (staged)"),
                        Err(e) => debug!("font VFS rule failed for {filename}: {e}"),
                    }
                }
            }
            continue;
        }

        // Bind mount exposes source permissions — must be world-readable for system_server/zygote
        let _ = fs::set_permissions(&source, fs::Permissions::from_mode(0o644));
        crate::utils::selinux::set_selinux_context(&source, "u:object_r:system_file:s0");

        let c_src = match std::ffi::CString::new(source.as_os_str().as_encoded_bytes()) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let c_tgt = match std::ffi::CString::new(target.as_bytes()) {
            Ok(s) => s,
            Err(_) => continue,
        };

        // SAFETY: CStrings are non-null NUL-terminated; null pointers for unused mount(2) args are valid.
        let ret = unsafe {
            libc::mount(
                c_src.as_ptr(),
                c_tgt.as_ptr(),
                std::ptr::null(),
                libc::MS_BIND,
                std::ptr::null(),
            )
        };
        if ret == 0 {
            count += 1;
        } else {
            let err = std::io::Error::last_os_error();
            debug!("font bind mount failed for {filename}: {err}");
        }
    }
    count
}

fn fix_font_selinux_contexts(font_dir: &Path) {
    let entries = match fs::read_dir(font_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let context = b"u:object_r:system_file:s0\0";
    let attr = b"security.selinux\0";
    let mut fixed = 0u32;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let c_path = match std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let ret = unsafe {
            libc::setxattr(
                c_path.as_ptr(),
                attr.as_ptr() as *const libc::c_char,
                context.as_ptr() as *const libc::c_void,
                context.len() - 1,
                0,
            )
        };

        if ret == 0 {
            fixed += 1;
        } else {
            let err = std::io::Error::last_os_error();
            warn!("setxattr system_file failed for {}: {err}", path.display());
        }
    }

    if fixed > 0 {
        info!("fixed SELinux context on {fixed} font files");
    }
}

/// Overlay-mode font hiding: kstat + path_hide only, no open_redirect.
/// When overlay already mounted /system/fonts, SUSFS open_redirect would conflict.
fn hide_font_modules_overlay(client: &SusfsClient) -> Vec<FontModuleInfo> {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.is_dir() {
        return Vec::new();
    }

    let entries = match fs::read_dir(modules_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    // bind mounts don't affect the parent dir — stat it for stock erofs dev
    let stock_dev = fs::metadata(SYSTEM_FONTS_DIR).ok().map(|m| m.dev());
    let vfs_driver = crate::vfs::VfsDriver::open().ok();

    let mut results = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let module_path = entry.path();
        if !module_path.is_dir() { continue; }
        if module_path.join("disable").exists() || module_path.join("remove").exists() {
            continue;
        }

        let font_dir = module_path.join("system/fonts");
        if !font_dir.is_dir() { continue; }

        let module_id = match module_path.file_name().and_then(|n| n.to_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };

        let mut hidden_count = 0u32;
        let font_entries = match fs::read_dir(&font_dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for fe in font_entries.filter_map(|e| e.ok()) {
            let path = fe.path();
            if !path.is_file() { continue; }

            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };

            let target = format!("{}/{}", SYSTEM_FONTS_DIR, filename);
            let replacement = path.to_string_lossy().to_string();

            // KSU sets system_file during install, but upgrade reboots can lose it.
            // Explicit set (not copy from overlay — that's circular when context is wrong).
            crate::utils::selinux::set_selinux_context(
                Path::new(&replacement), "u:object_r:system_file:s0"
            );

            if client.features().kstat {
                match kstat::build_kstat_values_from_paths(&target, &replacement) {
                    Ok(mut spoof) => {
                        if let Some(dev) = stock_dev {
                            spoof.dev = Some(dev);
                            let hash: u64 = target.bytes().fold(0xcbf29ce484222325u64, |h, b| {
                                (h ^ b as u64).wrapping_mul(0x100000001b3)
                            });
                            spoof.ino = Some(hash % 2_147_483_647);
                        }
                        if let Err(e) = client.add_sus_kstat_redirect(&target, &replacement, &spoof) {
                            debug!("overlay font kstat failed for {filename}: {e}");
                        }
                    }
                    Err(e) => debug!("overlay font kstat build failed for {filename}: {e}"),
                }
            }

            if client.features().path {
                if let Err(e) = client.add_sus_path(&replacement) {
                    debug!("overlay font path hide failed for {filename}: {e}");
                }
            }

            if let Some(ref driver) = vfs_driver {
                if let Some(staged) = stage_font_file(&path, filename) {
                    crate::utils::selinux::set_selinux_context(
                        &staged, "u:object_r:system_file:s0"
                    );
                    match driver.add_rule(&staged, Path::new(&target), false) {
                        Ok(()) => debug!("overlay font VFS rule: {filename} (staged)"),
                        Err(e) => debug!("overlay font VFS rule failed for {filename}: {e}"),
                    }
                }
            }

            hidden_count += 1;
        }

        if hidden_count > 0 {
            info!("font module '{}': overlay-mounted, {} files hidden via kstat+path", module_id, hidden_count);
        }

        results.push(FontModuleInfo { id: module_id, redirect_count: hidden_count });
    }
    results
}

/// Hide rooted-app sdcard folders: add_sus_path + add_sus_path_loop on /sdcard/ variant,
/// add_sus_path on /storage/emulated/0/ variant (matches BRENE boot-completed.sh:199-219).
fn hide_sdcard_rooted_folders(client: &SusfsClient) -> Result<u32> {
    if !client.is_available() || !client.features().path {
        return Ok(0);
    }

    let mut count = 0u32;
    for name in SDCARD_ROOTED_APP_FOLDERS {
        let sdcard = format!("/sdcard/{name}");
        let emulated = format!("/storage/emulated/0/{name}");

        if std::path::Path::new(&sdcard).exists() {
            if client.add_sus_path(&sdcard).is_ok() { count += 1; }
            let _ = client.add_sus_path_loop(&sdcard);
        }
        if std::path::Path::new(&emulated).exists() {
            if client.add_sus_path(&emulated).is_ok() { count += 1; }
        }
    }
    Ok(count)
}

/// Hide recovery sdcard folders with TWRP special case at /storage/emulated/TWRP
/// (matches BRENE boot-completed.sh:221-236).
fn hide_sdcard_recovery_folders(client: &SusfsClient) -> Result<u32> {
    if !client.is_available() || !client.features().path {
        return Ok(0);
    }

    let mut count = 0u32;
    for name in SDCARD_RECOVERY_FOLDERS {
        let sdcard = format!("/sdcard/{name}");
        let emulated = format!("/storage/emulated/0/{name}");

        if std::path::Path::new(&sdcard).exists() {
            if client.add_sus_path(&sdcard).is_ok() { count += 1; }
            let _ = client.add_sus_path_loop(&sdcard);
        }
        if std::path::Path::new(&emulated).exists() {
            if client.add_sus_path(&emulated).is_ok() { count += 1; }
        }
    }

    // TWRP also appears at /storage/emulated/TWRP (without /0/)
    let twrp_alt = "/storage/emulated/TWRP";
    if std::path::Path::new(twrp_alt).exists() {
        if client.add_sus_path(twrp_alt).is_ok() { count += 1; }
        let _ = client.add_sus_path_loop(twrp_alt);
    }

    Ok(count)
}

fn apply_uname(client: &SusfsClient, uname: &UnameConfig, result: &mut BreneResult) -> Result<()> {
    match uname.mode {
        UnameMode::Disabled => {}
        UnameMode::Static => {
            let release = if uname.release.is_empty() {
                "default"
            } else {
                &uname.release
            };
            let version = if uname.version.is_empty() {
                "default"
            } else {
                &uname.version
            };
            client.set_uname(release, version)?;
            result.uname_spoofed = true;
            info!("BRENE: uname spoofed (static: release={release}, version={version})");
        }
        UnameMode::Dynamic => {
            match build_dynamic_uname() {
                Ok((release, version)) => {
                    client.set_uname(&release, &version)?;
                    result.uname_spoofed = true;
                    info!("BRENE: uname spoofed (dynamic: release={release})");
                }
                Err(e) => warn!("BRENE: dynamic uname failed: {e}"),
            }
        }
    }
    Ok(())
}

/// Build sanitized uname values matching BRENE's uname2 mode.
/// Field-splits on '-', keeps only the base version + android suffix, then
/// strips known ROM/KSU markers. Pins version to a fixed timestamp so the
/// real build date is never exposed.
fn build_dynamic_uname() -> Result<(String, String)> {
    let raw = fs::read_to_string("/proc/version")
        .unwrap_or_else(|_| "Linux version 5.10.0".to_string());

    let parts: Vec<&str> = raw.splitn(4, ' ').collect();
    let kernel_release_raw = parts.get(2).unwrap_or(&"5.10.0").to_string();
    let lower = kernel_release_raw.to_lowercase();

    // Mirror BRENE's field-splitting: keep version + android suffix only
    let fields: Vec<&str> = lower.splitn(3, '-').collect();
    let base = fields.get(0).copied().unwrap_or("5.10.0");
    let release = if fields.get(1).map(|f| f.starts_with("android")).unwrap_or(false) {
        format!("{}-{}", base, fields[1])
    } else {
        base.to_string()
    };

    // Strip all known ROM/KSU build markers (existing + BRENE additions)
    let release = release
        .replace("sultan", "")
        .replace("lineage", "")
        .replace("wild", "")
        .replace("sukisu", "")
        .replace("ksu", "")
        .replace("🟢", "")
        .replace("✅", "")
        .replace("-susfs", "")
        .replace("-dirty", "")
        .replace("-custom", "")
        .replace("-gki", "");

    // Fixed timestamp — prevents leaking the real kernel build date
    let version = "#1 SMP PREEMPT Mon Jan 1 18:00:00 UTC 2010".to_string();

    let release = truncate_uname(&release);
    let version = truncate_uname(&version);

    Ok((release, version))
}



fn apply_force_hide_lsposed() {
    let ksud = if Path::new("/data/adb/ksu/bin/ksud").exists() {
        "/data/adb/ksu/bin/ksud"
    } else if Path::new("/data/adb/ap/bin/ksud").exists() {
        "/data/adb/ap/bin/ksud"
    } else {
        "ksud"
    };

    for path in DEX2OAT_UMOUNT_PATHS {
        if crate::utils::signal::shutdown_requested() {
            break;
        }
        match run_command_with_timeout(
            Command::new(ksud).args(["kernel", "umount", "add", path, "--flags", "2"]),
            CMD_TIMEOUT,
        ) {
            Ok(o) if o.status.success() => {
                debug!("dex2oat umount added: {path}");
            }
            Ok(o) => {
                debug!("dex2oat umount failed for {path} (exit {})", o.status.code().unwrap_or(-1));
            }
            Err(e) => {
                debug!("dex2oat umount failed for {path}: {e}");
            }
        }
    }
    info!("BRENE: force_hide_lsposed applied (6 dex2oat paths)");
}

fn apply_spoof_cmdline(client: &SusfsClient) {
    let cmdline_content = fs::read_to_string("/proc/cmdline").ok();
    let bootconfig_content = fs::read_to_string("/proc/bootconfig").ok();

    let (mut content, source) = if cmdline_content.as_ref().map_or(false, |c| c.contains("androidboot.verifiedbootstate")) {
        (cmdline_content.unwrap(), "cmdline")
    } else if bootconfig_content.is_some() {
        (bootconfig_content.unwrap(), "bootconfig")
    } else {
        warn!("BRENE: no cmdline or bootconfig available for spoofing");
        return;
    };

    content = content.replace("androidboot.verifiedbootstate=orange", "androidboot.verifiedbootstate=green");

    // Spoof hwname and hardware.sku to match ro.product.name
    if let Ok(output) = run_command_with_timeout(
        Command::new("getprop").arg("ro.product.name"),
        CMD_TIMEOUT,
    ) {
        if output.status.success() {
            let product_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !product_name.is_empty() {
                content = replace_boot_param(&content, "androidboot.hwname", &product_name);
                content = replace_boot_param(&content, "androidboot.product.hardware.sku", &product_name);
            }
        }
    }

    match client.set_cmdline(&content) {
        Ok(()) => info!("BRENE: {source} spoofed"),
        Err(e) => warn!("BRENE: {source} spoof failed: {e}"),
    }
}

fn replace_boot_param(content: &str, key: &str, new_value: &str) -> String {
    let prefix = format!("{key}=");
    if let Some(start) = content.find(&prefix) {
        let value_start = start + prefix.len();
        // cmdline uses spaces, bootconfig uses newlines
        let value_end = content[value_start..]
            .find(|c: char| c == ' ' || c == '\n' || c == '\r')
            .map(|i| value_start + i)
            .unwrap_or(content.len());
        let mut result = String::with_capacity(content.len());
        result.push_str(&content[..value_start]);
        result.push_str(new_value);
        result.push_str(&content[value_end..]);
        result
    } else {
        content.to_string()
    }
}

fn hide_ksu_loop_devices(client: &SusfsClient) -> u32 {
    let jbd2_dir = Path::new("/proc/fs/jbd2");
    if !jbd2_dir.is_dir() {
        return 0;
    }

    let entries = match fs::read_dir(jbd2_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut count = 0u32;

    for entry in entries.filter_map(|e| e.ok()) {
        let name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Match loop*-8 pattern (KSU loop devices use partition 8)
        if !name.starts_with("loop") || !name.ends_with("-8") {
            continue;
        }

        let device = &name[..name.len() - 2]; // strip "-8"

        let jbd2_path = format!("/proc/fs/jbd2/{name}");
        match client.add_sus_path(&jbd2_path) {
            Ok(()) => count += 1,
            Err(e) => debug!("hide loop jbd2 failed for {jbd2_path}: {e}"),
        }

        let ext4_path = format!("/proc/fs/ext4/{device}");
        match client.add_sus_path(&ext4_path) {
            Ok(()) => count += 1,
            Err(e) => debug!("hide loop ext4 failed for {ext4_path}: {e}"),
        }
    }

    count
}

fn truncate_uname(s: &str) -> String {
    if s.len() > 64 {
        s[..64].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::susfs::{SusfsClient, SusfsFeatures};

    #[test]
    fn brene_result_default_is_zeroed() {
        let r = BreneResult::default();
        assert_eq!(r.paths_hidden, 0);
        assert_eq!(r.maps_hidden, 0);
        assert!(r.font_modules.is_empty());
        assert!(!r.uname_spoofed);
        assert!(!r.avc_spoofed);
        assert!(!r.log_enabled);
    }

    #[test]
    fn brene_skips_when_susfs_unavailable() {
        let client = SusfsClient::new_for_test(false, SusfsFeatures::default());
        let config = ZeroMountConfig::default();
        let result = apply_brene(&client, &config, false, SusfsMode::Enhanced).expect("should not error");
        assert_eq!(result.paths_hidden, 0);
        assert_eq!(result.maps_hidden, 0);
        assert!(!result.uname_spoofed);
    }

    #[test]
    fn build_dynamic_uname_strips_markers() {
        let (release, version) = build_dynamic_uname().expect("should work on any host");
        assert!(!release.is_empty());
        assert!(!version.contains("-ksu"));
        assert!(!version.contains("-susfs"));
        assert!(!version.contains("-dirty"));
    }

    #[test]
    fn truncate_uname_respects_limit() {
        let long = "a".repeat(100);
        let truncated = truncate_uname(&long);
        assert_eq!(truncated.len(), 64);

        let short = "5.10.0";
        assert_eq!(truncate_uname(short), "5.10.0");
    }

    #[test]
    fn rooted_paths_includes_known_directories() {
        assert!(ROOTED_FOLDER_PATHS.contains(&"/data/adb"));
        assert!(ROOTED_FOLDER_PATHS.contains(&"/data/adb/modules"));
        assert!(ROOTED_FOLDER_PATHS.contains(&"/data/adb/ksu"));
        assert!(ROOTED_FOLDER_PATHS.contains(&"/data/adb/ap"));
    }

    #[test]
    fn zygisk_patterns_include_common_paths() {
        assert!(ZYGISK_MAP_PATTERNS.iter().any(|p| p.contains("zygisk")));
        assert!(ZYGISK_MAP_PATTERNS.iter().any(|p| p.contains("shamiko")));
    }

    #[test]
    fn hide_apk_returns_zero_when_no_data_app() {
        // /data/app doesn't exist on dev machines
        let client = SusfsClient::new_for_test(true, SusfsFeatures {
            path: true,
            ..SusfsFeatures::default()
        });
        let count = hide_apk_paths(&client);
        assert_eq!(count, 0);
    }

    #[test]
    fn process_font_modules_returns_empty_when_no_modules_dir() {
        let client = SusfsClient::new_for_test(true, SusfsFeatures {
            open_redirect: true,
            kstat: true,
            path: true,
            ..SusfsFeatures::default()
        });
        let results = process_font_modules(&client, "auto");
        assert!(results.is_empty());
    }

    #[test]
    fn direct_mount_hiding_never_invoked() {
        // S05: individual mount hiding causes LSPosed failures.
        // Global toggle via config sync is allowed.
        let src = include_str!("brene.rs");
        let banned_call = ["add_sus", "_mount"].concat();
        let msg = ["S05: banned call found in brene.rs: ", &banned_call].concat();
        assert!(!src.contains(&banned_call), "{msg}");
    }
}
