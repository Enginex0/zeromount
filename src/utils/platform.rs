use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::core::types::RootManager;

const ZEROMOUNT_MODULE_DIR: &str = "/data/adb/modules/zeromount";

// -- KernelSU --

pub struct KsuManager;

impl RootManager for KsuManager {
    fn name(&self) -> &str {
        "KernelSU"
    }

    fn base_dir(&self) -> &Path {
        Path::new("/data/adb/ksu/")
    }

    fn busybox_path(&self) -> PathBuf {
        PathBuf::from("/data/adb/ksu/bin/busybox")
    }

    fn susfs_binary_paths(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/data/adb/ksu/bin/ksu_susfs"),
            PathBuf::from("/data/adb/modules/zeromount/ksu_susfs"),
        ]
    }

    fn update_description(&self, text: &str) -> Result<()> {
        // KSU05: KernelSU has override.description via ksud module config
        let status = Command::new("ksud")
            .args(["module", "config", "set", "override.description", text])
            .status()
            .context("failed to exec ksud for override.description")?;
        if !status.success() {
            anyhow::bail!("ksud module config set override.description failed (exit {})",
                status.code().unwrap_or(-1));
        }
        Ok(())
    }

    fn notify_module_mounted(&self) -> Result<()> {
        let status = Command::new("ksud")
            .args(["kernel", "notify-module-mounted"])
            .status()
            .context("failed to exec ksud kernel notify-module-mounted")?;
        if !status.success() {
            anyhow::bail!("notify-module-mounted failed (exit {})",
                status.code().unwrap_or(-1));
        }
        Ok(())
    }
}

// -- APatch --

pub struct APatchManager;

impl RootManager for APatchManager {
    fn name(&self) -> &str {
        "APatch"
    }

    fn base_dir(&self) -> &Path {
        Path::new("/data/adb/ap/")
    }

    fn busybox_path(&self) -> PathBuf {
        PathBuf::from("/data/adb/ap/bin/busybox")
    }

    fn susfs_binary_paths(&self) -> Vec<PathBuf> {
        vec![
            PathBuf::from("/data/adb/ap/bin/ksu_susfs"),
            PathBuf::from("/data/adb/modules/zeromount/ksu_susfs"),
        ]
    }

    fn update_description(&self, text: &str) -> Result<()> {
        // KSU05: APatch has no override.description; edit module.prop directly
        let prop_path = Path::new(ZEROMOUNT_MODULE_DIR).join("module.prop");
        let content = std::fs::read_to_string(&prop_path)
            .context("failed to read module.prop for description update")?;

        let mut updated = String::with_capacity(content.len());
        for line in content.lines() {
            if line.starts_with("description=") {
                updated.push_str("description=");
                updated.push_str(text);
            } else {
                updated.push_str(line);
            }
            updated.push('\n');
        }

        std::fs::write(&prop_path, &updated)
            .context("failed to write module.prop for description update")?;
        Ok(())
    }

    fn notify_module_mounted(&self) -> Result<()> {
        // Same command on APatch -- ksud is available on both platforms
        let status = Command::new("ksud")
            .args(["kernel", "notify-module-mounted"])
            .status()
            .context("failed to exec ksud kernel notify-module-mounted")?;
        if !status.success() {
            anyhow::bail!("notify-module-mounted failed (exit {})",
                status.code().unwrap_or(-1));
        }
        Ok(())
    }
}

// -- Detection --

/// Detect the active root manager at runtime.
/// Checks env vars first (KSU02), then filesystem fallback.
pub fn detect_root_manager() -> Result<Box<dyn RootManager>> {
    // KSU sets $KSU=true
    if std::env::var("KSU").ok().as_deref() == Some("true") {
        return Ok(Box::new(KsuManager));
    }

    // APatch sets $APATCH=true
    if std::env::var("APATCH").ok().as_deref() == Some("true") {
        return Ok(Box::new(APatchManager));
    }

    // Filesystem fallback
    if Path::new("/data/adb/ksu/").exists() {
        return Ok(Box::new(KsuManager));
    }

    if Path::new("/data/adb/ap/").exists() {
        return Ok(Box::new(APatchManager));
    }

    anyhow::bail!("no supported root manager detected (neither KernelSU nor APatch found)")
}
