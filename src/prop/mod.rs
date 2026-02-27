mod enforcer;
mod ffi;
mod table;

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;

use anyhow::Result;
use tracing::{debug, info, trace};

use crate::core::config::ZeroMountConfig;

const EXTERNAL_SUSFS_FLAG: &str = "/data/adb/zeromount/flags/external_susfs";

pub fn run_prop_watch() -> Result<()> {
    let config = ZeroMountConfig::load(None)?;
    let hide_usb = config.adb.hide_usb_debugging;
    let prop_spoof = config.brene.prop_spoofing;

    if !hide_usb && !prop_spoof {
        info!("prop-watch: both toggles disabled, exiting");
        return Ok(());
    }

    // Phase 1: one-time general prop spoofing
    if prop_spoof && !has_external_susfs() {
        let props: Vec<(&str, &str)> = table::GENERAL
            .iter()
            .map(|p| (p.name, p.value))
            .collect();
        enforcer::enforce_once(&props);

        let size = config.brene.vbmeta_size.to_string();
        trace!(prop = "ro.boot.vbmeta.size", value = %size, "setting vbmeta size");
        enforcer::resetprop("ro.boot.vbmeta.size", &size);

        if !config.brene.verified_boot_hash.is_empty() {
            trace!(prop = "ro.boot.vbmeta.digest", "setting verified boot hash");
            enforcer::resetprop("ro.boot.vbmeta.digest", &config.brene.verified_boot_hash);
        }

        for prop in &["ro.warranty_bit", "ro.vendor.boot.warranty_bit",
                       "ro.vendor.warranty_bit", "ro.boot.warranty_bit"] {
            enforcer::resetprop(prop, "0");
        }

        info!("general prop spoofing applied");
    } else if prop_spoof {
        info!("prop spoofing deferred to external module");
    }

    if !hide_usb {
        return Ok(());
    }

    // Phase 1b: static debug props (one-time)
    let static_props: Vec<(&str, &str)> = table::HIDE_DEBUG
        .iter()
        .map(|p| (p.name, p.value))
        .collect();
    enforcer::enforce_once(&static_props);

    let dynamic = scan_build_props();
    debug!(count = dynamic, "dynamic build.prop props overridden");

    // Wait for boot_completed before Settings.Global + watch loop
    wait_boot_completed();
    apply_settings_global();

    // Phase 2: event-driven property monitoring (never returns)
    let watch: Vec<(&'static str, &'static str)> = table::USB_WATCH
        .iter()
        .map(|p| (p.name, p.value))
        .collect();
    enforcer::watch_loop(&watch);
}

fn has_external_susfs() -> bool {
    fs::read_to_string(EXTERNAL_SUSFS_FLAG)
        .map(|s| {
            let v = s.trim();
            !v.is_empty() && v != "none"
        })
        .unwrap_or(false)
}

fn wait_boot_completed() {
    loop {
        if let Ok(output) = Command::new("getprop").arg("sys.boot_completed").output() {
            if String::from_utf8_lossy(&output.stdout).trim() == "1" {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    info!("boot completed, starting USB prop enforcement");
}

fn apply_settings_global() {
    for (ns, key, val) in &[
        ("global", "adb_enabled", "0"),
        ("global", "development_settings_enabled", "0"),
        ("global", "adb_wifi_enabled", "0"),
    ] {
        trace!(namespace = ns, key, value = val, "settings put");
        let _ = Command::new("settings")
            .args(["put", ns, key, val])
            .output();
    }
    debug!("Settings.Global ADB entries cleared");
}

fn scan_build_props() -> usize {
    let mut overrides = Vec::new();

    for path in table::BUILD_PROP_PATHS {
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for line in BufReader::new(file).lines().flatten() {
            if !line.starts_with("ro.") { continue; }

            if line.contains("userdebug") {
                let fixed = line.replace("userdebug", "user");
                trace!(original = %line, replacement = %fixed, "build.prop override");
                overrides.push(fixed);
            } else if line.contains("test-keys") {
                let fixed = line.replace("test-keys", "release-keys");
                trace!(original = %line, replacement = %fixed, "build.prop override");
                overrides.push(fixed);
            }
        }
    }

    if overrides.is_empty() {
        return 0;
    }

    let count = overrides.len();
    let tmp = "/data/adb/zeromount/.prop_override_tmp";
    if fs::write(tmp, overrides.join("\n")).is_ok() {
        let _ = Command::new("resetprop").args(["--file", tmp]).output();
        let _ = fs::remove_file(tmp);
    }
    count
}
