use std::collections::HashSet;
use std::ffi::CString;
use std::fs;
use std::path::Path;

use tracing::{debug, info, warn};

use crate::core::config::SusfsConfig;
use crate::core::types::{CapabilityFlags, MountResult, Scenario};
use crate::modules::scanner::SUPPORTED_PARTITIONS;

#[derive(Debug)]
#[allow(dead_code)]
pub struct MountInfoEntry {
    pub mount_point: String,
    pub root: String,
    pub fs_type: String,
    pub mount_source: String,
}

#[derive(Debug)]
pub enum HijackAction {
    VfsReplaced,
    VfsAndSusfs,
    SusfsReplaced,
    Skipped,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct HijackResult {
    pub mount_point: String,
    pub source: String,
    pub action: HijackAction,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct SweepSummary {
    pub found: u32,
    pub hijacked: u32,
    pub skipped: u32,
    pub results: Vec<HijackResult>,
}

pub fn sweep(
    scenario: Scenario,
    capabilities: &CapabilityFlags,
    susfs_config: &SusfsConfig,
    mount_results: &[MountResult],
) -> SweepSummary {
    let entries = match parse_mountinfo() {
        Ok(e) => e,
        Err(e) => {
            warn!("sweep: failed to parse mountinfo: {e}");
            return SweepSummary { found: 0, hijacked: 0, skipped: 0, results: Vec::new() };
        }
    };

    let managed_paths: HashSet<String> = mount_results
        .iter()
        .filter(|r| r.success)
        .flat_map(|r| r.mount_paths.iter().cloned())
        .collect();

    let rogues = find_rogue_mounts(&entries, &managed_paths);
    let found = rogues.len() as u32;

    if found == 0 {
        debug!("sweep: no rogue bind mounts found");
        return SweepSummary { found: 0, hijacked: 0, skipped: 0, results: Vec::new() };
    }

    info!("sweep: found {} rogue bind mount(s)", found);
    for rogue in &rogues {
        debug!(
            mount_point = %rogue.mount_point,
            root = %rogue.root,
            fs_type = %rogue.fs_type,
            "rogue bind mount detected"
        );
    }

    if matches!(scenario, Scenario::None) {
        debug!("sweep: Scenario::None — no replacement mechanism, skipping all");
        let results: Vec<HijackResult> = rogues
            .iter()
            .map(|e| HijackResult {
                mount_point: e.mount_point.clone(),
                source: resolve_source_path(e),
                action: HijackAction::Skipped,
                success: false,
                error: Some("no replacement mechanism available".into()),
            })
            .collect();
        return SweepSummary { found, hijacked: 0, skipped: found, results };
    }

    let has_vfs = capabilities.vfs_driver;
    let has_susfs = susfs_config.enabled && capabilities.susfs_available;

    let driver = if has_vfs {
        match crate::vfs::VfsDriver::open() {
            Ok(d) => Some(d),
            Err(e) => {
                warn!("sweep: VFS driver open failed: {e}");
                None
            }
        }
    } else {
        None
    };

    let susfs = if has_susfs && capabilities.susfs_open_redirect {
        match crate::susfs::SusfsClient::probe() {
            Ok(c) if c.is_available() => Some(c),
            _ => None,
        }
    } else {
        None
    };

    if driver.is_none() && susfs.is_none() {
        debug!("sweep: neither VFS nor SUSFS available for replacement");
        let results: Vec<HijackResult> = rogues
            .iter()
            .map(|e| HijackResult {
                mount_point: e.mount_point.clone(),
                source: resolve_source_path(e),
                action: HijackAction::Skipped,
                success: false,
                error: Some("replacement drivers unavailable".into()),
            })
            .collect();
        return SweepSummary { found, hijacked: 0, skipped: found, results };
    }

    let mut results = Vec::with_capacity(rogues.len());
    let mut hijacked = 0u32;
    let mut skipped = 0u32;

    for entry in &rogues {
        let result = hijack_mount(entry, driver.as_ref(), susfs.as_ref());
        if result.success {
            hijacked += 1;
        } else {
            skipped += 1;
        }
        results.push(result);
    }

    // Flush dcache after adding VFS rules
    if let Some(ref d) = driver {
        if hijacked > 0 {
            if let Err(e) = d.refresh() {
                warn!("sweep: VFS refresh failed: {e}");
            }
        }
    }

    SweepSummary { found, hijacked, skipped, results }
}

fn parse_mountinfo() -> anyhow::Result<Vec<MountInfoEntry>> {
    let content = fs::read_to_string("/proc/self/mountinfo")
        .map_err(|e| anyhow::anyhow!("read /proc/self/mountinfo: {e}"))?;

    let mut entries = Vec::new();

    for line in content.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let root = fields[3];
        let mount_point = fields[4];

        let sep_idx = fields.iter().position(|&f| f == "-");
        let Some(sep) = sep_idx else { continue };
        if sep + 3 > fields.len() {
            continue;
        }

        let fs_type = fields[sep + 1];
        let mount_source = fields[sep + 2];

        entries.push(MountInfoEntry {
            mount_point: mount_point.to_string(),
            root: root.to_string(),
            fs_type: fs_type.to_string(),
            mount_source: mount_source.to_string(),
        });
    }

    Ok(entries)
}

fn find_rogue_mounts<'a>(
    entries: &'a [MountInfoEntry],
    managed_paths: &HashSet<String>,
) -> Vec<&'a MountInfoEntry> {
    entries
        .iter()
        .filter(|e| {
            e.root.starts_with("/adb/modules/")
                && e.fs_type != "overlay"
                && is_system_partition(&e.mount_point)
                && !managed_paths.contains(&e.mount_point)
        })
        .collect()
}

fn is_system_partition(mount_point: &str) -> bool {
    SUPPORTED_PARTITIONS
        .iter()
        .any(|p| mount_point.starts_with(&format!("/{p}/")))
}

fn resolve_source_path(entry: &MountInfoEntry) -> String {
    format!("/data{}", entry.root)
}

fn hijack_mount(
    entry: &MountInfoEntry,
    driver: Option<&crate::vfs::VfsDriver>,
    susfs: Option<&crate::susfs::SusfsClient>,
) -> HijackResult {
    let source = resolve_source_path(entry);
    let target = &entry.mount_point;
    let is_dir = Path::new(&source).is_dir();

    debug!(
        target,
        source = %source,
        fs_type = %entry.fs_type,
        is_dir,
        "hijacking rogue bind mount"
    );

    // BRENE handles font paths via kstat + path_hide on top of the existing bind mounts.
    // Unmounting them would break font rendering — BRENE adds stealth, not file serving.
    let brene_owned = crate::vfs::executor::is_brene_owned_target(Path::new(target));
    if brene_owned {
        debug!(
            target,
            source = %source,
            "BRENE-owned path — preserving bind mount, BRENE adds stealth in finalize"
        );
        return HijackResult {
            mount_point: target.clone(),
            source,
            action: HijackAction::Skipped,
            success: false,
            error: None,
        };
    }

    // Phase 1: Add replacement mechanism BEFORE unmounting
    let action = if let Some(d) = driver {
        if is_dir {
            let (added, _failed) = walk_and_add_vfs_rules(target, &source, d);
            if added == 0 {
                return HijackResult {
                    mount_point: target.clone(),
                    source,
                    action: HijackAction::Skipped,
                    success: false,
                    error: Some("VFS rule injection failed for directory".into()),
                };
            }
        } else if let Err(e) = d.add_rule(Path::new(&source), Path::new(target), false) {
            warn!("sweep: VFS add_rule failed for {target}: {e}");
            return HijackResult {
                mount_point: target.clone(),
                source,
                action: HijackAction::Skipped,
                success: false,
                error: Some(format!("VFS add_rule: {e}")),
            };
        }

        if let Some(s) = susfs {
            if s.features().open_redirect_all {
                if let Err(e) = s.add_open_redirect_all(target, &source) {
                    debug!("sweep: SUSFS redirect for {target} failed (non-fatal): {e}");
                }
                HijackAction::VfsAndSusfs
            } else {
                HijackAction::VfsReplaced
            }
        } else {
            HijackAction::VfsReplaced
        }
    } else if let Some(s) = susfs {
        if let Err(e) = s.add_open_redirect_all(target, &source) {
            return HijackResult {
                mount_point: target.clone(),
                source,
                action: HijackAction::Skipped,
                success: false,
                error: Some(format!("SUSFS redirect: {e}")),
            };
        }
        HijackAction::SusfsReplaced
    } else {
        return HijackResult {
            mount_point: target.clone(),
            source,
            action: HijackAction::Skipped,
            success: false,
            error: Some("no driver available".into()),
        };
    };

    // Phase 2: Unmount the rogue bind mount
    let unmounted = lazy_umount(target);
    if !unmounted {
        warn!("sweep: umount failed for {target}, replacement still active");
    }

    debug!(
        target,
        source = %source,
        action = ?action,
        unmounted,
        "rogue mount hijacked"
    );

    HijackResult {
        mount_point: target.clone(),
        source,
        action,
        success: true,
        error: None,
    }
}

fn walk_and_add_vfs_rules(
    mount_point: &str,
    source_base: &str,
    driver: &crate::vfs::VfsDriver,
) -> (u32, u32) {
    let mut added = 0u32;
    let mut failed = 0u32;

    let source_path = Path::new(source_base);
    let walker = match fs::read_dir(source_path) {
        Ok(w) => w,
        Err(e) => {
            warn!("sweep: cannot walk {source_base}: {e}");
            return (0, 1);
        }
    };

    // Add rule for the directory itself
    if let Err(e) = driver.add_rule(Path::new(source_base), Path::new(mount_point), true) {
        debug!("sweep: dir rule failed for {mount_point}: {e}");
        failed += 1;
    } else {
        added += 1;
    }

    // Walk children
    for entry in walker.flatten() {
        let child_name = match entry.file_name().into_string() {
            Ok(n) => n,
            Err(_) => continue,
        };

        let child_source = format!("{source_base}/{child_name}");
        let child_target = format!("{mount_point}/{child_name}");
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if is_dir {
            let (a, f) = walk_and_add_vfs_rules(&child_target, &child_source, driver);
            added += a;
            failed += f;
        } else {
            match driver.add_rule(Path::new(&child_source), Path::new(&child_target), false) {
                Ok(()) => added += 1,
                Err(e) => {
                    debug!("sweep: rule failed for {child_target}: {e}");
                    failed += 1;
                }
            }
        }
    }

    (added, failed)
}

fn lazy_umount(path: &str) -> bool {
    let c_path = match CString::new(path.as_bytes()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let ret = unsafe { libc::umount2(c_path.as_ptr(), libc::MNT_DETACH) };
    ret == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rogue_font_entry() {
        let line = "500000 36 253:47 /adb/modules/Facebook15.0/system/fonts/NotoColorEmoji.ttf /system/fonts/NotoColorEmoji.ttf rw,nosuid,nodev,noatime shared:58 - f2fs /dev/block/dm-47 rw,foo";
        let fields: Vec<&str> = line.split_whitespace().collect();

        let root = fields[3];
        let mount_point = fields[4];

        let sep = fields.iter().position(|&f| f == "-").unwrap();
        let fs_type = fields[sep + 1];

        assert_eq!(root, "/adb/modules/Facebook15.0/system/fonts/NotoColorEmoji.ttf");
        assert_eq!(mount_point, "/system/fonts/NotoColorEmoji.ttf");
        assert_eq!(fs_type, "f2fs");
        assert!(root.starts_with("/adb/modules/"));
        assert_ne!(fs_type, "overlay");
    }

    #[test]
    fn source_path_reconstruction() {
        let entry = MountInfoEntry {
            mount_point: "/system/fonts/NotoColorEmoji.ttf".into(),
            root: "/adb/modules/Facebook15.0/system/fonts/NotoColorEmoji.ttf".into(),
            fs_type: "f2fs".into(),
            mount_source: "/dev/block/dm-47".into(),
        };
        let source = resolve_source_path(&entry);
        assert_eq!(source, "/data/adb/modules/Facebook15.0/system/fonts/NotoColorEmoji.ttf");
    }

    #[test]
    fn is_system_partition_checks() {
        assert!(is_system_partition("/system/fonts/Roboto.ttf"));
        assert!(is_system_partition("/vendor/lib64/libfoo.so"));
        assert!(is_system_partition("/product/app/SomeApp.apk"));
        assert!(!is_system_partition("/data/adb/modules/foo"));
        assert!(!is_system_partition("/proc/self/mountinfo"));
        assert!(!is_system_partition("/sys/kernel/debug"));
    }

    #[test]
    fn rogue_detection_filters() {
        let entries = vec![
            MountInfoEntry {
                mount_point: "/system/fonts/Emoji.ttf".into(),
                root: "/adb/modules/FontMod/system/fonts/Emoji.ttf".into(),
                fs_type: "f2fs".into(),
                mount_source: "/dev/block/dm-47".into(),
            },
            MountInfoEntry {
                mount_point: "/system/app".into(),
                root: "/adb/modules/SomeApp/system/app".into(),
                fs_type: "overlay".into(),
                mount_source: "overlay".into(),
            },
            MountInfoEntry {
                mount_point: "/system/bin/sh".into(),
                root: "/".into(),
                fs_type: "ext4".into(),
                mount_source: "/dev/block/dm-0".into(),
            },
            MountInfoEntry {
                mount_point: "/system/lib64/libmanaged.so".into(),
                root: "/adb/modules/Managed/system/lib64/libmanaged.so".into(),
                fs_type: "f2fs".into(),
                mount_source: "/dev/block/dm-47".into(),
            },
        ];

        let mut managed = HashSet::new();
        managed.insert("/system/lib64/libmanaged.so".to_string());

        let rogues = find_rogue_mounts(&entries, &managed);
        assert_eq!(rogues.len(), 1);
        assert_eq!(rogues[0].mount_point, "/system/fonts/Emoji.ttf");
    }

    #[test]
    fn sweep_none_scenario_skips_all() {
        let summary = sweep(
            Scenario::None,
            &CapabilityFlags::default(),
            &SusfsConfig::default(),
            &[],
        );
        assert_eq!(summary.hijacked, 0);
    }
}
