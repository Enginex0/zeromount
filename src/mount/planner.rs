use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, info};

use crate::core::types::{
    CapabilityFlags, ModuleFileType, MountPlan, MountStrategy, PartitionMount,
    PlannedModule, ScannedModule, Scenario,
};
use crate::modules::scanner::SUPPORTED_PARTITIONS;

/// Produce a MountPlan from scanned modules using BFS to find minimum mount points.
///
/// Hard constraint (ME05): never mount at partition roots (/system, /vendor, etc.).
/// Always mount one level deeper (/system/bin, /vendor/lib64, etc.).
pub fn plan_mounts(
    modules: &[ScannedModule],
    scenario: Scenario,
    capabilities: &CapabilityFlags,
) -> Result<MountPlan> {
    if modules.is_empty() {
        return Ok(MountPlan {
            scenario,
            modules: Vec::new(),
            partition_mounts: Vec::new(),
        });
    }

    let strategy = select_strategy(scenario, capabilities);

    // For VFS mode, no mount planning needed -- the VFS executor handles per-file rules
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

    // Merge all module files into per-partition trees
    let partition_trees = build_partition_trees(modules);

    // BFS to find minimum mount points per partition
    let mut partition_mounts = Vec::new();
    for (partition, tree) in &partition_trees {
        let mounts = bfs_mount_points(partition, tree);
        partition_mounts.extend(mounts);
    }

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

fn select_strategy(scenario: Scenario, capabilities: &CapabilityFlags) -> MountStrategy {
    match scenario {
        Scenario::Full | Scenario::SusfsFrontend | Scenario::KernelOnly => MountStrategy::Vfs,
        Scenario::SusfsOnly | Scenario::None => {
            if capabilities.overlay_supported {
                MountStrategy::Overlay
            } else {
                MountStrategy::MagicMount
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

/// A tree node representing a directory in the merged module filesystem.
#[derive(Debug, Default)]
struct DirNode {
    /// Modules that contribute files directly in this directory (not subdirs)
    contributing_modules: BTreeSet<String>,
    /// Child directory names -> DirNode
    children: BTreeMap<String, DirNode>,
    /// Whether any file (not dir) exists at this level
    has_files: bool,
}

/// Build per-partition directory trees from all modules' files.
fn build_partition_trees(modules: &[ScannedModule]) -> BTreeMap<String, DirNode> {
    let mut trees: BTreeMap<String, DirNode> = BTreeMap::new();

    for module in modules {
        for file in &module.files {
            let components: Vec<&str> = file
                .relative_path
                .components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect();

            if components.is_empty() {
                continue;
            }

            let partition = components[0].to_string();
            if !SUPPORTED_PARTITIONS.contains(&partition.as_str()) {
                continue;
            }

            let root = trees.entry(partition).or_default();
            insert_into_tree(root, &components[1..], &module.id, &file.file_type);
        }
    }

    trees
}

fn insert_into_tree(
    node: &mut DirNode,
    path_components: &[&str],
    module_id: &str,
    file_type: &ModuleFileType,
) {
    if path_components.is_empty() {
        return;
    }

    if path_components.len() == 1 {
        // Leaf entry -- this is a file or terminal directory in the module
        match file_type {
            ModuleFileType::Directory | ModuleFileType::OpaqueDir => {
                let child = node
                    .children
                    .entry(path_components[0].to_string())
                    .or_default();
                child.contributing_modules.insert(module_id.to_string());
            }
            _ => {
                // File-level entry: mark this directory as having files
                node.has_files = true;
                node.contributing_modules.insert(module_id.to_string());
            }
        }
        return;
    }

    // Intermediate directory
    let child = node
        .children
        .entry(path_components[0].to_string())
        .or_default();
    insert_into_tree(child, &path_components[1..], module_id, file_type);
}

/// BFS from partition root to find minimum mount points.
/// ME05: Never mount at the partition root itself.
fn bfs_mount_points(partition: &str, root: &DirNode) -> Vec<PartitionMount> {
    let mut mounts = Vec::new();

    // Start BFS from the partition root's children (never mount at root)
    let mut queue: VecDeque<(PathBuf, &DirNode)> = VecDeque::new();

    for (child_name, child_node) in &root.children {
        let mount_path = resolve_partition_path(partition, child_name);
        queue.push_back((mount_path, child_node));
    }

    while let Some((path, node)) = queue.pop_front() {
        let total_contributors = count_all_contributors(node);

        if node.has_files || should_mount_here(node) {
            // Mount at this level
            let contributors: Vec<String> = collect_all_modules(node)
                .into_iter()
                .collect();

            debug!(
                mount_point = %path.display(),
                modules = contributors.len(),
                "planned mount point"
            );

            mounts.push(PartitionMount {
                partition: partition.to_string(),
                mount_point: path,
                contributing_modules: contributors,
            });
        } else if !total_contributors.is_empty() {
            // No files at this level, but children have content -- descend
            for (child_name, child_node) in &node.children {
                queue.push_back((path.join(child_name), child_node));
            }
        }
    }

    mounts
}

/// Decide if we should place a mount at this node rather than descending further.
/// Mount here if: files exist at this level, or all children have content (cheaper
/// to mount once than N times).
fn should_mount_here(node: &DirNode) -> bool {
    if node.has_files {
        return true;
    }

    // If >75% of children have content, mount here instead of per-child
    if node.children.len() >= 3 {
        let populated = node
            .children
            .values()
            .filter(|c| c.has_files || !c.children.is_empty())
            .count();
        let ratio = populated as f64 / node.children.len() as f64;
        if ratio > 0.75 {
            return true;
        }
    }

    false
}

/// Resolve the actual filesystem path for a partition + subpath.
/// Handles SAR: /product may be a symlink to /system/product (legacy)
/// or a real mount point (modern).
fn resolve_partition_path(partition: &str, subpath: &str) -> PathBuf {
    let direct = PathBuf::from(format!("/{}", partition));

    match partition {
        "system" => Path::new("/system").join(subpath),
        "vendor" | "product" | "system_ext" | "odm" => {
            // SAR detection: check if /<partition> is a symlink to /system/<partition>
            if is_sar_symlink(partition) {
                Path::new("/system").join(partition).join(subpath)
            } else {
                direct.join(subpath)
            }
        }
        _ => direct.join(subpath),
    }
}

/// Check if a partition path is a SAR symlink (e.g., /product -> /system/product).
fn is_sar_symlink(partition: &str) -> bool {
    let path = PathBuf::from(format!("/{}", partition));
    match std::fs::read_link(&path) {
        Ok(target) => {
            let expected = PathBuf::from(format!("/system/{}", partition));
            let alt = PathBuf::from(format!("system/{}", partition));
            target == expected || target == alt
        }
        Err(_) => false,
    }
}

/// Collect all unique module IDs from a node and all its descendants.
fn collect_all_modules(node: &DirNode) -> BTreeSet<String> {
    let mut modules = node.contributing_modules.clone();
    for child in node.children.values() {
        modules.extend(collect_all_modules(child));
    }
    modules
}

/// Count all unique contributors in a node subtree.
fn count_all_contributors(node: &DirNode) -> BTreeSet<String> {
    collect_all_modules(node)
}
