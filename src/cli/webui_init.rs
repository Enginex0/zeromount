use std::io::BufRead;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::bridge::BridgeValues;
use crate::core::config::ZeroMountConfig;
use crate::core::types::{ExternalSusfsModule, RuntimeState};

const MODULES_DIR: &str = "/data/adb/modules";
const EXCLUSION_FILE: &str = "/data/adb/zeromount/.exclusion_list";
const EXCLUSION_META: &str = "/data/adb/zeromount/.exclusion_meta.json";
const ACTIVITY_LOG: &str = "/data/adb/zeromount/activity.log";

#[derive(Serialize)]
struct WebUiInitResponse {
    pub status: RuntimeState,
    pub config: ZeroMountConfig,
    pub system_info: WebUiSystemInfo,
    pub rules: Vec<WebUiRule>,
    pub excluded_uids: Vec<WebUiExcludedUid>,
    pub activity: Vec<WebUiActivityItem>,
    pub modules: Vec<WebUiModule>,
    pub emoji_conflict: Option<String>,
    pub bridge_values: Option<BridgeValues>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebUiSystemInfo {
    pub kernel_version: String,
    pub uptime: String,
    pub device_model: String,
    pub android_version: String,
    pub selinux_status: String,
}

#[derive(Serialize)]
struct WebUiRule {
    pub id: String,
    pub name: String,
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebUiExcludedUid {
    pub uid: u32,
    pub package_name: String,
    pub app_name: String,
    pub excluded_at: String,
}

#[derive(Serialize)]
struct WebUiActivityItem {
    pub id: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebUiModule {
    pub name: String,
    pub path: String,
    pub has_system: bool,
    pub has_vendor: bool,
    pub has_product: bool,
    pub is_loaded: bool,
    pub file_count: usize,
}

pub fn handle_webui_init() -> Result<()> {
    let status = super::handlers::build_runtime_status();
    let config = ZeroMountConfig::load(None)?;
    let system_info = collect_system_info();
    let rules = collect_rules();
    let excluded_uids = read_exclusion_files();
    let activity = read_activity_log();
    let modules = build_module_list(&rules);

    let emoji_conflict = status.font_modules.first().cloned();

    let external_module = status.capabilities.external_susfs_module;
    let bridge_values = match external_module {
        ExternalSusfsModule::None => None,
        _ => crate::bridge::read_bridge_values(external_module)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "failed to read bridge values");
                None
            }),
    };

    let response = WebUiInitResponse {
        status,
        config,
        system_info,
        rules,
        excluded_uids,
        activity,
        modules,
        emoji_conflict,
        bridge_values,
    };

    let json = serde_json::to_string(&response)?;
    println!("{json}");
    Ok(())
}

fn collect_system_info() -> WebUiSystemInfo {
    let kernel_version = std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from))
        .unwrap_or_default();

    let uptime = std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            let secs = s.split_whitespace().next()?.parse::<f64>().ok()? as u64;
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            Some(format!("{hours}h {mins}m"))
        })
        .unwrap_or_default();

    let device_model = getprop("ro.product.model");
    let android_version = getprop("ro.build.version.release");

    let selinux_status = std::fs::read_to_string("/sys/fs/selinux/enforce")
        .ok()
        .map(|s| match s.trim() {
            "1" => "Enforcing".to_string(),
            "0" => "Permissive".to_string(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| "Disabled".to_string());

    WebUiSystemInfo {
        kernel_version,
        uptime,
        device_model,
        android_version,
        selinux_status,
    }
}

fn getprop(key: &str) -> String {
    use crate::utils::command::run_command_with_timeout;
    use std::process::Command;
    use std::time::Duration;

    let mut cmd = Command::new("getprop");
    cmd.arg(key);
    run_command_with_timeout(&mut cmd, Duration::from_secs(5))
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn collect_rules() -> Vec<WebUiRule> {
    let driver = match crate::vfs::VfsDriver::open() {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let list = match driver.get_list() {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };

    list.lines()
        .filter(|l| !l.is_empty())
        .enumerate()
        .map(|(i, line)| {
            let idx = line.find("->");
            let (source, target) = match idx {
                Some(pos) => (line[..pos].trim().to_string(), line[pos + 2..].trim().to_string()),
                None => (line.trim().to_string(), "[BLOCKED]".to_string()),
            };
            let name = target.rsplit('/').next().unwrap_or("Rule").to_string();
            WebUiRule {
                id: (i + 1).to_string(),
                name,
                source,
                target,
            }
        })
        .collect()
}

fn read_exclusion_files() -> Vec<WebUiExcludedUid> {
    let list_content = match std::fs::read_to_string(EXCLUSION_FILE) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let uids: Vec<u32> = list_content
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    if uids.is_empty() {
        return Vec::new();
    }

    #[derive(serde::Deserialize)]
    struct UidMeta {
        #[serde(rename = "packageName")]
        package_name: Option<String>,
        #[serde(rename = "appName")]
        app_name: Option<String>,
        #[serde(rename = "excludedAt")]
        excluded_at: Option<String>,
    }

    let meta: std::collections::HashMap<String, UidMeta> = std::fs::read_to_string(EXCLUSION_META)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    uids.into_iter()
        .map(|uid| {
            let key = uid.to_string();
            let info = meta.get(&key);
            WebUiExcludedUid {
                uid,
                package_name: info
                    .and_then(|m| m.package_name.clone())
                    .unwrap_or_else(|| format!("app_{uid}")),
                app_name: info
                    .and_then(|m| m.app_name.clone())
                    .unwrap_or_else(|| format!("UID {uid}")),
                excluded_at: info
                    .and_then(|m| m.excluded_at.clone())
                    .unwrap_or_default(),
            }
        })
        .collect()
}

fn read_activity_log() -> Vec<WebUiActivityItem> {
    let file = match std::fs::File::open(ACTIVITY_LOG) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let reader = std::io::BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().filter_map(|l| l.ok()).collect();
    let start = all_lines.len().saturating_sub(10);
    let tail = &all_lines[start..];

    let mut items: Vec<WebUiActivityItem> = Vec::new();
    for (i, line) in tail.iter().enumerate() {
        // Format: [timestamp] TYPE: message
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let close = match line.find(']') {
            Some(pos) => pos,
            None => continue,
        };
        if !line.starts_with('[') {
            continue;
        }
        let timestamp = &line[1..close];
        let rest = line[close + 1..].trim();
        let colon = match rest.find(':') {
            Some(pos) => pos,
            None => continue,
        };
        let item_type = rest[..colon].trim().to_lowercase();
        let message = rest[colon + 1..].trim().to_string();

        items.push(WebUiActivityItem {
            id: (i + 1).to_string(),
            item_type,
            message,
            timestamp: timestamp.to_string(),
        });
    }

    items.reverse();
    items
}

fn build_module_list(rules: &[WebUiRule]) -> Vec<WebUiModule> {
    let modules_dir = Path::new(MODULES_DIR);
    if !modules_dir.exists() {
        return Vec::new();
    }

    let loaded_module_paths: std::collections::HashSet<String> = rules
        .iter()
        .filter_map(|r| {
            // source looks like /data/adb/modules/<name>/system/... — extract base path
            let parts: Vec<&str> = r.source.splitn(6, '/').collect();
            if parts.len() >= 5 {
                Some(format!("/{}/{}/{}/{}", parts[1], parts[2], parts[3], parts[4]))
            } else {
                None
            }
        })
        .collect();

    let entries = match std::fs::read_dir(modules_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut modules = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name_os = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };

        let has_system = path.join("system").is_dir();
        let has_vendor = path.join("vendor").is_dir();
        let has_product = path.join("product").is_dir();

        if !has_system && !has_vendor && !has_product {
            continue;
        }

        let file_count = count_partition_files(&path);

        let display_name = path
            .join("module.prop")
            .exists()
            .then(|| {
                std::fs::read_to_string(path.join("module.prop"))
                    .ok()
                    .and_then(|content| {
                        content.lines().find_map(|l| {
                            l.strip_prefix("name=").map(|v| v.trim().to_string())
                        })
                    })
            })
            .flatten()
            .unwrap_or_else(|| name_os.clone());

        let module_path = format!("/data/adb/modules/{name_os}");
        let is_loaded = loaded_module_paths.contains(&module_path);

        modules.push(WebUiModule {
            name: display_name,
            path: module_path,
            has_system,
            has_vendor,
            has_product,
            is_loaded,
            file_count,
        });
    }

    modules
}

fn count_partition_files(module_dir: &Path) -> usize {
    let mut count = 0;
    for partition in &["system", "vendor", "product"] {
        let dir = module_dir.join(partition);
        if dir.is_dir() {
            count += count_files_recursive(&dir);
        }
    }
    count
}

fn count_files_recursive(dir: &Path) -> usize {
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files_recursive(&path);
            }
        }
    }
    count
}
