use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::Path;

use anyhow::{Context, Result};

use crate::core::config::ZeroMountConfig;

use super::translate;

pub(super) const BASE_DIR: &str = "/data/adb/susfs4ksu";
pub(super) const CONFIG_FILE: &str = "config.sh";

pub(super) const TXT_FILES: &[&str] = &[
    "sus_path.txt",
    "sus_maps.txt",
    "sus_path_loop.txt",
    "sus_mount.txt",
    "try_umount.txt",
    "legit_mounts.txt",
    "sus_open_redirect.txt",
];

pub(super) fn read_config(dir: &Path) -> Result<HashMap<String, String>> {
    let path = dir.join(CONFIG_FILE);
    let file = fs::File::open(&path)
        .with_context(|| format!("opening {}", path.display()))?;

    let mut map = HashMap::new();
    for line in std::io::BufReader::new(file).lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    tracing::debug!(keys = map.len(), "read susfs4ksu config.sh");
    Ok(map)
}

pub(super) fn write_config(dir: &Path, config: &ZeroMountConfig) -> Result<()> {
    let keys = config_to_keys(config);
    let path = dir.join(CONFIG_FILE);

    let mut lines = Vec::with_capacity(24);
    // Bridged keys (12)
    for &key in BRIDGED_KEY_ORDER {
        if let Some(val) = keys.get(key) {
            lines.push(format!("{key}={val}"));
        }
    }
    // Hardcoded non-bridged defaults
    for &(key, val) in HARDCODED_DEFAULTS {
        lines.push(format!("{key}={val}"));
    }

    lines.push(String::new());
    fs::write(&path, lines.join("\n"))
        .with_context(|| format!("writing {}", path.display()))?;
    tracing::debug!("wrote susfs4ksu config.sh");
    Ok(())
}

pub(super) fn merge_config(
    dir: &Path,
    config: &ZeroMountConfig,
    existing: &HashMap<String, String>,
) -> Result<()> {
    let ours = config_to_keys(config);
    let mut merged = HashMap::new();

    for (key, our_val) in &ours {
        let final_val = match key.as_str() {
            // true-default keys: always overwrite with zeromount's value
            "avc_log_spoofing" | "force_hide_lsposed" | "hide_loops"
            | "emulate_vold_app_data" | "hide_sus_mnts_for_all_or_non_su_procs"
            | "skip_legit_mounts" => {
                our_val.clone()
            }
            // vbmeta_size: always write zeromount's randomized value
            "vbmeta_size" => our_val.clone(),
            // String keys: preserve external non-empty/non-default
            "kernel_version" | "kernel_build" => {
                if let Some(ext) = existing.get(key.as_str()) {
                    let normalized = translate::normalize_string_value(ext);
                    if normalized.is_empty() {
                        our_val.clone()
                    } else {
                        ext.clone()
                    }
                } else {
                    our_val.clone()
                }
            }
            // false-default keys: preserve external value if non-zero
            "susfs_log" | "spoof_cmdline" | "spoof_uname" | "auto_try_umount"
            | "hide_cusrom" => {
                if let Some(ext) = existing.get(key.as_str()) {
                    let non_zero = ext.parse::<u8>().unwrap_or(0) > 0;
                    if non_zero { ext.clone() } else { our_val.clone() }
                } else {
                    our_val.clone()
                }
            }
            // Non-bridged hardcoded: always write defaults
            _ => our_val.clone(),
        };
        merged.insert(key.clone(), final_val);
    }

    // Write merged values using the same format as write_config
    let path = dir.join(CONFIG_FILE);
    let mut lines = Vec::with_capacity(24);
    for &key in BRIDGED_KEY_ORDER {
        if let Some(val) = merged.get(key) {
            lines.push(format!("{key}={val}"));
        }
    }
    for &(key, _) in HARDCODED_DEFAULTS {
        if let Some(val) = merged.get(key) {
            lines.push(format!("{key}={val}"));
        }
    }
    lines.push(String::new());
    fs::write(&path, lines.join("\n"))
        .with_context(|| format!("writing merged {}", path.display()))?;
    tracing::debug!("merged susfs4ksu config.sh");
    Ok(())
}

const KSTAT_JSON_FILE: &str = "sus_kstat_statically.json";

pub(super) fn ensure_txt_files(dir: &Path) -> Result<()> {
    for name in TXT_FILES {
        let path = dir.join(name);
        if !path.exists() {
            fs::write(&path, "")
                .with_context(|| format!("creating {}", path.display()))?;
            tracing::debug!(file = name, "created empty txt file");
        }
    }
    let json_path = dir.join(KSTAT_JSON_FILE);
    if !json_path.exists() {
        fs::write(&json_path, "[]")
            .with_context(|| format!("creating {}", json_path.display()))?;
        tracing::debug!("created empty sus_kstat_statically.json");
    }
    Ok(())
}

fn config_to_keys(config: &ZeroMountConfig) -> HashMap<String, String> {
    let mut m = HashMap::with_capacity(24);

    // 12 bridged keys per spec Section 3a
    m.insert("susfs_log".into(), translate::bool_to_int(config.brene.susfs_log).to_string());
    m.insert("avc_log_spoofing".into(), translate::bool_to_int(config.brene.avc_log_spoofing).to_string());
    let hide_sus_mnts_val = match (config.brene.hide_sus_mounts, config.brene.hide_sus_mounts_off_after_boot) {
        (true, true) => 2,
        (true, false) => 1,
        _ => 0,
    };
    m.insert("hide_sus_mnts_for_all_or_non_su_procs".into(), hide_sus_mnts_val.to_string());
    m.insert("spoof_uname".into(), translate::uname_mode_to_susfs4ksu(&config.uname.mode).to_string());
    m.insert("kernel_version".into(), translate::string_to_external(&config.uname.release));
    m.insert("kernel_build".into(), translate::string_to_external(&config.uname.version));
    m.insert("spoof_cmdline".into(), translate::bool_to_int(config.brene.spoof_cmdline).to_string());
    m.insert("hide_loops".into(), translate::bool_to_int(config.brene.hide_ksu_loops).to_string());
    m.insert("force_hide_lsposed".into(), translate::bool_to_int(config.brene.force_hide_lsposed).to_string());
    m.insert("vbmeta_size".into(), config.brene.vbmeta_size.to_string());
    let vold_val = match (config.brene.emulate_vold_app_data, config.brene.vold_use_path_loop) {
        (true, true) => 2,
        (true, false) => 1,
        _ => 0,
    };
    m.insert("emulate_vold_app_data".into(), vold_val.to_string());
    m.insert("auto_try_umount".into(), translate::bool_to_int(config.brene.try_umount).to_string());
    m.insert("skip_legit_mounts".into(), translate::bool_to_int(config.brene.skip_legit_mounts).to_string());
    m.insert("hide_cusrom".into(), config.brene.hide_cusrom.to_string());
    m.insert("disable_webui_bin_update".into(), "1".into());

    // Hardcoded non-bridged defaults
    for &(key, val) in HARDCODED_DEFAULTS {
        m.insert(key.into(), val.into());
    }

    m
}

pub(super) fn apply_keys_to_config(keys: &HashMap<String, String>, config: &mut ZeroMountConfig) -> bool {
    let mut changed = false;

    if let Some(v) = keys.get("susfs_log") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.susfs_log != val {
            config.brene.susfs_log = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("avc_log_spoofing") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.avc_log_spoofing != val {
            config.brene.avc_log_spoofing = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_sus_mnts_for_all_or_non_su_procs") {
        let raw: u8 = v.parse().unwrap_or(0);
        let enabled = raw >= 1;
        let off_after_boot = raw == 2;
        if config.brene.hide_sus_mounts != enabled {
            config.brene.hide_sus_mounts = enabled;
            changed = true;
        }
        if config.brene.hide_sus_mounts_off_after_boot != off_after_boot {
            config.brene.hide_sus_mounts_off_after_boot = off_after_boot;
            changed = true;
        }
    }

    if let Some(v) = keys.get("spoof_uname") {
        let val = translate::uname_mode_from_susfs4ksu(v.parse().unwrap_or(0));
        if config.uname.mode != val {
            config.uname.mode = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("kernel_version") {
        let val = translate::normalize_string_value(v);
        if config.uname.release != val {
            config.uname.release = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("kernel_build") {
        let val = translate::normalize_string_value(v);
        if config.uname.version != val {
            config.uname.version = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("spoof_cmdline") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.spoof_cmdline != val {
            config.brene.spoof_cmdline = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_loops") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.hide_ksu_loops != val {
            config.brene.hide_ksu_loops = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("force_hide_lsposed") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.force_hide_lsposed != val {
            config.brene.force_hide_lsposed = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("vbmeta_size") {
        if let Ok(val) = v.parse::<u32>() {
            if config.brene.vbmeta_size != val {
                config.brene.vbmeta_size = val;
                changed = true;
            }
        }
    }

    if let Some(v) = keys.get("emulate_vold_app_data") {
        let raw: u8 = v.parse().unwrap_or(0);
        let enabled = raw >= 1;
        let use_loop = raw == 2;
        if config.brene.emulate_vold_app_data != enabled {
            config.brene.emulate_vold_app_data = enabled;
            changed = true;
        }
        if config.brene.vold_use_path_loop != use_loop {
            config.brene.vold_use_path_loop = use_loop;
            changed = true;
        }
    }

    if let Some(v) = keys.get("auto_try_umount") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.try_umount != val {
            config.brene.try_umount = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("skip_legit_mounts") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.skip_legit_mounts != val {
            config.brene.skip_legit_mounts = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_cusrom") {
        if let Ok(val) = v.parse::<u8>() {
            let clamped = val.min(5);
            if config.brene.hide_cusrom != clamped {
                config.brene.hide_cusrom = clamped;
                changed = true;
            }
        }
    }

    changed
}

// Bridged keys in write order (spec Section 3a)
const BRIDGED_KEY_ORDER: &[&str] = &[
    "susfs_log",
    "avc_log_spoofing",
    "hide_sus_mnts_for_all_or_non_su_procs",
    "spoof_uname",
    "kernel_version",
    "kernel_build",
    "spoof_cmdline",
    "hide_loops",
    "force_hide_lsposed",
    "vbmeta_size",
    "emulate_vold_app_data",
    "auto_try_umount",
    "skip_legit_mounts",
    "hide_cusrom",
    "disable_webui_bin_update",
];

const HARDCODED_DEFAULTS: &[(&str, &str)] = &[
    ("sus_su", "2"),
    ("sus_su_active", "2"),
    ("hide_vendor_sepolicy", "0"),
    ("hide_compat_matrix", "0"),
    ("hide_gapps", "0"),
    ("hide_revanced", "0"),
    ("umount_for_zygote_iso_service", "0"),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::UnameMode;
    use std::io::Write;
    use tempfile::TempDir;

    fn sample_config() -> ZeroMountConfig {
        let mut c = ZeroMountConfig::default();
        c.brene.susfs_log = true;
        c.brene.avc_log_spoofing = true;
        c.brene.hide_sus_mounts = true;
        c.brene.spoof_cmdline = false;
        c.brene.hide_ksu_loops = true;
        c.brene.force_hide_lsposed = true;
        c.brene.vbmeta_size = 8192;
        c.brene.emulate_vold_app_data = true;
        c.uname.mode = UnameMode::Static;
        c.uname.release = "5.10.0-gki".to_string();
        c.uname.version = "#1 SMP".to_string();
        c
    }

    #[test]
    fn config_to_keys_maps_all_bridged() {
        let keys = config_to_keys(&sample_config());
        assert_eq!(keys["susfs_log"], "1");
        assert_eq!(keys["avc_log_spoofing"], "1");
        assert_eq!(keys["hide_sus_mnts_for_all_or_non_su_procs"], "1");
        assert_eq!(keys["spoof_uname"], "1");
        assert_eq!(keys["kernel_version"], "'5.10.0-gki'");
        assert_eq!(keys["kernel_build"], "'#1 SMP'");
        assert_eq!(keys["spoof_cmdline"], "0");
        assert_eq!(keys["hide_loops"], "1");
        assert_eq!(keys["force_hide_lsposed"], "1");
        assert_eq!(keys["vbmeta_size"], "8192");
        assert_eq!(keys["emulate_vold_app_data"], "2"); // vold_use_path_loop defaults true
        assert_eq!(keys["auto_try_umount"], "0");
        assert_eq!(keys["skip_legit_mounts"], "1"); // defaults true
        assert_eq!(keys["hide_cusrom"], "0");
        assert_eq!(keys["disable_webui_bin_update"], "1");
        // Hardcoded
        assert_eq!(keys["sus_su"], "2");
        assert_eq!(keys["sus_su_active"], "2");
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = sample_config();

        write_config(dir.path(), &config).unwrap();
        let read = read_config(dir.path()).unwrap();

        assert_eq!(read["susfs_log"], "1");
        assert_eq!(read["spoof_uname"], "1");
        assert_eq!(read["kernel_version"], "'5.10.0-gki'");
        assert_eq!(read["vbmeta_size"], "8192");
        assert_eq!(read["sus_su"], "2");
        assert_eq!(read["disable_webui_bin_update"], "1");
    }

    #[test]
    fn apply_keys_detects_changes() {
        let mut config = ZeroMountConfig::default();
        let mut keys = HashMap::new();
        keys.insert("susfs_log".into(), "1".into());
        keys.insert("spoof_uname".into(), "1".into());
        keys.insert("kernel_version".into(), "'5.15.0'".into());
        keys.insert("vbmeta_size".into(), "6144".into());

        let changed = apply_keys_to_config(&keys, &mut config);
        assert!(changed);
        assert!(config.brene.susfs_log);
        assert_eq!(config.uname.mode, UnameMode::Static);
        assert_eq!(config.uname.release, "5.15.0");
        assert_eq!(config.brene.vbmeta_size, 6144);
    }

    #[test]
    fn apply_keys_no_change_returns_false() {
        let config = ZeroMountConfig::default();
        let keys = config_to_keys(&config);

        // Parse values back through the same path
        let mut parsed_keys = HashMap::new();
        for (k, v) in &keys {
            // Only include bridged keys that apply_keys_to_config handles
            if BRIDGED_KEY_ORDER.contains(&k.as_str()) && k != "disable_webui_bin_update" {
                parsed_keys.insert(k.clone(), v.clone());
            }
        }

        let mut config2 = ZeroMountConfig::default();
        let changed = apply_keys_to_config(&parsed_keys, &mut config2);
        assert!(!changed);
    }

    #[test]
    fn ensure_txt_files_creates_missing() {
        let dir = TempDir::new().unwrap();
        ensure_txt_files(dir.path()).unwrap();

        for name in TXT_FILES {
            assert!(dir.path().join(name).exists(), "{name} should exist");
        }
    }

    #[test]
    fn ensure_txt_files_preserves_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("sus_path.txt");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(b"/data/adb/modules\n").unwrap();

        ensure_txt_files(dir.path()).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "/data/adb/modules\n");
    }

    #[test]
    fn read_config_skips_comments() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        fs::write(&path, "# comment\nsusfs_log=1\n\navc_log_spoofing=0\n").unwrap();

        let map = read_config(dir.path()).unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map["susfs_log"], "1");
        assert_eq!(map["avc_log_spoofing"], "0");
    }

    #[test]
    fn merge_preserves_user_strings() {
        let dir = TempDir::new().unwrap();
        let config = sample_config();

        let mut existing = HashMap::new();
        existing.insert("kernel_version".into(), "'6.1.0-custom'".into());
        existing.insert("susfs_log".into(), "1".into());

        merge_config(dir.path(), &config, &existing).unwrap();
        let result = read_config(dir.path()).unwrap();

        // User's custom kernel string preserved
        assert_eq!(result["kernel_version"], "'6.1.0-custom'");
        // true-default key: zeromount always overwrites
        assert_eq!(result["avc_log_spoofing"], "1");
    }

    #[test]
    fn merge_overwrites_opinionated_keys() {
        let dir = TempDir::new().unwrap();
        let config = ZeroMountConfig::default();

        let mut existing = HashMap::new();
        existing.insert("avc_log_spoofing".into(), "0".into());
        existing.insert("force_hide_lsposed".into(), "0".into());
        existing.insert("hide_loops".into(), "0".into());

        merge_config(dir.path(), &config, &existing).unwrap();
        let result = read_config(dir.path()).unwrap();

        assert_eq!(result["avc_log_spoofing"], "1");
        assert_eq!(result["force_hide_lsposed"], "1");
        assert_eq!(result["hide_loops"], "1");
    }
}
