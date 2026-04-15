use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::Path;

use anyhow::{Context, Result};

use crate::core::config::ZeroMountConfig;

use super::translate;

pub(super) const BASE_DIR: &str = "/data/adb/brene";
pub(super) const CONFIG_FILE: &str = "config.sh";

pub(super) const TXT_FILES: &[&str] = &[
    "custom_sus_path.txt",
    "custom_sus_map.txt",
    "custom_sus_path_loop.txt",
];

const CONFIG_PREFIX: &str = "config_";

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
        if let Some((raw_key, value)) = trimmed.split_once('=') {
            // Strip config_ prefix in returned keys
            let key = raw_key.strip_prefix(CONFIG_PREFIX).unwrap_or(raw_key);
            map.insert(key.to_string(), value.to_string());
        }
    }
    tracing::debug!(keys = map.len(), "read BRENE config.sh");
    Ok(map)
}

pub(super) fn write_config(dir: &Path, config: &ZeroMountConfig) -> Result<()> {
    let keys = config_to_keys(config);
    let path = dir.join(CONFIG_FILE);

    let mut lines = Vec::with_capacity(22);
    for &key in BRIDGED_KEY_ORDER {
        if let Some(val) = keys.get(key) {
            lines.push(format!("{CONFIG_PREFIX}{key}={val}"));
        }
    }
    lines.push(format!("{CONFIG_PREFIX}developer_options={}", translate::bool_to_int(config.adb.developer_options)));

    lines.push(String::new());
    fs::write(&path, lines.join("\n"))
        .with_context(|| format!("writing {}", path.display()))?;
    tracing::debug!("wrote BRENE config.sh");
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
            "enable_avc_log_spoofing" | "hide_modules_img"
            | "hide_sus_mnts_for_non_su_procs" => {
                our_val.clone()
            }
            // String keys: preserve external non-empty/non-default
            "custom_uname_kernel_release" | "custom_uname_kernel_version" => {
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
            // false-default keys: preserve external value if set to 1
            "enable_log" | "uname_spoofing" | "uname2_spoofing"
            | "custom_uname_spoofing" => {
                if let Some(ext) = existing.get(key.as_str()) {
                    if ext == "1" {
                        ext.clone()
                    } else {
                        our_val.clone()
                    }
                } else {
                    our_val.clone()
                }
            }
            // All other true-default keys: always write zeromount's value
            _ => our_val.clone(),
        };
        merged.insert(key.clone(), final_val);
    }

    let path = dir.join(CONFIG_FILE);
    let mut lines = Vec::with_capacity(22);
    for &key in BRIDGED_KEY_ORDER {
        if let Some(val) = merged.get(key) {
            lines.push(format!("{CONFIG_PREFIX}{key}={val}"));
        }
    }
    lines.push(format!("{CONFIG_PREFIX}developer_options={}", translate::bool_to_int(config.adb.developer_options)));
    lines.push(String::new());
    fs::write(&path, lines.join("\n"))
        .with_context(|| format!("writing merged {}", path.display()))?;
    tracing::debug!("merged BRENE config.sh");
    Ok(())
}

pub(super) fn ensure_txt_files(dir: &Path) -> Result<()> {
    for name in TXT_FILES {
        let path = dir.join(name);
        if !path.exists() {
            fs::write(&path, "")
                .with_context(|| format!("creating {}", path.display()))?;
            tracing::debug!(file = name, "created empty txt file");
        }
    }
    Ok(())
}

fn config_to_keys(config: &ZeroMountConfig) -> HashMap<String, String> {
    let mut m = HashMap::with_capacity(20);

    // 18 bridged keys per spec Section 3b (keys stored WITHOUT config_ prefix)
    m.insert("enable_avc_log_spoofing".into(), translate::bool_to_int(config.brene.avc_log_spoofing).to_string());
    m.insert("hide_custom_recovery_folders".into(), translate::bool_to_int(config.brene.auto_hide_recovery).to_string());
    m.insert("hide_data_local_tmp".into(), translate::bool_to_int(config.brene.auto_hide_tmp).to_string());
    m.insert("hide_rooted_app_folders".into(), translate::bool_to_int(config.brene.auto_hide_rooted_folders).to_string());
    m.insert("hide_sdcard_android_data".into(), translate::bool_to_int(config.brene.emulate_vold_app_data).to_string());
    m.insert("hide_sus_mnts_for_non_su_procs".into(), translate::bool_to_int(config.brene.hide_sus_mounts).to_string());
    m.insert("kernel_umount".into(), translate::bool_to_int(config.brene.kernel_umount).to_string());
    m.insert("try_umount".into(), translate::bool_to_int(config.brene.try_umount).to_string());

    // Uname: 3 mutually exclusive booleans
    let (uname, uname2, custom) = translate::uname_mode_to_brene_triple(
        &config.uname.mode,
        &config.uname.release,
    );
    m.insert("uname_spoofing".into(), uname.to_string());
    m.insert("uname2_spoofing".into(), uname2.to_string());
    m.insert("custom_uname_spoofing".into(), custom.to_string());

    m.insert("hide_zygisk_modules".into(), translate::bool_to_int(config.brene.auto_hide_zygisk).to_string());
    m.insert("hide_injections".into(), translate::bool_to_int(config.brene.auto_hide_injections).to_string());
    m.insert("usb_debugging".into(), translate::bool_to_int(config.adb.usb_debugging).to_string());
    m.insert("developer_options".into(), translate::bool_to_int(config.adb.developer_options).to_string());
    m.insert("enable_log".into(), translate::bool_to_int(config.brene.susfs_log).to_string());
    m.insert("hide_modules_img".into(), translate::bool_to_int(config.brene.hide_ksu_loops).to_string());
    m.insert("custom_uname_kernel_release".into(), translate::string_to_external(&config.uname.release));
    m.insert("custom_uname_kernel_version".into(), translate::string_to_external(&config.uname.version));

    m
}

pub(super) fn apply_keys_to_config(keys: &HashMap<String, String>, config: &mut ZeroMountConfig) -> bool {
    let mut changed = false;

    if let Some(v) = keys.get("enable_avc_log_spoofing") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.avc_log_spoofing != val {
            config.brene.avc_log_spoofing = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_custom_recovery_folders") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.auto_hide_recovery != val {
            config.brene.auto_hide_recovery = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_data_local_tmp") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.auto_hide_tmp != val {
            config.brene.auto_hide_tmp = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_rooted_app_folders") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.auto_hide_rooted_folders != val {
            config.brene.auto_hide_rooted_folders = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_sdcard_android_data") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.emulate_vold_app_data != val {
            config.brene.emulate_vold_app_data = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_sus_mnts_for_non_su_procs") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.hide_sus_mounts != val {
            config.brene.hide_sus_mounts = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("kernel_umount") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.kernel_umount != val {
            config.brene.kernel_umount = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("try_umount") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.try_umount != val {
            config.brene.try_umount = val;
            changed = true;
        }
    }

    // Uname: reconstruct mode from 3 booleans
    let uname_val = keys.get("uname_spoofing").and_then(|v| v.parse().ok()).unwrap_or(0u8);
    let uname2_val = keys.get("uname2_spoofing").and_then(|v| v.parse().ok()).unwrap_or(0u8);
    let custom_val = keys.get("custom_uname_spoofing").and_then(|v| v.parse().ok()).unwrap_or(0u8);
    if keys.contains_key("uname_spoofing")
        || keys.contains_key("uname2_spoofing")
        || keys.contains_key("custom_uname_spoofing")
    {
        let val = translate::uname_mode_from_brene_triple(uname_val, uname2_val, custom_val);
        if config.uname.mode != val {
            config.uname.mode = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_zygisk_modules") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.auto_hide_zygisk != val {
            config.brene.auto_hide_zygisk = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_injections") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.auto_hide_injections != val {
            config.brene.auto_hide_injections = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("usb_debugging") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.adb.usb_debugging != val {
            config.adb.usb_debugging = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("developer_options") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.adb.developer_options != val {
            config.adb.developer_options = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("enable_log") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.susfs_log != val {
            config.brene.susfs_log = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("hide_modules_img") {
        let val = translate::int_to_bool(v.parse().unwrap_or(0));
        if config.brene.hide_ksu_loops != val {
            config.brene.hide_ksu_loops = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("custom_uname_kernel_release") {
        let val = translate::normalize_string_value(v);
        if config.uname.release != val {
            config.uname.release = val;
            changed = true;
        }
    }

    if let Some(v) = keys.get("custom_uname_kernel_version") {
        let val = translate::normalize_string_value(v);
        if config.uname.version != val {
            config.uname.version = val;
            changed = true;
        }
    }

    changed
}

// Key order WITHOUT config_ prefix (prefix added on write)
const BRIDGED_KEY_ORDER: &[&str] = &[
    "enable_avc_log_spoofing",
    "hide_custom_recovery_folders",
    "hide_data_local_tmp",
    "hide_rooted_app_folders",
    "hide_sdcard_android_data",
    "hide_sus_mnts_for_non_su_procs",
    "kernel_umount",
    "try_umount",
    "uname_spoofing",
    "uname2_spoofing",
    "custom_uname_spoofing",
    "hide_zygisk_modules",
    "hide_injections",
    "usb_debugging",
    "developer_options",
    "enable_log",
    "hide_modules_img",
    "custom_uname_kernel_release",
    "custom_uname_kernel_version",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::UnameMode;
    use std::io::Write;
    use tempfile::TempDir;

    fn sample_config() -> ZeroMountConfig {
        let mut c = ZeroMountConfig::default();
        c.brene.avc_log_spoofing = true;
        c.brene.auto_hide_recovery = true;
        c.brene.auto_hide_tmp = true;
        c.brene.auto_hide_rooted_folders = true;
        c.brene.hide_sus_mounts = true;
        c.brene.kernel_umount = true;
        c.brene.try_umount = false;
        c.brene.auto_hide_zygisk = true;
        c.brene.auto_hide_injections = true;
        c.adb.usb_debugging = true;
        c.adb.developer_options = true;
        c.brene.susfs_log = false;
        c.brene.hide_ksu_loops = true;
        c.uname.mode = UnameMode::Static;
        c.uname.release = "5.10.0-gki".into();
        c.uname.version = "#1 SMP".into();
        c
    }

    #[test]
    fn config_to_keys_maps_all_bridged() {
        let keys = config_to_keys(&sample_config());
        assert_eq!(keys["enable_avc_log_spoofing"], "1");
        assert_eq!(keys["hide_custom_recovery_folders"], "1");
        assert_eq!(keys["hide_data_local_tmp"], "1");
        assert_eq!(keys["hide_rooted_app_folders"], "1");
        assert_eq!(keys["hide_sdcard_android_data"], "1");
        assert_eq!(keys["hide_sus_mnts_for_non_su_procs"], "1");
        assert_eq!(keys["kernel_umount"], "1");
        // Static + custom release -> (0, 1, 0)
        assert_eq!(keys["uname_spoofing"], "0");
        assert_eq!(keys["uname2_spoofing"], "1");
        assert_eq!(keys["custom_uname_spoofing"], "0");
        assert_eq!(keys["hide_zygisk_modules"], "1");
        assert_eq!(keys["hide_injections"], "1");
        assert_eq!(keys["usb_debugging"], "1");
        assert_eq!(keys["developer_options"], "1");
        assert_eq!(keys["enable_log"], "0");
        assert_eq!(keys["hide_modules_img"], "1");
        assert_eq!(keys["custom_uname_kernel_release"], "'5.10.0-gki'");
        assert_eq!(keys["custom_uname_kernel_version"], "'#1 SMP'");
        assert_eq!(keys["try_umount"], "0");
        assert_eq!(keys.len(), 19);
    }

    #[test]
    fn write_uses_config_prefix() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), &sample_config()).unwrap();

        let content = fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        assert!(content.contains("config_enable_avc_log_spoofing=1"));
        assert!(content.contains("config_uname2_spoofing=1"));
        assert!(content.contains("config_developer_options="));
        // Should NOT contain bare keys without prefix
        assert!(!content.contains("\nenable_avc_log_spoofing="));
    }

    #[test]
    fn read_strips_config_prefix() {
        let dir = TempDir::new().unwrap();
        write_config(dir.path(), &sample_config()).unwrap();
        let map = read_config(dir.path()).unwrap();

        // Keys should NOT have config_ prefix
        assert!(map.contains_key("enable_avc_log_spoofing"));
        assert!(!map.contains_key("config_enable_avc_log_spoofing"));
        // developer_options is also stripped
        assert!(map.contains_key("developer_options"));
    }

    #[test]
    fn write_and_read_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = sample_config();

        write_config(dir.path(), &config).unwrap();
        let read = read_config(dir.path()).unwrap();

        assert_eq!(read["enable_avc_log_spoofing"], "1");
        assert_eq!(read["uname2_spoofing"], "1");
        assert_eq!(read["custom_uname_kernel_release"], "'5.10.0-gki'");
    }

    #[test]
    fn apply_keys_detects_changes() {
        let mut config = ZeroMountConfig::default();
        let mut keys = HashMap::new();
        keys.insert("enable_log".into(), "1".into());
        keys.insert("custom_uname_spoofing".into(), "1".into());
        keys.insert("uname_spoofing".into(), "0".into());
        keys.insert("uname2_spoofing".into(), "0".into());
        keys.insert("kernel_umount".into(), "0".into());

        let changed = apply_keys_to_config(&keys, &mut config);
        assert!(changed);
        assert!(config.brene.susfs_log);
        assert_eq!(config.uname.mode, UnameMode::Dynamic);
        assert!(!config.brene.kernel_umount);
    }

    #[test]
    fn apply_keys_no_change_returns_false() {
        let mut config = ZeroMountConfig::default();
        // Default config values should produce no change
        let keys = config_to_keys(&config);
        let changed = apply_keys_to_config(&keys, &mut config);
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
        let path = dir.path().join("custom_sus_path.txt");
        let mut f = fs::File::create(&path).unwrap();
        f.write_all(b"/data/adb/modules\n").unwrap();

        ensure_txt_files(dir.path()).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "/data/adb/modules\n");
    }

    #[test]
    fn merge_preserves_user_strings() {
        let dir = TempDir::new().unwrap();
        let config = sample_config();

        let mut existing = HashMap::new();
        existing.insert("custom_uname_kernel_release".into(), "'6.1.0-custom'".into());

        merge_config(dir.path(), &config, &existing).unwrap();
        let result = read_config(dir.path()).unwrap();

        assert_eq!(result["custom_uname_kernel_release"], "'6.1.0-custom'");
    }

    #[test]
    fn merge_overwrites_opinionated_keys() {
        let dir = TempDir::new().unwrap();
        let config = ZeroMountConfig::default();

        let mut existing = HashMap::new();
        existing.insert("enable_avc_log_spoofing".into(), "0".into());
        existing.insert("hide_modules_img".into(), "0".into());

        merge_config(dir.path(), &config, &existing).unwrap();
        let result = read_config(dir.path()).unwrap();

        assert_eq!(result["enable_avc_log_spoofing"], "1");
        assert_eq!(result["hide_modules_img"], "1");
    }

    #[test]
    fn uname_disabled_triple() {
        let mut config = ZeroMountConfig::default();
        config.uname.mode = UnameMode::Disabled;
        let keys = config_to_keys(&config);
        assert_eq!(keys["uname_spoofing"], "0");
        assert_eq!(keys["uname2_spoofing"], "0");
        assert_eq!(keys["custom_uname_spoofing"], "0");
    }

    #[test]
    fn uname_static_default_release_triple() {
        let mut config = ZeroMountConfig::default();
        config.uname.mode = UnameMode::Static;
        config.uname.release = String::new();
        let keys = config_to_keys(&config);
        assert_eq!(keys["uname_spoofing"], "1");
        assert_eq!(keys["uname2_spoofing"], "0");
        assert_eq!(keys["custom_uname_spoofing"], "0");
    }

    #[test]
    fn uname_dynamic_triple() {
        let mut config = ZeroMountConfig::default();
        config.uname.mode = UnameMode::Dynamic;
        let keys = config_to_keys(&config);
        assert_eq!(keys["uname_spoofing"], "0");
        assert_eq!(keys["uname2_spoofing"], "0");
        assert_eq!(keys["custom_uname_spoofing"], "1");
    }
}
