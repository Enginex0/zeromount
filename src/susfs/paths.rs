use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use tracing::debug;

use super::SusfsClient;

/// Hide a list of paths, skipping any that don't exist.
/// Returns the count of successfully hidden paths.
pub fn hide_paths(client: &SusfsClient, paths: &[&str]) -> Result<u32> {
    if !client.is_available() || !client.features().path {
        bail!("SUSFS path hiding not available");
    }

    let mut count = 0u32;
    for path in paths {
        if !Path::new(path).exists() {
            debug!("skip nonexistent path: {path}");
            continue;
        }
        match client.add_sus_path(path) {
            Ok(()) => count += 1,
            Err(e) => debug!("add_sus_path failed for {path}: {e}"),
        }
    }
    Ok(count)
}

/// Hide a list of paths with re-flag per zygote spawn.
pub fn hide_paths_loop(client: &SusfsClient, paths: &[&str]) -> Result<u32> {
    if !client.is_available() || !client.features().path {
        bail!("SUSFS path hiding not available");
    }

    let mut count = 0u32;
    for path in paths {
        if !Path::new(path).exists() {
            debug!("skip nonexistent path: {path}");
            continue;
        }
        match client.add_sus_path_loop(path) {
            Ok(()) => count += 1,
            Err(e) => debug!("add_sus_path_loop failed for {path}: {e}"),
        }
    }
    Ok(count)
}

// Hide children of each directory via add_sus_path_loop so they get
// re-flagged on every non-root process spawn (matches BRENE behavior).
pub fn hide_dir_children_loop(client: &SusfsClient, dirs: &[&str]) -> Result<u32> {
    if !client.is_available() || !client.features().path {
        bail!("SUSFS path hiding not available");
    }

    let mut count = 0u32;
    for dir in dirs {
        let dir_path = Path::new(dir);
        if !dir_path.is_dir() {
            debug!("skip nonexistent dir: {dir}");
            continue;
        }
        let entries = match fs::read_dir(dir_path) {
            Ok(e) => e,
            Err(e) => {
                debug!("read_dir failed for {dir}: {e}");
                continue;
            }
        };
        for entry in entries.flatten() {
            let child = format!("{}/{}", dir, entry.file_name().to_string_lossy());
            match client.add_sus_path_loop(&child) {
                Ok(()) => count += 1,
                Err(e) => debug!("add_sus_path_loop failed for {child}: {e}"),
            }
        }
    }
    Ok(count)
}

/// Hide a list of library paths from /proc/self/maps.
pub fn hide_maps(client: &SusfsClient, map_paths: &[&str]) -> Result<u32> {
    if !client.is_available() || !client.features().maps {
        bail!("SUSFS maps hiding not available");
    }

    let mut count = 0u32;
    for entry in map_paths {
        if entry.starts_with('/') && !Path::new(entry).exists() {
            debug!("skipping nonexistent map path: {entry}");
            continue;
        }
        match client.add_sus_map(entry) {
            Ok(()) => count += 1,
            Err(e) => {
                let kind = if entry.starts_with('/') { "path" } else { "pattern" };
                debug!("add_sus_map failed for {kind} {entry}: {e}");
            }
        }
    }
    Ok(count)
}
