use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{info, warn};

use crate::core::types::{
    CapabilityFlags, MountPlan, MountStrategy, PartitionMount,
    PlannedModule, ScannedModule, Scenario,
};
use crate::modules::scanner::SUPPORTED_PARTITIONS;

const SAR_ALIAS_PARTITIONS: &[&str] = &["vendor", "product", "system_ext", "odm"];

pub fn plan_mounts(
    modules: &[ScannedModule],
    scenario: Scenario,
    capabilities: &CapabilityFlags,
    user_override: Option<MountStrategy>,
) -> Result<MountPlan> {
    if modules.is_empty() {
        return Ok(MountPlan {
            scenario,
            modules: Vec::new(),
            partition_mounts: Vec::new(),
        });
    }

    let strategy = select_strategy(scenario, capabilities, user_override);

    if strategy == MountStrategy::Vfs {
        let planned: Vec<PlannedModule> = modules
            .iter()
            .map(|m| build_planned_module(m))
            .collect();

        info!(
            strategy = "vfs",
            modules = planned.len(),
            "VFS mode: mount planning skipped, per-file rules used instead"
        );

        return Ok(MountPlan {
            scenario,
            modules: planned,
            partition_mounts: Vec::new(),
        });
    }

    let partition_mounts = build_partition_root_mounts(modules);

    let planned: Vec<PlannedModule> = modules
        .iter()
        .map(|m| build_planned_module(m))
        .collect();

    info!(
        strategy = ?strategy,
        modules = planned.len(),
        mount_points = partition_mounts.len(),
        "mount plan generated"
    );

    Ok(MountPlan {
        scenario,
        modules: planned,
        partition_mounts,
    })
}

/// One overlay per resolved partition. Aligns overlay boundaries with partition
/// boundaries so stat-based detectors see consistent mount IDs within each partition.
fn build_partition_root_mounts(modules: &[ScannedModule]) -> Vec<PartitionMount> {
    let mut partition_contributors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for module in modules {
        for file in &module.files {
            let (resolved, _) = resolve_file_partition(&file.relative_path);
            if !resolved.is_empty() {
                partition_contributors
                    .entry(resolved)
                    .or_default()
                    .insert(module.id.clone());
            }
        }
    }

    partition_contributors
        .into_iter()
        .map(|(partition, contributors)| {
            let raw = PathBuf::from(format!("/{partition}"));
            let mount_point = canonicalize_or_raw(&raw);

            PartitionMount {
                partition,
                mount_point,
                staging_rel: PathBuf::new(),
                contributing_modules: contributors.into_iter().collect(),
            }
        })
        .collect()
}

/// Resolve a module file's relative path to (resolved_partition, sub_path).
///
/// On SAR devices, system/vendor/* maps to the /vendor partition:
///   "system/vendor/etc/foo.conf" -> ("vendor", "etc/foo.conf")
///   "system/etc/audio.conf"      -> ("system", "etc/audio.conf")
///   "vendor/etc/foo.conf"        -> ("vendor", "etc/foo.conf")
pub fn resolve_file_partition(relative_path: &Path) -> (String, PathBuf) {
    let components: Vec<&str> = relative_path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.is_empty() {
        return (String::new(), PathBuf::new());
    }

    let top = components[0];
    if !SUPPORTED_PARTITIONS.contains(&top) {
        return (String::new(), PathBuf::new());
    }

    // system/vendor/... -> redirect to vendor partition if /vendor exists on device
    if top == "system" && components.len() >= 2 {
        let maybe_alias = components[1];
        if SAR_ALIAS_PARTITIONS.contains(&maybe_alias) {
            let canonical = Path::new("/").join(maybe_alias);
            if canonical.is_dir() {
                let sub: PathBuf = components[2..].iter().collect();
                return (maybe_alias.to_string(), sub);
            }
        }
    }

    let sub: PathBuf = components[1..].iter().collect();
    (top.to_string(), sub)
}

fn select_strategy(
    scenario: Scenario,
    capabilities: &CapabilityFlags,
    user_override: Option<MountStrategy>,
) -> MountStrategy {
    match scenario {
        Scenario::Full | Scenario::SusfsFrontend | Scenario::KernelOnly => {
            match user_override {
                Some(s @ MountStrategy::Overlay) | Some(s @ MountStrategy::MagicMount) => s,
                Some(other) => {
                    warn!(strategy = ?other, "mount strategy fell through to Vfs — check planner conditions");
                    MountStrategy::Vfs
                }
                _ => MountStrategy::Vfs,
            }
        }
        Scenario::SusfsOnly | Scenario::None => {
            match user_override {
                Some(MountStrategy::MagicMount) => MountStrategy::MagicMount,
                _ if capabilities.overlay_supported => MountStrategy::Overlay,
                _ => MountStrategy::MagicMount,
            }
        }
    }
}

fn build_planned_module(module: &ScannedModule) -> PlannedModule {
    let mut partitions = BTreeSet::new();
    for file in &module.files {
        if let Some(first) = file.relative_path.components().next() {
            let part = first.as_os_str().to_string_lossy().to_string();
            if SUPPORTED_PARTITIONS.contains(&part.as_str()) {
                partitions.insert(part);
            }
        }
    }

    PlannedModule {
        id: module.id.clone(),
        source_path: module.path.clone(),
        target_partitions: partitions.into_iter().collect(),
        file_count: module.files.len(),
    }
}

fn canonicalize_or_raw(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
