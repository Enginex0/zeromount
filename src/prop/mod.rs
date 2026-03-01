mod enforcer;
mod table;

use std::fs;

use anyhow::Result;
use tracing::info;

use crate::core::config::ZeroMountConfig;

const EXTERNAL_SUSFS_FLAG: &str = "/data/adb/zeromount/flags/external_susfs";

pub fn run_prop_watch() -> Result<()> {
    let config = ZeroMountConfig::load(None)?;
    let prop_spoof = config.brene.prop_spoofing;

    if !prop_spoof {
        info!("prop-watch: prop spoofing disabled, exiting");
        return Ok(());
    }

    let ext = has_external_susfs();

    if !ext {
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
    } else {
        info!("prop spoofing deferred to external module");
    }

    Ok(())
}

fn has_external_susfs() -> bool {
    fs::read_to_string(EXTERNAL_SUSFS_FLAG)
        .map(|s| {
            let v = s.trim();
            !v.is_empty() && v != "none"
        })
        .unwrap_or(false)
}
