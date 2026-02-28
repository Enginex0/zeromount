mod enforcer;
mod table;

use std::fs;
use std::io::{BufRead, BufReader};
use std::process::Command;
use std::thread;

use anyhow::Result;
use tracing::{debug, info};

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

    let ext = has_external_susfs();

    if prop_spoof && !ext {
        let props: Vec<(&str, &str)> = table::GENERAL
            .iter()
            .map(|p| (p.name, p.value))
            .collect();
        enforcer::enforce_once(&props);

        let size = config.brene.vbmeta_size.to_string();
        enforcer::resetprop("ro.boot.vbmeta.size", &size);

        if !config.brene.verified_boot_hash.is_empty() {
            enforcer::resetprop("ro.boot.vbmeta.digest", &config.brene.verified_boot_hash);
        }

        info!("general prop spoofing applied");
    } else if prop_spoof {
        info!("prop spoofing deferred to external module");
    }

    if !hide_usb {
        return Ok(());
    }

    enforcer::enforce_once(
        &table::HIDE_DEBUG.iter().map(|p| (p.name, p.value)).collect::<Vec<_>>(),
    );

    let dynamic = scan_build_props();
    debug!(count = dynamic, "build.prop overrides applied");

    for entry in table::DYNAMIC_USB {
        enforcer::watch_prop(entry.name, entry.value);
    }
    info!("USB stealth active ({} watchers)", table::DYNAMIC_USB.len());

    loop {
        thread::park();
    }
}

fn has_external_susfs() -> bool {
    fs::read_to_string(EXTERNAL_SUSFS_FLAG)
        .map(|s| {
            let v = s.trim();
            !v.is_empty() && v != "none"
        })
        .unwrap_or(false)
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
                overrides.push(line.replace("userdebug", "user"));
            } else if line.contains("test-keys") {
                overrides.push(line.replace("test-keys", "release-keys"));
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
