use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, info, warn};

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

/// A tree node representing a directory in the merged module filesystem.
#[derive(Debug, Default)]
struct DirNode {
    contributing_modules: BTreeSet<String>,
    children: BTreeMap<String, DirNode>,
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

// Broad overlays on these dirs mask critical system files (shared libs, configs)
// or trigger stat-based detection. Shallow files defer to bind mounts; subdirs
// get overlaid at the deeper level.
const NARROW_DIRS: &[&str] = &["lib", "lib64", "etc", "fonts"];

/// BFS from partition root to find minimum mount points.
/// ME05: Never mount at the partition root itself.
/// Sensitive subpaths (vendor, product, etc.) always descend to per-subdir mounts.
///
/// Mount paths are canonicalized to resolve SAR symlinks (e.g., /system/vendor → /vendor).
/// Staging-relative paths are tracked separately so the executor can locate staged files.
fn bfs_mount_points(partition: &str, root: &DirNode) -> Vec<PartitionMount> {
    let mut mounts: Vec<PartitionMount> = Vec::new();

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
        let narrow = path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| NARROW_DIRS.contains(&n))
            .unwrap_or(false);

        let narrow_with_files = narrow && node.has_files;
        let mount_here = !force_descend
            && (narrow_with_files || (!narrow && (node.has_files || should_mount_here(node))));

        if mount_here {
            let (mp, sr) = elevate_novel_target(&path, &staging_rel);

            let contributors: Vec<String> = collect_all_modules(node)
                .into_iter()
                .collect();

            if let Some(existing) = mounts.iter_mut().find(|m| m.mount_point == mp) {
                for c in contributors {
                    if !existing.contributing_modules.contains(&c) {
                        existing.contributing_modules.push(c);
                    }
                }
            } else {
                debug!(
                    mount_point = %mp.display(),
                    staging_rel = %sr.display(),
                    modules = contributors.len(),
                    "planned mount point"
                );

                mounts.push(PartitionMount {
                    partition: partition.to_string(),
                    mount_point: mp,
                    staging_rel: sr,
                    contributing_modules: contributors,
                });
            }
        } else if has_any_contributors(node) {
            if force_descend {
                debug!(mount_point = %path.display(), "sensitive partition path — descending to subdirs");
            }
            if narrow {
                debug!(mount_point = %path.display(), "narrow dir — descending to subdirs");
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

fn elevate_novel_target(path: &Path, staging_rel: &Path) -> (PathBuf, PathBuf) {
    if path.exists() {
        return (path.to_path_buf(), staging_rel.to_path_buf());
    }
    let mut p = path.to_path_buf();
    let mut r = staging_rel.to_path_buf();
    while !p.exists() {
        match (p.parent(), r.parent()) {
            (Some(pp), Some(rr)) if !rr.as_os_str().is_empty() => {
                p = pp.to_path_buf();
                r = rr.to_path_buf();
            }
            _ => break,
        }
    }
    debug!(
        novel = %path.display(),
        elevated = %p.display(),
        "novel overlay target elevated to existing ancestor"
    );
    (p, r)
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

const SAR_ALIAS_PARTITIONS: &[&str] = &["vendor", "product", "system_ext", "odm"];

fn resolve_partition_path(partition: &str, subpath: &str) -> PathBuf {
    if partition == "system" {
        if SAR_ALIAS_PARTITIONS.contains(&subpath) {
            let canonical = Path::new("/").join(subpath);
            if canonical.is_dir() {
                return canonical;
            }
        }
        if let Some((alias, rest)) = subpath.split_once('/') {
            if SAR_ALIAS_PARTITIONS.contains(&alias) {
                let canonical = Path::new("/").join(alias);
                if canonical.is_dir() {
                    return canonical.join(rest);
                }
            }
        }
        return Path::new("/system").join(subpath);
    }
    PathBuf::from(format!("/{}", partition)).join(subpath)
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

fn has_any_contributors(node: &DirNode) -> bool {
    if !node.contributing_modules.is_empty() {
        return true;
    }
    node.children.values().any(has_any_contributors)
}
