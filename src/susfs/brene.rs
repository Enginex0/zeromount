use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use crate::core::config::{UnameConfig, UnameMode, ZeroMountConfig};
use super::SusfsClient;
use super::fonts;
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

// /sdcard/Android/data patterns for loop hiding (re-flagged per zygote spawn)
const SDCARD_DATA_PATTERNS: &[&str] = &[
    "/sdcard/Android/data",
    "/storage/emulated/0/Android/data",
];

/// Summary of all BRENE operations applied during a boot cycle.
#[derive(Debug, Default)]
pub struct BreneResult {
    pub paths_hidden: u32,
    pub maps_hidden: u32,
    pub font_modules_processed: u32,
    pub uname_spoofed: bool,
    pub avc_spoofed: bool,
    pub log_enabled: bool,
}

/// Apply BRENE (Be Root, ENjoy Everything) toggles via SUSFS.
/// Mount hiding is intentionally excluded (S05).
/// Property spoofing uses resetprop (not SUSFS) and is handled separately.
pub fn apply_brene(client: &SusfsClient, config: &ZeroMountConfig) -> Result<BreneResult> {
    let mut result = BreneResult::default();

    if !client.is_available() {
        debug!("SUSFS unavailable, skipping BRENE application");
        return Ok(result);
    }

    let brene = &config.brene;

    // -- Auto-hide toggles (path-based) --

    if brene.auto_hide_rooted_folders && client.features().path {
        let count = paths::hide_paths(client, ROOTED_FOLDER_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: rooted folders hidden ({count})");
    }

    if brene.auto_hide_recovery && client.features().path {
        let count = paths::hide_paths(client, RECOVERY_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: recovery paths hidden ({count})");
    }

    if brene.auto_hide_tmp && client.features().path {
        let count = paths::hide_paths(client, TMP_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: tmp paths hidden ({count})");
    }

    if brene.auto_hide_apk && client.features().path {
        let count = hide_apk_paths(client);
        result.paths_hidden += count;
        info!("BRENE: APK paths hidden ({count})");
    }

    if brene.auto_hide_sdcard_data && client.features().path {
        let count = paths::hide_paths_loop(client, SDCARD_DATA_PATTERNS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: sdcard data roots hidden ({count})");
    }

    // -- Maps hiding --

    if brene.auto_hide_zygisk && client.features().maps {
        let count = paths::hide_maps(client, ZYGISK_MAP_PATTERNS).unwrap_or(0);
        result.maps_hidden += count;
        info!("BRENE: zygisk maps hidden ({count})");
    }

    // -- Font redirect (delegates to F15) --

    if brene.auto_hide_fonts {
        let count = process_font_modules(client, &config.mount.overlay_source);
        result.font_modules_processed = count;
        info!("BRENE: processed {count} font modules");
    }

    // -- Custom user-defined lists --

    if !brene.custom_sus_paths.is_empty() && client.features().path {
        let path_refs: Vec<&str> = brene.custom_sus_paths.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: custom sus_paths hidden ({count}/{})", path_refs.len());
    }

    if !brene.custom_sus_path_loops.is_empty() && client.features().path {
        let path_refs: Vec<&str> = brene.custom_sus_path_loops.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths_loop(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE: custom sus_path_loops hidden ({count}/{})", path_refs.len());
    }

    if !brene.custom_sus_maps.is_empty() && client.features().maps {
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

    // -- Uname spoofing --

    apply_uname(client, &config.uname, &mut result)?;

    // Sync all 5 controlled settings to SUSFS config.sh
    if let Err(e) = sync_susfs_config(config) {
        warn!("BRENE: SUSFS config sync failed: {e}");
    }

    info!(
        "BRENE complete: {} paths, {} maps, {} font modules, uname={}, avc={}",
        result.paths_hidden,
        result.maps_hidden,
        result.font_modules_processed,
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

/// Scan /data/adb/modules/ for font modules and apply redirect via F15.
fn process_font_modules(client: &SusfsClient, overlay_source: &str) -> u32 {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.is_dir() {
        return 0;
    }

    let entries = match fs::read_dir(modules_dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };

    let mut count = 0u32;

    for entry in entries.filter_map(|e| e.ok()) {
        let module_path = entry.path();
        if !module_path.is_dir() {
            continue;
        }

        // Skip disabled/removing modules
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

        match fonts::redirect_font_module(client, &module_id, &font_dir, SYSTEM_FONTS_DIR, overlay_source) {
            Ok(result) => {
                debug!(
                    "font module '{}': strategy={:?}, redirected={}",
                    module_id, result.strategy, result.redirect_count
                );
                count += 1;
            }
            Err(e) => warn!("font module '{module_id}' failed: {e}"),
        }
    }

    count
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

    // Strip known build markers from version string
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

const SUSFS_PERSISTENT_CONFIG: &str = "/data/adb/susfs4ksu/config.sh";
const SUSFS_CONFIG_DIR: &str = "/data/adb/susfs4ksu";

const SUSFS_SHARED_KEYS: [(&str, fn(&crate::core::config::BreneConfig) -> bool); 5] = [
    ("susfs_log", |b| b.susfs_log),
    ("avc_log_spoofing", |b| b.avc_log_spoofing),
    ("hide_sus_mnts_for_all_or_non_su_procs", |b| b.hide_sus_mounts),
    ("emulate_vold_app_data", |b| b.emulate_vold_app_data),
    ("force_hide_lsposed", |b| b.force_hide_lsposed),
];

fn parse_shell_bool(content: &str, key: &str) -> Option<bool> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(val) = trimmed.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')) {
            // Strip inline comments and quotes: `1 # comment` -> `1`, `"1"` -> `1`
            let clean = val.split('#').next().unwrap_or(val).trim().trim_matches('"');
            return Some(clean == "1");
        }
    }
    None
}

pub fn import_susfs_config(config: &mut ZeroMountConfig) -> Result<bool> {
    let config_path = Path::new(SUSFS_PERSISTENT_CONFIG);
    if !config_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(config_path)
        .context("reading SUSFS config.sh for import")?;

    let mut changed = false;
    let brene = &mut config.brene;

    for &(shell_key, getter) in &SUSFS_SHARED_KEYS {
        if let Some(file_val) = parse_shell_bool(&content, shell_key) {
            let current = getter(brene);
            if file_val != current {
                match shell_key {
                    "susfs_log" => brene.susfs_log = file_val,
                    "avc_log_spoofing" => brene.avc_log_spoofing = file_val,
                    "hide_sus_mnts_for_all_or_non_su_procs" => brene.hide_sus_mounts = file_val,
                    "emulate_vold_app_data" => brene.emulate_vold_app_data = file_val,
                    "force_hide_lsposed" => brene.force_hide_lsposed = file_val,
                    _ => {}
                }
                info!("imported from SUSFS: {shell_key} = {file_val}");
                changed = true;
            }
        }
    }

    if changed {
        config.save(None)?;
    }

    Ok(changed)
}

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
            info!("BRENE: created SUSFS config.sh with 5 settings");
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
    info!("BRENE: synced 5 settings to SUSFS config.sh");
    Ok(())
}

/// Deferred BRENE: only re-run path-hiding operations that require
/// android_data_root_path (which isn't available at boot time).
/// Maps, mounts, AVC, log, uname, fonts all succeed at boot — skip them.
pub fn apply_brene_deferred(client: &SusfsClient, config: &ZeroMountConfig) -> Result<BreneResult> {
    let mut result = BreneResult::default();

    if !client.is_available() {
        return Ok(result);
    }

    let brene = &config.brene;
    let has_path = client.features().path;

    if brene.auto_hide_rooted_folders && has_path {
        let count = paths::hide_paths(client, ROOTED_FOLDER_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: rooted folders hidden ({count})");
    }

    if brene.auto_hide_recovery && has_path {
        let count = paths::hide_paths(client, RECOVERY_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: recovery paths hidden ({count})");
    }

    if brene.auto_hide_tmp && has_path {
        let count = paths::hide_paths(client, TMP_PATHS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: tmp paths hidden ({count})");
    }

    if brene.auto_hide_apk && has_path {
        let count = hide_apk_paths(client);
        result.paths_hidden += count;
        info!("BRENE deferred: APK paths hidden ({count})");
    }

    if brene.auto_hide_sdcard_data && has_path {
        let count = paths::hide_paths_loop(client, SDCARD_DATA_PATTERNS).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: sdcard data roots hidden ({count})");
    }

    if !brene.custom_sus_paths.is_empty() && has_path {
        let path_refs: Vec<&str> = brene.custom_sus_paths.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: custom sus_paths hidden ({count})");
    }

    if !brene.custom_sus_path_loops.is_empty() && has_path {
        let path_refs: Vec<&str> = brene.custom_sus_path_loops.iter().map(|s| s.as_str()).collect();
        let count = paths::hide_paths_loop(client, &path_refs).unwrap_or(0);
        result.paths_hidden += count;
        info!("BRENE deferred: custom sus_path_loops hidden ({count})");
    }

    info!("BRENE deferred complete: {} paths hidden", result.paths_hidden);
    Ok(result)
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
        assert_eq!(r.font_modules_processed, 0);
        assert!(!r.uname_spoofed);
        assert!(!r.avc_spoofed);
        assert!(!r.log_enabled);
    }

    #[test]
    fn brene_skips_when_susfs_unavailable() {
        let client = SusfsClient::new_for_test(false, SusfsFeatures::default());
        let config = ZeroMountConfig::default();
        let result = apply_brene(&client, &config).expect("should not error");
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
    fn sdcard_patterns_cover_both_paths() {
        assert!(SDCARD_DATA_PATTERNS.contains(&"/sdcard/Android/data"));
        assert!(SDCARD_DATA_PATTERNS.contains(&"/storage/emulated/0/Android/data"));
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
    fn process_font_modules_returns_zero_when_no_modules_dir() {
        let client = SusfsClient::new_for_test(true, SusfsFeatures {
            open_redirect: true,
            kstat: true,
            path: true,
            ..SusfsFeatures::default()
        });
        let count = process_font_modules(&client, "auto");
        assert_eq!(count, 0);
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
