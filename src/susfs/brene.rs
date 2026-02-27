use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tracing::{debug, error, info, warn};

use crate::core::config::{UnameConfig, UnameMode, ZeroMountConfig};
use crate::core::types::{ExternalSusfsModule, SusfsMode};
use crate::utils::command::{run_command_with_timeout, CMD_TIMEOUT};
use crate::utils::hash::fnv1a_ino;
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

pub fn apply_brene(
    client: &SusfsClient,
    config: &ZeroMountConfig,
    fonts_overlay_mounted: bool,
    _susfs_mode: SusfsMode,
    external_module: ExternalSusfsModule,
) -> Result<BreneResult> {
    let mut result = BreneResult::default();
    let brene = &config.brene;

    // Fonts and emoji use bind mounts / overlayfs — standard Linux ops that
    // work on any kernel. SUSFS kstat/path is layered on top when available.
    if brene.auto_hide_fonts {
        let fonts = if fonts_overlay_mounted {
            hide_font_modules_overlay(client)
        } else {
            process_font_modules(client, &config.mount.overlay_source)
        };
        info!("BRENE: processed {} font modules (overlay={}): {:?}", fonts.len(), fonts_overlay_mounted, fonts);
        result.font_modules = fonts;
    }

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

    apply_hide_usb_debugging(config.adb.hide_usb_debugging);

    if !client.is_available() {
        debug!("SUSFS unavailable, skipping remaining BRENE protections");
        return Ok(result);
    }

    let defer_supercalls = external_module != ExternalSusfsModule::None;
    let run_complement = external_module != ExternalSusfsModule::Susfs4ksu;

    if defer_supercalls {
        info!("BRENE: deferring supercall ops to external module ({:?})", external_module);
    }
    if !run_complement {
        info!("BRENE: skipping complement ops (susfs4ksu handles cmdline/APK/loops/LSPosed)");
    }

    let susfs_cfg = &config.susfs;

    let has_path = client.features().path && susfs_cfg.path_hide;
    let has_maps = client.features().maps && susfs_cfg.maps_hide;
    let _has_kstat = client.features().kstat && susfs_cfg.kstat;

    if brene.auto_hide_rooted_folders && has_path {
        let count = paths::hide_paths(client, ROOTED_FOLDER_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: rooted folders hidden ({count})");
    }

    if brene.auto_hide_recovery && has_path {
        let count = paths::hide_paths(client, RECOVERY_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: recovery paths hidden ({count})");
    }

    if brene.auto_hide_tmp && has_path {
        let count = paths::hide_dir_children_loop(client, TMP_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: tmp children hidden via loop ({count})");
    }

    if run_complement {
        if brene.auto_hide_apk && has_path {
            let count = hide_apk_paths(client);
            result.paths_hidden += count;
            info!("BRENE: APK paths hidden ({count})");
        }
    }

    if brene.auto_hide_zygisk && has_maps {
        let count = paths::hide_maps(client, ZYGISK_MAP_PATTERNS).unwrap_or(0);
        result.maps_hidden += count;
        info!("BRENE: zygisk maps hidden ({count})");
    }

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

    if !defer_supercalls {
        match client.hide_sus_mounts(brene.hide_sus_mounts) {
            Ok(()) => info!("BRENE: hide_sus_mounts set to {}", brene.hide_sus_mounts),
            Err(e) => warn!("BRENE: hide_sus_mounts failed: {e}"),
        }
    }

    if !defer_supercalls {
        if brene.avc_log_spoofing {
            match client.enable_avc_log_spoofing(true) {
                Ok(()) => {
                    result.avc_spoofed = true;
                    info!("BRENE: AVC log spoofing enabled");
                }
                Err(e) => warn!("BRENE: AVC log spoofing failed: {e}"),
            }
        }
    }

    if !defer_supercalls {
        if brene.susfs_log {
            match client.enable_log(true) {
                Ok(()) => {
                    result.log_enabled = true;
                    info!("BRENE: SUSFS logging enabled");
                }
                Err(e) => warn!("BRENE: SUSFS log enable failed: {e}"),
            }
        }
    }

    if run_complement {
        if brene.spoof_cmdline {
            apply_spoof_cmdline(client);
        }
    }

    if run_complement {
        if brene.hide_ksu_loops && client.features().path {
            let count = hide_ksu_loop_devices(client);
            result.paths_hidden += count;
            if count > 0 {
                info!("BRENE: KSU loop devices hidden ({count})");
            }
        }
    }

    if run_complement {
        if brene.force_hide_lsposed {
            apply_force_hide_lsposed();
        }
    }

    if !defer_supercalls {
        apply_uname(client, &config.uname, &mut result)?;
    }

    if let Err(e) = sync_susfs_config(config) {
        warn!("BRENE: SUSFS config sync failed: {e}");
    }

    info!(
        "BRENE complete: {} paths, {} maps, {} font modules, uname={}, avc={}, external={:?}",
        result.paths_hidden,
        result.maps_hidden,
        result.font_modules.len(),
        result.uname_spoofed,
        result.avc_spoofed,
        external_module,
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
                            spoof.ino = Some(fnv1a_ino(&target));
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

/// Build sanitized uname values by stripping kernel build markers.
/// Reads /proc/version and removes KernelSU/SUSFS/custom build indicators.
fn build_dynamic_uname() -> Result<(String, String)> {
    let raw = fs::read_to_string("/proc/version")
        .unwrap_or_else(|_| "Linux version 5.10.0".to_string());

    let parts: Vec<&str> = raw.splitn(4, ' ').collect();
    let release = parts.get(2).unwrap_or(&"5.10.0").to_string();

    let version = raw
        .replace("-ksu", "")
        .replace("-susfs", "")
        .replace("-dirty", "")
        .replace("-custom", "")
        .replace("-gki", "-android13");

    // Truncate to kernel's NEW_UTS_LEN (64 chars)
    let release = truncate_uname(&release);
    let version = truncate_uname(&version);

    Ok((release, version))
}

const HIDE_USB_DEBUGGING_SENTINEL: &str = "/data/adb/zeromount/flags/hide_usb_debugging";
const HIDE_ADB_SYSFS_KNOB: &str = "/sys/kernel/zeromount/hide_adb";

pub fn apply_hide_usb_debugging(enabled: bool) {
    let sentinel = Path::new(HIDE_USB_DEBUGGING_SENTINEL);

    if enabled {
        if let Some(parent) = sentinel.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(e) = fs::write(sentinel, "1") {
            warn!("hide_usb_debugging: sentinel write failed: {e}");
        }
        if let Err(e) = fs::write(HIDE_ADB_SYSFS_KNOB, "1") {
            debug!("hide_usb_debugging: sysfs knob unavailable: {e}");
        }
        info!("hide_usb_debugging: enabled");
    } else {
        if sentinel.exists() {
            if let Err(e) = fs::remove_file(sentinel) {
                warn!("hide_usb_debugging: sentinel removal failed: {e}");
            }
        }
        if let Err(e) = fs::write(HIDE_ADB_SYSFS_KNOB, "0") {
            debug!("hide_usb_debugging: sysfs knob unavailable: {e}");
        }
        info!("hide_usb_debugging: disabled");
    }
}

const SUSFS_PERSISTENT_CONFIG: &str = "/data/adb/susfs4ksu/config.sh";
const SUSFS_CONFIG_DIR: &str = "/data/adb/susfs4ksu";

const SUSFS_SHARED_KEYS: [(&str, fn(&crate::core::config::BreneConfig) -> bool); 8] = [
    ("susfs_log", |b| b.susfs_log),
    ("avc_log_spoofing", |b| b.avc_log_spoofing),
    ("hide_sus_mnts_for_all_or_non_su_procs", |b| b.hide_sus_mounts),
    ("emulate_vold_app_data", |b| b.emulate_vold_app_data),
    ("force_hide_lsposed", |b| b.force_hide_lsposed),
    ("spoof_cmdline", |b| b.spoof_cmdline),
    ("hide_loops", |b| b.hide_ksu_loops),
    ("auto_try_umount", |b| b.try_umount),
];

// Sync our BRENE toggles to SUSFS config.sh so SUSFS boot scripts stay in sync
pub fn sync_susfs_config(config: &ZeroMountConfig) -> Result<()> {
    let config_path = Path::new(SUSFS_PERSISTENT_CONFIG);
    let brene = &config.brene;

    if !config_path.exists() {
        // SUSFS installed but config.sh missing — create it
        if Path::new(SUSFS_CONFIG_DIR).is_dir() {
            let mut content = String::new();
            for &(key, getter) in &SUSFS_SHARED_KEYS {
                let val = if getter(brene) { "1" } else { "0" };
                content.push_str(&format!("{key}={val}\n"));
            }
            fs::write(config_path, &content).context("creating SUSFS config.sh")?;
            info!("BRENE: created SUSFS config.sh with 8 settings");
        } else {
            debug!("SUSFS not installed, skipping config sync");
        }
        return Ok(());
    }

    let pairs: Vec<(&str, bool)> = SUSFS_SHARED_KEYS
        .iter()
        .map(|&(key, getter)| (key, getter(brene)))
        .collect();

    let mut content = fs::read_to_string(config_path)
        .context("reading SUSFS config.sh")?;

    for (key, value) in &pairs {
        let val_str = if *value { "1" } else { "0" };
        let pattern = format!("{key}=");
        if let Some(pos) = content.find(&pattern) {
            let line_end = content[pos..].find('\n').map(|i| pos + i).unwrap_or(content.len());
            content.replace_range(pos..line_end, &format!("{key}={val_str}"));
        }
    }

    fs::write(config_path, &content).context("writing SUSFS config.sh")?;
    info!("BRENE: synced 8 settings to SUSFS config.sh");
    Ok(())
}

pub fn emulate_vold_app_data(client: &SusfsClient) -> u32 {
    let output = match run_command_with_timeout(
        Command::new("pm").args(["list", "packages", "-3"]),
        CMD_TIMEOUT,
    ) {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            warn!("pm list packages -3 failed (exit {})", o.status.code().unwrap_or(-1));
            return 0;
        }
        Err(e) => {
            warn!("pm list packages -3 failed: {e}");
            return 0;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut count = 0u32;

    for line in stdout.lines() {
        let pkg = match line.strip_prefix("package:") {
            Some(p) => p.trim(),
            None => continue,
        };
        if pkg.is_empty() {
            continue;
        }

        let path = format!("/sdcard/Android/data/{pkg}");
        match client.add_sus_path(&path) {
            Ok(()) => count += 1,
            Err(e) => debug!("vold_app_data: hide failed for {pkg}: {e}"),
        }
    }

    count
}

const SUSFS4KSU_TRY_UMOUNT_TXT: &str = "/data/adb/susfs4ksu/try_umount.txt";

fn find_ksud() -> &'static str {
    if Path::new("/data/adb/ksu/bin/ksud").exists() {
        "/data/adb/ksu/bin/ksud"
    } else if Path::new("/data/adb/ap/bin/ksud").exists() {
        "/data/adb/ap/bin/ksud"
    } else {
        "ksud"
    }
}

pub fn try_umount_ksu_mounts(client: &SusfsClient, hide_sus_mounts_enabled: bool) -> u32 {
    let ksud = find_ksud();

    if hide_sus_mounts_enabled {
        let _ = client.hide_sus_mounts(false);
    }

    let mountinfo = match fs::read_to_string("/proc/1/mountinfo") {
        Ok(m) => m,
        Err(e) => {
            warn!("try_umount: cannot read /proc/1/mountinfo: {e}");
            if hide_sus_mounts_enabled {
                let _ = client.hide_sus_mounts(true);
            }
            return 0;
        }
    };

    let mut count = 0u32;
    for line in mountinfo.lines() {
        if !is_ksu_mount(line) {
            continue;
        }
        let mount_point = match extract_mount_point(line) {
            Some(p) => p,
            None => continue,
        };
        if crate::utils::signal::shutdown_requested() {
            break;
        }
        match run_command_with_timeout(
            Command::new(ksud).args(["kernel", "umount", "add", mount_point, "--flags", "2"]),
            CMD_TIMEOUT,
        ) {
            Ok(o) if o.status.success() => {
                count += 1;
                debug!("try_umount registered: {mount_point}");
            }
            Ok(o) => {
                debug!(
                    "try_umount failed for {mount_point} (exit {})",
                    o.status.code().unwrap_or(-1)
                );
            }
            Err(e) => {
                debug!("try_umount failed for {mount_point}: {e}");
            }
        }
    }

    let txt_count = process_try_umount_txt(ksud);
    count += txt_count;

    if hide_sus_mounts_enabled {
        let _ = client.hide_sus_mounts(true);
    }

    info!("try_umount: {count} paths registered ({txt_count} from txt)");
    count
}

fn is_ksu_mount(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let mount_id: u32 = match fields.next().and_then(|f| f.parse().ok()) {
        Some(id) => id,
        None => return false,
    };
    let in_range =
        (100_000..400_000).contains(&mount_id) || (500_000..600_000).contains(&mount_id);
    if !in_range {
        return false;
    }
    line.contains(" KSU") || line.contains(" shared")
}

fn extract_mount_point(line: &str) -> Option<&str> {
    line.split_whitespace().nth(4)
}

fn process_try_umount_txt(ksud: &str) -> u32 {
    let content = match fs::read_to_string(SUSFS4KSU_TRY_UMOUNT_TXT) {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut count = 0u32;
    for line in content.lines() {
        let path = line.trim();
        if path.is_empty() || path.starts_with('#') {
            continue;
        }
        if crate::utils::signal::shutdown_requested() {
            break;
        }
        match run_command_with_timeout(
            Command::new(ksud).args(["kernel", "umount", "add", path, "--flags", "2"]),
            CMD_TIMEOUT,
        ) {
            Ok(o) if o.status.success() => {
                count += 1;
                debug!("try_umount (txt) registered: {path}");
            }
            Ok(_) | Err(_) => {
                debug!("try_umount (txt) failed for {path}");
            }
        }
    }
    count
}

fn apply_force_hide_lsposed() {
    let ksud = find_ksud();

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
    if s.len() <= 64 {
        return s.to_string();
    }
    // Truncate at the last char boundary at or before byte 64 to avoid splitting
    // multi-byte sequences. In practice uname strings are ASCII, but be safe.
    let boundary = s.char_indices()
        .take_while(|(i, _)| *i < 64)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    s[..boundary].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::ExternalSusfsModule;
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
    fn brene_skips_susfs_ops_when_unavailable() {
        let client = SusfsClient::new_for_test(false, SusfsFeatures::default());
        let config = ZeroMountConfig::default();
        let result = apply_brene(&client, &config, false, SusfsMode::Enhanced, ExternalSusfsModule::None).expect("should not error");
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
