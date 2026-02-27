use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

const SYSFS_DEBUG: &str = "/sys/kernel/zeromount/debug";
const VERBOSE_MARKER: &str = "/data/adb/zeromount/.verbose";

/// Read the current kernel debug level from sysfs (0=off, 1=standard, 2=verbose).
pub fn read_kernel_debug_level() -> Result<u32> {
    let content = fs::read_to_string(SYSFS_DEBUG)
        .context("cannot read /sys/kernel/zeromount/debug -- is the kernel module loaded?")?;
    content
        .trim()
        .parse::<u32>()
        .context("unexpected value in sysfs debug node")
}

/// Write a debug level to the kernel sysfs node.
pub fn write_kernel_debug_level(level: u32) -> Result<()> {
    fs::write(SYSFS_DEBUG, format!("{level}\n"))
        .context("cannot write /sys/kernel/zeromount/debug -- check permissions / SELinux")
}

/// Touch or remove the .verbose marker file based on the requested state.
fn set_verbose_marker(enabled: bool) -> Result<()> {
    let path = Path::new(VERBOSE_MARKER);
    if enabled {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(path, "").context("cannot create .verbose marker")?;
    } else if path.exists() {
        fs::remove_file(path).context("cannot remove .verbose marker")?;
    }
    Ok(())
}

fn persist_verbose_config(enabled: bool) {
    if let Ok(mut config) = crate::core::config::ZeroMountConfig::load(None) {
        config.logging.verbose = enabled;
        let _ = config.save();
    }
}

fn try_write_sysfs(level: u32) -> &'static str {
    if Path::new(SYSFS_DEBUG).exists() {
        match write_kernel_debug_level(level) {
            Ok(()) => "ok",
            Err(e) => {
                eprintln!("warning: sysfs write failed: {e}");
                "error"
            }
        }
    } else {
        "n/a"
    }
}

pub fn enable() -> Result<()> {
    // Level 2 = ZM_DBG active (full verbose kernel logging)
    let sysfs = try_write_sysfs(2);
    set_verbose_marker(true)?;
    persist_verbose_config(true);
    if let Ok(client) = crate::susfs::SusfsClient::probe() {
        if let Err(e) = client.enable_log(true) {
            eprintln!("warning: SUSFS log toggle failed: {e}");
        }
    }
    println!("logging enabled (sysfs={sysfs}, .verbose=present, config=true)");
    Ok(())
}

pub fn disable() -> Result<()> {
    let sysfs = try_write_sysfs(0);
    set_verbose_marker(false)?;
    persist_verbose_config(false);
    if let Ok(client) = crate::susfs::SusfsClient::probe() {
        if let Err(e) = client.enable_log(false) {
            eprintln!("warning: SUSFS log toggle failed: {e}");
        }
    }
    println!("logging disabled (sysfs={sysfs}, .verbose=removed, config=false)");
    Ok(())
}

pub fn set_level(level: u32) -> Result<()> {
    // Kernel sysfs accepts 0/1/2 only; level 3 (VERBOSE) maps to sysfs=2
    let sysfs_level = level.min(2);
    let sysfs = try_write_sysfs(sysfs_level);
    // Only VERBOSE (level 3) persists across reboots and activates SUSFS logging
    let verbose = level >= 3;
    set_verbose_marker(verbose)?;
    persist_verbose_config(verbose);
    if let Ok(client) = crate::susfs::SusfsClient::probe() {
        if let Err(e) = client.enable_log(verbose) {
            eprintln!("warning: SUSFS log toggle failed: {e}");
        }
    }
    println!("debug level set to {level} (sysfs={sysfs})");
    Ok(())
}

pub fn status() -> Result<()> {
    let sysfs_available = Path::new(SYSFS_DEBUG).exists();
    let verbose_present = Path::new(VERBOSE_MARKER).exists();

    if sysfs_available {
        match read_kernel_debug_level() {
            Ok(level) => println!("kernel debug level: {level}"),
            Err(e) => println!("kernel debug level: error ({e})"),
        }
    } else {
        println!("kernel debug level: sysfs node not available");
    }

    println!(".verbose marker: {}", if verbose_present { "present" } else { "absent" });

    match crate::core::config::ZeroMountConfig::load(None) {
        Ok(config) => println!("config.toml logging.verbose: {}", config.logging.verbose),
        Err(_) => println!("config.toml logging.verbose: unknown (load failed)"),
    }
    Ok(())
}
