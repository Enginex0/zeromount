use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::types::{RootManager, RootMountMode};

const ZEROMOUNT_MODULE_DIR: &str = "/data/adb/modules/meta-zeromount";

// -- KernelSU --

struct KsuManager;

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
            PathBuf::from("/data/adb/modules/meta-zeromount/ksu_susfs"),
        ]
    }

    fn update_description(&self, text: &str) -> Result<()> {
        write_description_to_module_prop(text)
    }

    fn mount_mode(&self) -> RootMountMode {
        if Path::new("/data/adb/ksu/.nomount").exists() {
            return RootMountMode::BindMount;
        }
        RootMountMode::Metamodule
    }
}

// -- APatch --

struct APatchManager;

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
            PathBuf::from("/data/adb/modules/meta-zeromount/ksu_susfs"),
        ]
    }

    fn update_description(&self, text: &str) -> Result<()> {
        write_description_to_module_prop(text)
    }

    fn mount_mode(&self) -> RootMountMode {
        if Path::new("/data/adb/.litemode_enable").exists() {
            return RootMountMode::BindMount;
        }
        RootMountMode::Metamodule
    }
}

// -- Magisk --

struct MagiskManager;

impl RootManager for MagiskManager {
    fn name(&self) -> &str {
        "Magisk"
    }

    fn base_dir(&self) -> &Path {
        Path::new("/data/adb/magisk/")
    }

    fn busybox_path(&self) -> PathBuf {
        PathBuf::from("/data/adb/magisk/busybox")
    }

    fn susfs_binary_paths(&self) -> Vec<PathBuf> {
        vec![]
    }

    fn update_description(&self, text: &str) -> Result<()> {
        write_description_to_module_prop(text)
    }

    fn mount_mode(&self) -> RootMountMode {
        RootMountMode::BindMount
    }
}

// -- Shared --

pub(crate) fn write_description_to_module_prop(text: &str) -> Result<()> {
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

    let tmp_path = prop_path.with_extension("prop.tmp");
    std::fs::write(&tmp_path, &updated)
        .context("failed to write module.prop.tmp for description update")?;

    if let Err(e) = std::fs::rename(&tmp_path, &prop_path) {
        tracing::warn!("atomic rename failed, trying direct write: {e}");
        std::fs::write(&prop_path, &updated)
            .context("failed to write module.prop for description update")?;
    }
    Ok(())
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

    // Magisk: check $MAGISK or filesystem
    if std::env::var("MAGISK").ok().as_deref() == Some("true") {
        return Ok(Box::new(MagiskManager));
    }

    if Path::new("/data/adb/magisk/").exists() {
        return Ok(Box::new(MagiskManager));
    }

    anyhow::bail!("no supported root manager detected (neither KernelSU, APatch, nor Magisk found)")
}
