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

/// Partition-equivalent subpaths that must never be overlaid at root level.
/// Overlaying /system/vendor (→ /vendor) masks GPU EGL drivers and other
/// critical partition content. Both mountify and meta-hybrid_mount enforce
/// per-subdirectory mounting for these paths.
const SENSITIVE_SUBPATHS: &[&str] = &["vendor", "product", "system_ext", "odm"];

/// BFS from partition root to find minimum mount points.
/// ME05: Never mount at the partition root itself.
/// Sensitive subpaths (vendor, product, etc.) always descend to per-subdir mounts.
///
/// Mount paths are canonicalized to resolve SAR symlinks (e.g., /system/vendor → /vendor).
/// Staging-relative paths are tracked separately so the executor can locate staged files.
fn bfs_mount_points(partition: &str, root: &DirNode) -> Vec<PartitionMount> {
    let mut mounts = Vec::new();

    // Queue: (canonical_mount_path, staging_rel, node, force_descend)
    let mut queue: VecDeque<(PathBuf, PathBuf, &DirNode, bool)> = VecDeque::new();

    for (child_name, child_node) in &root.children {
        let raw_path = resolve_partition_path(partition, child_name);
        let mount_path = canonicalize_or_raw(&raw_path);
        let staging_rel = PathBuf::from(child_name);
        let sensitive = SENSITIVE_SUBPATHS.contains(&child_name.as_str());
        queue.push_back((mount_path, staging_rel, child_node, sensitive));
    }

    while let Some((path, staging_rel, node, force_descend)) = queue.pop_front() {
        let total_contributors = count_all_contributors(node);

        let mount_here = !force_descend && (node.has_files || should_mount_here(node));

        if mount_here {
            let contributors: Vec<String> = collect_all_modules(node)
                .into_iter()
                .collect();

            debug!(
                mount_point = %path.display(),
                staging_rel = %staging_rel.display(),
                modules = contributors.len(),
                "planned mount point"
            );

            mounts.push(PartitionMount {
                partition: partition.to_string(),
                mount_point: path,
                staging_rel,
                contributing_modules: contributors,
            });
        } else if !total_contributors.is_empty() {
            if force_descend {
                debug!(mount_point = %path.display(), "sensitive partition path — descending to subdirs");
            }
            for (child_name, child_node) in &node.children {
                queue.push_back((
                    path.join(child_name),
                    staging_rel.join(child_name),
                    child_node,
                    false,
                ));
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

/// Build the raw filesystem path for a partition + subpath.
/// The result is later canonicalized to resolve SAR symlinks.
fn resolve_partition_path(partition: &str, subpath: &str) -> PathBuf {
    match partition {
        "system" => Path::new("/system").join(subpath),
        _ => PathBuf::from(format!("/{}", partition)).join(subpath),
    }
}

/// Canonicalize a path, resolving all symlinks. Falls back to the raw path
/// if the target doesn't exist (e.g., during cross-compilation or if the
/// device path isn't reachable).
fn canonicalize_or_raw(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
