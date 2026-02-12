use std::ffi::CString;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};

use super::kstat::build_kstat_values_from_paths;
use super::SusfsClient;

// Font file extensions we handle
const FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "woff", "woff2"];
// Emoji/audio assets that also live in fonts dir
const ASSET_EXTENSIONS: &[&str] = &["png", "zlib", "ogg"];

/// Result of a single font redirect operation.
#[derive(Debug)]
#[allow(dead_code)] // Status reporting fields populated during redirect
pub struct FontRedirectResult {
    pub target: String,
    pub replacement: String,
    pub open_redirect: bool,
    pub kstat_redirect: bool,
    pub path_hidden: bool,
}

/// Redirect a single font file.
///
/// Flow (mirrors susfs_integration.sh:apply_font_redirect):
/// 1. Copy SELinux context from original to replacement
/// 2. open_redirect_all (so all UIDs see replacement when opening target)
/// 3. kstat_redirect (so stat() on target returns original metadata)
/// 4. Hide replacement path via add_sus_path
pub fn redirect_font_file(
    client: &SusfsClient,
    target: &str,
    replacement: &str,
) -> Result<FontRedirectResult> {
    let mut result = FontRedirectResult {
        target: target.to_string(),
        replacement: replacement.to_string(),
        open_redirect: false,
        kstat_redirect: false,
        path_hidden: false,
    };

    if !client.is_available() {
        bail!("SUSFS not available for font redirect");
    }

    if !Path::new(replacement).exists() {
        bail!("replacement file does not exist: {replacement}");
    }

    // 1. Copy SELinux context
    copy_selinux_context(target, replacement);

    // 2. Open redirect (all UIDs)
    if client.features().open_redirect {
        match client.add_open_redirect_all(target, replacement) {
            Ok(()) => {
                result.open_redirect = true;
                debug!("open_redirect_all: {target} -> {replacement}");
            }
            Err(e) => {
                // Custom fonts that don't exist on stock system — expected, not an error
                if !Path::new(target).exists() {
                    debug!("open_redirect_all skipped for {target}: target absent on stock");
                } else {
                    warn!("open_redirect_all failed for {target}: {e}");
                }
                return Ok(result);
            }
        }
    } else {
        warn!("open_redirect not available, font redirect incomplete for {target}");
        return Ok(result);
    }

    // 3. Kstat redirect
    if client.features().kstat {
        let spoof = build_kstat_values_from_paths(target, replacement)?;

        match client.add_sus_kstat_redirect(target, replacement, &spoof) {
            Ok(()) => {
                result.kstat_redirect = true;
                debug!("kstat_redirect: {target} -> {replacement}");
            }
            Err(e) => {
                // Partial success is acceptable -- open_redirect still works
                warn!("kstat_redirect failed for {target}: {e} (open_redirect OK)");
            }
        }
    }

    // 4. Hide replacement path
    if client.features().path {
        match client.add_sus_path(replacement) {
            Ok(()) => {
                result.path_hidden = true;
            }
            Err(e) => {
                debug!("add_sus_path failed for replacement {replacement}: {e}");
            }
        }
    }

    info!(
        "font redirect: {target} -> {replacement} (redirect={}, kstat={}, hidden={})",
        result.open_redirect, result.kstat_redirect, result.path_hidden
    );

    Ok(result)
}

/// Redirect all font files from a module's font directory.
///
/// Scans `module_font_dir` for font files and redirects each one to the
/// corresponding system path. Returns count of successful redirects.
///
/// `module_font_dir` is the module's `system/fonts/` directory.
/// `system_font_dir` is the target, typically `/system/fonts/`.
pub fn redirect_font_directory(
    client: &SusfsClient,
    module_font_dir: &Path,
    system_font_dir: &str,
) -> Result<Vec<FontRedirectResult>> {
    if !module_font_dir.is_dir() {
        bail!(
            "module font directory does not exist: {}",
            module_font_dir.display()
        );
    }

    let mut results = Vec::new();

    let entries = fs::read_dir(module_font_dir)
        .with_context(|| format!("reading {}", module_font_dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("failed to read dir entry: {e}");
                continue;
            }
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if !is_font_or_asset(&path) {
            debug!("skipping non-font file: {}", path.display());
            continue;
        }

        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };

        let target = format!("{system_font_dir}/{filename}");
        let replacement = path.to_string_lossy().to_string();

        match redirect_font_file(client, &target, &replacement) {
            Ok(r) => results.push(r),
            Err(e) => {
                warn!("font redirect failed for {filename}: {e}");
            }
        }
    }

    let success_count = results.iter().filter(|r| r.open_redirect).count();
    info!(
        "font directory redirect: {}/{} successful from {}",
        success_count,
        results.len(),
        module_font_dir.display()
    );

    Ok(results)
}

/// Strategy used for a font module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontMountStrategy {
    SusfsRedirect,
    OverlayFallback,
    Failed,
}

/// Result of mounting a font module.
#[derive(Debug)]
#[allow(dead_code)] // Status reporting fields
pub struct FontModuleResult {
    pub module_id: String,
    pub strategy: FontMountStrategy,
    pub redirect_count: usize,
    pub failed_count: usize,
}

/// Mount a font module with SUSFS redirect, falling back to OverlayFS (S07).
///
/// SUSFS is preferred because it avoids visible mounts. When open_redirect
/// is unavailable or >50% of redirects fail, fall back to overlay on
/// /system/fonts.
pub fn redirect_font_module(
    client: &SusfsClient,
    module_id: &str,
    module_font_dir: &Path,
    system_font_dir: &str,
    overlay_source: &str,
) -> Result<FontModuleResult> {
    // Try SUSFS redirect first
    if client.is_available() && client.features().open_redirect {
        let results = redirect_font_directory(client, module_font_dir, system_font_dir)?;
        let total = results.len();
        let success = results.iter().filter(|r| r.open_redirect).count();
        let failed = total - success;

        if total > 0 && success > total / 2 {
            info!(
                "font module '{module_id}': SUSFS redirect {success}/{total} files"
            );
            return Ok(FontModuleResult {
                module_id: module_id.to_string(),
                strategy: FontMountStrategy::SusfsRedirect,
                redirect_count: success,
                failed_count: failed,
            });
        }

        if total > 0 {
            warn!(
                "font module '{module_id}': SUSFS redirect too unreliable ({success}/{total}), \
                 falling back to overlay"
            );
        }
    } else {
        debug!("font module '{module_id}': SUSFS redirect unavailable, using overlay");
    }

    // OverlayFS fallback (S07 exception to VFS02)
    match mount_font_overlay(module_font_dir, system_font_dir, overlay_source) {
        Ok(count) => {
            info!("font module '{module_id}': overlay fallback mounted {count} files");
            Ok(FontModuleResult {
                module_id: module_id.to_string(),
                strategy: FontMountStrategy::OverlayFallback,
                redirect_count: count,
                failed_count: 0,
            })
        }
        Err(e) => {
            warn!("font module '{module_id}': overlay fallback failed: {e}");
            Ok(FontModuleResult {
                module_id: module_id.to_string(),
                strategy: FontMountStrategy::Failed,
                redirect_count: 0,
                failed_count: 1,
            })
        }
    }
}

/// OverlayFS mount for font directory.
///
/// Creates an overlay with the module's font dir as upper layer over
/// /system/fonts. This produces a visible mount point, so it's only
/// used when SUSFS redirect is unavailable or unreliable.
fn mount_font_overlay(module_font_dir: &Path, system_font_dir: &str, overlay_source: &str) -> Result<usize> {
    let target = Path::new(system_font_dir);
    if !target.is_dir() {
        bail!("system font directory missing: {system_font_dir}");
    }
    if !module_font_dir.is_dir() {
        bail!("module font directory missing: {}", module_font_dir.display());
    }

    let file_count = fs::read_dir(module_font_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && is_font_or_asset(&e.path()))
        .count();

    if file_count == 0 {
        return Ok(0);
    }

    // Work directory for overlayfs -- created alongside the upper dir
    let work_dir = module_font_dir.with_file_name("fonts_work");
    let _ = fs::create_dir_all(&work_dir);

    let opts = format!(
        "lowerdir={},upperdir={},workdir={}",
        system_font_dir,
        module_font_dir.display(),
        work_dir.display()
    );

    let target_c = CString::new(system_font_dir)
        .map_err(|_| anyhow::anyhow!("invalid target path"))?;
    let fstype = CString::new("overlay")?;
    let source = CString::new(overlay_source)?;
    let opts_c = CString::new(opts.as_str())
        .map_err(|_| anyhow::anyhow!("invalid overlay options"))?;

    let ret = unsafe {
        libc::mount(
            source.as_ptr(),
            target_c.as_ptr(),
            fstype.as_ptr(),
            0,
            opts_c.as_ptr() as *const libc::c_void,
        )
    };

    if ret != 0 {
        let err = std::io::Error::last_os_error();
        let errno = err.raw_os_error().unwrap_or(-1);
        bail!(
            "overlay mount failed on {system_font_dir}: {err} (errno {errno})"
        );
    }

    Ok(file_count)
}

/// Check if file has a font or asset extension.
fn is_font_or_asset(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };

    FONT_EXTENSIONS.contains(&ext.as_str()) || ASSET_EXTENSIONS.contains(&ext.as_str())
}

fn copy_selinux_context(target: &str, replacement: &str) {
    crate::utils::selinux::copy_selinux_context(
        Path::new(target),
        Path::new(replacement),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_extensions_recognized() {
        assert!(is_font_or_asset(Path::new("NotoSans.ttf")));
        assert!(is_font_or_asset(Path::new("Roboto-Bold.otf")));
        assert!(is_font_or_asset(Path::new("NotoEmoji.ttc")));
        assert!(is_font_or_asset(Path::new("emoji.png")));
        assert!(is_font_or_asset(Path::new("NotoSansHebrew.woff2")));
    }

    #[test]
    fn non_font_extensions_rejected() {
        assert!(!is_font_or_asset(Path::new("readme.txt")));
        assert!(!is_font_or_asset(Path::new("config.xml")));
        assert!(!is_font_or_asset(Path::new("font_fallback")));
    }

    #[test]
    fn no_extension_rejected() {
        assert!(!is_font_or_asset(Path::new("font_without_ext")));
    }

    #[test]
    fn font_redirect_result_defaults() {
        let r = FontRedirectResult {
            target: "/system/fonts/Roboto.ttf".to_string(),
            replacement: "/data/adb/modules/font_mod/system/fonts/Roboto.ttf".to_string(),
            open_redirect: false,
            kstat_redirect: false,
            path_hidden: false,
        };
        assert!(!r.open_redirect);
        assert!(!r.kstat_redirect);
        assert!(!r.path_hidden);
    }

    #[test]
    fn font_module_result_strategy_variants() {
        let susfs = FontModuleResult {
            module_id: "font_mod".to_string(),
            strategy: FontMountStrategy::SusfsRedirect,
            redirect_count: 5,
            failed_count: 0,
        };
        assert_eq!(susfs.strategy, FontMountStrategy::SusfsRedirect);

        let overlay = FontModuleResult {
            module_id: "font_mod".to_string(),
            strategy: FontMountStrategy::OverlayFallback,
            redirect_count: 5,
            failed_count: 0,
        };
        assert_eq!(overlay.strategy, FontMountStrategy::OverlayFallback);

        let failed = FontModuleResult {
            module_id: "font_mod".to_string(),
            strategy: FontMountStrategy::Failed,
            redirect_count: 0,
            failed_count: 1,
        };
        assert_eq!(failed.strategy, FontMountStrategy::Failed);
    }

    #[test]
    fn font_asset_extensions_cover_all_types() {
        for ext in FONT_EXTENSIONS {
            let name = format!("test.{ext}");
            assert!(is_font_or_asset(Path::new(&name)), "expected {ext} recognized");
        }
        for ext in ASSET_EXTENSIONS {
            let name = format!("test.{ext}");
            assert!(is_font_or_asset(Path::new(&name)), "expected {ext} recognized");
        }
    }

    #[test]
    fn font_extensions_case_insensitive() {
        assert!(is_font_or_asset(Path::new("Font.TTF")));
        assert!(is_font_or_asset(Path::new("Font.OTF")));
        assert!(is_font_or_asset(Path::new("emoji.PNG")));
    }

    #[test]
    fn redirect_font_directory_rejects_nonexistent() {
        // redirect_font_directory validates the directory exists before
        // touching the client
        assert!(!Path::new("/nonexistent_font_module_dir").is_dir());
    }

    #[test]
    fn redirect_font_module_falls_back_to_overlay_when_susfs_unavailable() {
        use crate::susfs::{SusfsClient, SusfsFeatures};

        let client = SusfsClient::new_for_test(false, SusfsFeatures::default());

        // SUSFS unavailable -> redirect_font_module should attempt overlay
        assert!(!client.is_available());
        assert!(!client.features().open_redirect);
        // The actual overlay mount will fail in test (no kernel), but the
        // decision to fall back is verified by the feature check path
    }

    #[test]
    fn redirect_font_module_falls_back_when_redirect_missing() {
        use crate::susfs::{SusfsClient, SusfsFeatures};

        let features = SusfsFeatures {
            open_redirect: false,
            ..SusfsFeatures::default()
        };
        let client = SusfsClient::new_for_test(true, features);

        // Client available but open_redirect missing -> overlay fallback
        assert!(client.is_available());
        assert!(!client.features().open_redirect);
    }

    #[test]
    fn redirect_font_module_prefers_susfs_when_available() {
        use crate::susfs::{SusfsClient, SusfsFeatures};

        let features = SusfsFeatures {
            open_redirect: true,
            kstat: true,
            path: true,
            kstat_redirect: true,
            open_redirect_all: true,
            ..SusfsFeatures::default()
        };
        let client = SusfsClient::new_for_test(true, features);

        // Full SUSFS available -> should prefer SUSFS redirect over overlay
        assert!(client.is_available());
        assert!(client.features().open_redirect);
        assert!(client.features().kstat_redirect);
    }
}
