use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Result};
use tracing::{debug, error, info, warn};

use super::brene::FontModuleInfo;
use super::kstat;
use super::SusfsClient;
use crate::utils::command::run_command_with_timeout;

const EMOJI_STAGING_DIR: &str = "/data/adb/zeromount/emoji";
const EMOJI_FONT_NAME: &str = "NotoColorEmoji.ttf";
const SYSTEM_FONTS_DIR: &str = "/system/fonts";
const FONTS_XML_PATH: &str = "/system/etc/fonts.xml";

const EMOJI_VARIANTS: &[&str] = &[
    "NotoColorEmoji.ttf",
    "NotoColorEmojiFlags.ttf",
    "NotoColorEmojiLegacy.ttf",
    "SamsungColorEmoji.ttf",
    "AndroidEmoji-htc.ttf",
    "ColorUniEmoji.ttf",
    "DcmColorEmoji.ttf",
    "CombinedColorEmoji.ttf",
    "HTC_ColorEmoji.ttf",
    "LGNotoColorEmoji.ttf",
];

const FB_PACKAGES: &[(&str, &str)] = &[
    ("com.facebook.orca", "Messenger"),
    ("com.facebook.katana", "Facebook"),
];

const GMS_FONT_PROVIDER: &str = "com.google.android.gms/com.google.android.gms.fonts.provider.FontsProvider";
const GMS_UPDATE_SERVICE: &str = "com.google.android.gms/com.google.android.gms.fonts.update.UpdateSchedulerService";
const GBOARD_PACKAGE: &str = "com.google.android.inputmethod.latin";
const GMS_FONTS_DIR: &str = "/data/data/com.google.android.gms/files/fonts/opentype";

#[derive(Debug, Default)]
pub struct EmojiResult {
    pub mounts: u32,
    pub redirects: u32,
    pub vfs_rules: u32,
    pub skipped: u32,
    pub strategy: String,
}

#[derive(Debug, Default)]
pub struct EmojiAppResult {
    pub fb_succeeded: u32,
    pub fb_total: u32,
    pub gboard_ok: bool,
    pub gms_ok: bool,
}

pub fn check_emoji_font_conflict(font_modules: &[FontModuleInfo]) -> Option<String> {
    if font_modules.is_empty() {
        debug!("emoji: no font module conflicts");
        return None;
    }
    let first = &font_modules[0].id;
    info!("emoji: conflict check — {} font module(s) detected, first: {}", font_modules.len(), first);
    Some(first.clone())
}

fn discover_emoji_targets() -> Vec<String> {
    let mut targets: Vec<String> = EMOJI_VARIANTS.iter().map(|s| s.to_string()).collect();

    if let Ok(content) = fs::read_to_string(FONTS_XML_PATH) {
        let mut in_emoji_family = false;
        let mut xml_count = 0u32;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("lang=\"und-Zsye\"") {
                in_emoji_family = true;
            }
            if in_emoji_family {
                if let Some(start) = trimmed.find('>') {
                    if let Some(end) = trimmed[start + 1..].find('<') {
                        let font_name = trimmed[start + 1..start + 1 + end].trim();
                        if font_name.ends_with(".ttf") || font_name.ends_with(".ttc") || font_name.ends_with(".otf") {
                            if !targets.iter().any(|t| t == font_name) {
                                targets.push(font_name.to_string());
                                xml_count += 1;
                            }
                        }
                    }
                }
                if trimmed.contains("</family>") {
                    in_emoji_family = false;
                }
            }
        }
        debug!("emoji: discovered {} additional targets from fonts.xml", xml_count);
    }

    debug!("emoji: {} total unique targets", targets.len());
    targets
}

pub fn apply_emoji_fonts(
    client: &SusfsClient,
    _overlay_source: &str,
    _fonts_overlay_mounted: bool,
) -> Result<EmojiResult> {
    let mut result = EmojiResult::default();

    let source = Path::new(EMOJI_STAGING_DIR).join(EMOJI_FONT_NAME);
    if !source.exists() {
        error!("emoji: source font missing at {}", source.display());
        bail!("emoji font not found at {}", source.display());
    }

    let size = fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
    info!("emoji: source font at {} ({} bytes)", source.display(), size);

    // Bind mount exposes source permissions — must be world-readable for system_server/zygote
    let _ = fs::set_permissions(&source, fs::Permissions::from_mode(0o644));
    crate::utils::selinux::set_selinux_context(&source, "u:object_r:system_file:s0");
    debug!("emoji: SELinux context set to system_file, perms=0644 on source");

    let targets = discover_emoji_targets();
    result.strategy = "bind".to_string();
    info!("emoji: mounting strategy=bind, targets={}", targets.len());

    let stock_dev = fs::metadata(SYSTEM_FONTS_DIR).ok().map(|m| m.dev());
    let vfs_driver = crate::vfs::VfsDriver::open().ok();

    for target_name in &targets {
        let target_path = format!("{}/{}", SYSTEM_FONTS_DIR, target_name);

        if !Path::new(&target_path).exists() {
            if let Some(ref driver) = vfs_driver {
                match driver.add_rule(&source, Path::new(&target_path), false) {
                    Ok(()) => {
                        debug!("emoji: VFS rule for {} (target doesn't exist, staged)", target_name);
                        result.vfs_rules += 1;
                    }
                    Err(e) => {
                        debug!("emoji: skipped {} (no system target, VFS failed: {})", target_name, e);
                        result.skipped += 1;
                    }
                }
            } else {
                debug!("emoji: skipped {} (no system target, no VFS driver)", target_name);
                result.skipped += 1;
            }
            continue;
        }

        // Always bind-mount: emoji comes from our staging dir, not a KSU module
        let c_src = match std::ffi::CString::new(source.as_os_str().as_encoded_bytes()) {
            Ok(s) => s,
            Err(_) => { result.skipped += 1; continue; }
        };
        let c_tgt = match std::ffi::CString::new(target_path.as_bytes()) {
            Ok(s) => s,
            Err(_) => { result.skipped += 1; continue; }
        };

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
            result.mounts += 1;
            debug!("emoji: bind {} → ok", target_name);
        } else {
            let err = std::io::Error::last_os_error();
            debug!("emoji: bind {} → fail ({})", target_name, err);
            result.skipped += 1;
            continue;
        }

        // Stealth: kstat redirect + path hide on top of bind mount
        let replacement = source.to_string_lossy().to_string();
        if client.features().kstat {
            match kstat::build_kstat_values_from_paths(&target_path, &replacement) {
                Ok(mut spoof) => {
                    if let Some(dev) = stock_dev {
                        spoof.dev = Some(dev);
                        let hash: u64 = target_path.bytes().fold(0xcbf29ce484222325u64, |h, b| {
                            (h ^ b as u64).wrapping_mul(0x100000001b3)
                        });
                        spoof.ino = Some(hash % 2_147_483_647);
                    }
                    if let Err(e) = client.add_sus_kstat_redirect(&target_path, &replacement, &spoof) {
                        debug!("emoji: kstat redirect failed for {}: {}", target_name, e);
                    } else {
                        result.redirects += 1;
                    }
                }
                Err(e) => debug!("emoji: kstat build failed for {}: {}", target_name, e),
            }
        }

        if client.features().path {
            if let Err(e) = client.add_sus_path(&replacement) {
                debug!("emoji: path hide failed for {}: {}", target_name, e);
            }
        }
    }

    info!("emoji: complete — {} mounts, {} redirects, {} VFS rules, {} skipped",
          result.mounts, result.redirects, result.vfs_rules, result.skipped);
    Ok(result)
}

pub fn apply_emoji_app_overrides() -> EmojiAppResult {
    let mut result = EmojiAppResult::default();
    let source = Path::new(EMOJI_STAGING_DIR).join(EMOJI_FONT_NAME);

    if !source.exists() {
        warn!("emoji: source font missing for app overrides, skipping");
        return result;
    }

    info!("emoji: applying app-level overrides (FB + GBoard + GMS)");

    // Layer 2: Facebook app_ras_blobs injection
    for (pkg, name) in FB_PACKAGES {
        result.fb_total += 1;
        if !package_installed(pkg) {
            info!("emoji: {} ({}) — not installed", name, pkg);
            continue;
        }
        info!("emoji: {} ({}) — installed", name, pkg);

        let ras_dir = format!("/data/data/{}/app_ras_blobs", pkg);
        let font_dest = format!("{}/FacebookEmoji.ttf", ras_dir);

        // Nuke and recreate
        let _ = fs::remove_dir_all(&ras_dir);
        if let Err(e) = fs::create_dir_all(&ras_dir) {
            warn!("emoji: {} app_ras_blobs mkdir failed: {}", name, e);
            continue;
        }

        if let Err(e) = fs::copy(&source, &font_dest) {
            warn!("emoji: {} app_ras_blobs copy failed: {}", name, e);
            continue;
        }

        // Match ownership to the app's UID
        let app_uid = get_app_uid(pkg);
        if let Some(uid) = app_uid {
            let _ = set_ownership(&ras_dir, uid, uid);
            let _ = set_ownership(&font_dest, uid, uid);
            let _ = fs::set_permissions(&ras_dir, fs::Permissions::from_mode(0o755));
            let _ = fs::set_permissions(&font_dest, fs::Permissions::from_mode(0o644));
            info!("emoji: {} app_ras_blobs replaced (uid={}, perms=0644)", name, uid);
            result.fb_succeeded += 1;
        } else {
            // Fallback: world-readable
            let _ = fs::set_permissions(&ras_dir, fs::Permissions::from_mode(0o755));
            let _ = fs::set_permissions(&font_dest, fs::Permissions::from_mode(0o644));
            warn!("emoji: {} app_ras_blobs replaced (uid unknown, perms=0644)", name);
            result.fb_succeeded += 1;
        }
    }

    // Layer 3a: GBoard emoji font overwrite
    let gboard_dir = Path::new(GMS_FONTS_DIR);
    if gboard_dir.is_dir() {
        let mut overwritten = 0u32;
        if let Ok(entries) = fs::read_dir(gboard_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let fname = entry.file_name();
                let name = fname.to_string_lossy();
                if name.starts_with("Noto_Color_Emoji_Compat") && name.ends_with(".ttf") {
                    let dest = entry.path();
                    if fs::copy(&source, &dest).is_ok() {
                        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o644));
                        overwritten += 1;
                    }
                }
            }
        }
        if overwritten > 0 {
            info!("emoji: GBoard — overwrote {} emoji compat files", overwritten);
            result.gboard_ok = true;
        } else {
            debug!("emoji: GBoard — no emoji compat files found to overwrite");
        }
    } else {
        debug!("emoji: GBoard font dir not found, skipping");
    }

    // Clear GBoard caches and force-stop
    clear_gboard_caches();

    // Layer 3b: GMS font killer (one-shot)
    result.gms_ok = disable_gms_font_provider();

    info!("emoji: app overrides complete — fb={}/{}, gboard={}, gms={}",
          result.fb_succeeded, result.fb_total, result.gboard_ok, result.gms_ok);
    result
}

fn package_installed(pkg: &str) -> bool {
    let mut cmd = Command::new("pm");
    cmd.args(["path", pkg]);
    run_command_with_timeout(&mut cmd, Duration::from_secs(10))
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn get_app_uid(pkg: &str) -> Option<u32> {
    let data_dir = format!("/data/data/{}", pkg);
    fs::metadata(&data_dir).ok().map(|m| m.uid())
}

fn set_ownership(path: &str, uid: u32, gid: u32) -> Result<()> {
    use std::ffi::CString;
    let c_path = CString::new(path)?;
    let ret = unsafe { libc::chown(c_path.as_ptr(), uid, gid) };
    if ret != 0 {
        bail!("chown failed for {}: {}", path, std::io::Error::last_os_error());
    }
    Ok(())
}

fn clear_gboard_caches() {
    let mut cleared = 0u32;
    let data_dir = Path::new("/data/data").join(GBOARD_PACKAGE);
    if data_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&data_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.contains("cache") {
                    let _ = fs::remove_dir_all(entry.path());
                    cleared += 1;
                }
            }
        }
    }

    // Force-stop GBoard
    let mut cmd = Command::new("am");
    cmd.args(["force-stop", GBOARD_PACKAGE]);
    let _ = run_command_with_timeout(&mut cmd, Duration::from_secs(5));

    if cleared > 0 {
        info!("emoji: GBoard — cleared {} cache dirs, force-stopped", cleared);
    }
}

fn disable_gms_font_provider() -> bool {
    let user_dirs = match fs::read_dir("/data/user") {
        Ok(e) => e,
        Err(_) => {
            debug!("emoji: /data/user not readable, trying user 0 only");
            return disable_gms_for_user("0");
        }
    };

    let mut any_ok = false;
    for entry in user_dirs.filter_map(|e| e.ok()) {
        let user_id = entry.file_name().to_string_lossy().to_string();
        if user_id.parse::<u32>().is_err() {
            continue;
        }
        if disable_gms_for_user(&user_id) {
            any_ok = true;
        }
    }

    // Delete GMS downloaded fonts
    let _ = fs::remove_dir_all("/data/fonts");
    let gms_fonts = "/data/data/com.google.android.gms/files/fonts";
    if Path::new(gms_fonts).is_dir() {
        let _ = fs::remove_dir_all(gms_fonts);
        info!("emoji: GMS font dirs purged");
    } else {
        debug!("emoji: GMS font dir not found, skipping purge");
    }

    any_ok
}

fn disable_gms_for_user(user_id: &str) -> bool {
    let mut ok = true;

    for component in &[GMS_FONT_PROVIDER, GMS_UPDATE_SERVICE] {
        let mut cmd = Command::new("pm");
        cmd.args(["disable", "--user", user_id, component]);
        match run_command_with_timeout(&mut cmd, Duration::from_secs(10)) {
            Ok(output) if output.status.success() => {
                debug!("emoji: GMS {} disabled for user {}", component.rsplit('/').next().unwrap_or(component), user_id);
            }
            _ => {
                debug!("emoji: GMS {} disable failed for user {}", component.rsplit('/').next().unwrap_or(component), user_id);
                ok = false;
            }
        }
    }

    ok
}

use std::os::unix::fs::PermissionsExt;
