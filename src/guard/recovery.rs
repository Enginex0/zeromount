use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;

use crate::core::config::ZeroMountConfig;

const MODULES_DIR: &str = "/data/adb/modules";
const SERVICE_D: &str = "/data/adb/service.d";
const POST_FS_DATA_D: &str = "/data/adb/post-fs-data.d";
const ACTIVITY_LOG: &str = "/data/adb/zeromount/activity.log";

pub fn execute(config: &ZeroMountConfig) -> ! {
    let mut whitelist: HashSet<&str> = HashSet::new();
    whitelist.insert("meta-zeromount");
    for m in &config.guard.allowed_modules {
        whitelist.insert(m.as_str());
    }

    let mut disabled = Vec::new();

    if let Ok(entries) = fs::read_dir(MODULES_DIR) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if whitelist.contains(name_str.as_ref()) {
                continue;
            }
            let disable_path = entry.path().join("disable");
            if fs::File::create(&disable_path).is_ok() {
                disabled.push(name_str.to_string());
            }
        }
    }

    let script_whitelist: HashSet<&str> = config.guard.allowed_scripts.iter().map(|s| s.as_str()).collect();

    for dir in [SERVICE_D, POST_FS_DATA_D] {
        neuter_scripts(dir, &script_whitelist);
    }

    let timestamp = timestamp_iso8601();
    let msg = format!(
        "[{timestamp}] guard_recovery: disabled {} modules [{}]",
        disabled.len(),
        disabled.join(", ")
    );

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(ACTIVITY_LOG) {
        let _ = writeln!(f, "{msg}");
    }
    tracing::error!("{msg}");

    let desc = format!(
        "⚠\u{fe0f} Guard recovery — {} modules disabled. Re-enable in module manager.",
        disabled.len()
    );
    let _ = crate::utils::platform::write_description_to_module_prop(&desc);

    super::markers::clear_all().ok();

    let _ = Command::new("/system/bin/svc").args(["power", "reboot"]).status();
    std::process::exit(1);
}

fn neuter_scripts(dir: &str, whitelist: &HashSet<&str>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(".status.sh") || whitelist.contains(name_str.as_ref()) {
            continue;
        }
        // Remove execute permission
        let _ = Command::new("chmod").args(["644", &entry.path().to_string_lossy()]).status();
    }
}

fn timestamp_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // UTC timestamp without chrono dependency
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    // Days since 1970-01-01
    days += 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
