use std::fs;
use std::path::Path;

use anyhow::Result;
use tracing::{debug, warn};

pub fn sysfs_write(path: &str, value: &str) -> Result<bool> {
    if !Path::new(path).exists() {
        debug!(path, "sysfs path not found, skipping");
        return Ok(false);
    }

    match fs::write(path, value) {
        Ok(()) => {
            debug!(path, value, "sysfs write ok");
            Ok(true)
        }
        Err(e) => {
            warn!(path, %e, "sysfs write failed");
            Ok(false)
        }
    }
}

pub fn sysfs_read(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
}

pub fn sysfs_read_u64(path: &str) -> Option<u64> {
    sysfs_read(path)?.parse().ok()
}

pub fn procfs_write(path: &str, value: &str) -> Result<bool> {
    if !Path::new(path).exists() {
        debug!(path, "procfs path not found, skipping");
        return Ok(false);
    }

    match fs::write(path, value) {
        Ok(()) => {
            debug!(path, value, "procfs write ok");
            Ok(true)
        }
        Err(e) => {
            warn!(path, %e, "procfs write failed");
            Ok(false)
        }
    }
}

pub fn glob_dirs(pattern: &str) -> Vec<String> {
    let Some((parent, prefix)) = pattern.rsplit_once('/') else {
        return Vec::new();
    };

    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(prefix.trim_end_matches('*'))
            && entry.path().is_dir()
        {
            result.push(entry.path().to_string_lossy().into_owned());
        }
    }
    result.sort();
    result
}

#[allow(dead_code)]
pub fn glob_files(pattern: &str) -> Vec<String> {
    let Some((parent, prefix)) = pattern.rsplit_once('/') else {
        return Vec::new();
    };

    let Ok(entries) = fs::read_dir(parent) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(prefix.trim_end_matches('*')) {
            result.push(entry.path().to_string_lossy().into_owned());
        }
    }
    result.sort();
    result
}
