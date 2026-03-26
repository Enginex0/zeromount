use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::core::config::ZeroMountConfig;

const MODULE_DIR: &str = "/data/adb/modules/meta-zeromount";
const ACTIVITY_LOG: &str = "/data/adb/zeromount/activity.log";
const RECOVERY_LOCKOUT: &str = "/data/adb/zeromount/.recovery_lockout";

pub fn execute(_config: &ZeroMountConfig) -> ! {
    let timestamp = timestamp_iso8601();

    let _ = fs::File::create(Path::new(MODULE_DIR).join("disable"));

    let msg = format!("[{timestamp}] guard_recovery: zeromount self-disabled");

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(ACTIVITY_LOG) {
        let _ = writeln!(f, "{msg}");
    }
    tracing::error!("{msg}");

    let _ = crate::utils::platform::write_description_to_module_prop(
        "\u{26a0}\u{fe0f} Guard recovery — disabled due to boot failure. Re-enable manually.",
    );

    let _ = fs::remove_file("/data/adb/zeromount/.bootcount");
    let _ = fs::write(RECOVERY_LOCKOUT, timestamp.as_bytes());

    let _ = Command::new("/system/bin/svc").args(["power", "reboot"]).status();
    std::process::exit(1);
}

pub fn is_locked_out() -> bool {
    Path::new(RECOVERY_LOCKOUT).exists()
}

pub fn clear_lockout() {
    let _ = fs::remove_file(RECOVERY_LOCKOUT);
}

fn timestamp_iso8601() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;
    let (y, mo, d) = days_to_ymd(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
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
