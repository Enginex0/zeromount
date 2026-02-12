use std::ffi::CString;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::{fs, io};

use anyhow::{bail, Context, Result};
use tracing::{debug, info, warn};

use crate::core::types::{MountResult, MountStrategy, ScannedModule};
use crate::utils::selinux::copy_selinux_context;

use super::node::{build_node_tree, needs_tmpfs, Node, NodeFileType};

// ---------------------------------------------------------------------------
// Mount helpers
// ---------------------------------------------------------------------------

fn path_to_cstring(path: &Path) -> Result<CString> {
    CString::new(path.as_os_str().as_encoded_bytes())
        .with_context(|| format!("path contains null byte: {}", path.display()))
}

fn bind_mount(source: &Path, target: &Path) -> Result<()> {
    let c_src = path_to_cstring(source)?;
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe {
        libc::mount(
            c_src.as_ptr(),
            c_tgt.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        bail!(
            "bind mount {} -> {} failed: {}",
            source.display(),
            target.display(),
            io::Error::last_os_error()
        );
    }
    debug!(src = %source.display(), tgt = %target.display(), "bind mount");
    Ok(())
}

fn bind_mount_recursive(source: &Path, target: &Path) -> Result<()> {
    let c_src = path_to_cstring(source)?;
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe {
        libc::mount(
            c_src.as_ptr(),
            c_tgt.as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_REC,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        bail!(
            "recursive bind mount {} -> {} failed: {}",
            source.display(),
            target.display(),
            io::Error::last_os_error()
        );
    }
    debug!(src = %source.display(), tgt = %target.display(), "recursive bind mount");
    Ok(())
}

fn mount_tmpfs(target: &Path, source_label: &str) -> Result<()> {
    let c_src = CString::new(source_label)?;
    let c_tgt = path_to_cstring(target)?;
    let c_fs = CString::new("tmpfs")?;
    let c_data = CString::new("mode=0755")?;
    let ret = unsafe {
        libc::mount(
            c_src.as_ptr(),
            c_tgt.as_ptr(),
            c_fs.as_ptr(),
            0,
            c_data.as_ptr() as *const libc::c_void,
        )
    };
    if ret != 0 {
        bail!(
            "tmpfs mount at {} failed: {}",
            target.display(),
            io::Error::last_os_error()
        );
    }
    debug!(target = %target.display(), label = source_label, "tmpfs mounted");
    Ok(())
}

fn mount_move(source: &Path, target: &Path) -> Result<()> {
    let c_src = path_to_cstring(source)?;
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe {
        libc::mount(
            c_src.as_ptr(),
            c_tgt.as_ptr(),
            std::ptr::null(),
            libc::MS_MOVE,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        bail!(
            "MS_MOVE {} -> {} failed: {}",
            source.display(),
            target.display(),
            io::Error::last_os_error()
        );
    }
    debug!(src = %source.display(), tgt = %target.display(), "mount moved");
    Ok(())
}

fn remount_readonly(target: &Path) -> Result<()> {
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe {
        libc::mount(
            std::ptr::null(),
            c_tgt.as_ptr(),
            std::ptr::null(),
            libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        bail!(
            "remount readonly {} failed: {}",
            target.display(),
            io::Error::last_os_error()
        );
    }
    Ok(())
}

fn mount_private(target: &Path) -> Result<()> {
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe {
        libc::mount(
            std::ptr::null(),
            c_tgt.as_ptr(),
            std::ptr::null(),
            libc::MS_REC | libc::MS_PRIVATE,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        bail!(
            "MS_PRIVATE {} failed: {}",
            target.display(),
            io::Error::last_os_error()
        );
    }
    Ok(())
}

fn lazy_unmount(target: &Path) -> Result<()> {
    let c_tgt = path_to_cstring(target)?;
    let ret = unsafe { libc::umount2(c_tgt.as_ptr(), libc::MNT_DETACH) };
    if ret != 0 {
        bail!(
            "lazy unmount {} failed: {}",
            target.display(),
            io::Error::last_os_error()
        );
    }
    debug!(target = %target.display(), "lazy unmount");
    Ok(())
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

struct MountStats {
    applied: u32,
    failed: u32,
    whiteouts: u32,
    errors: Vec<String>,
    mount_paths: Vec<String>,
}

impl MountStats {
    fn new() -> Self {
        Self {
            applied: 0,
            failed: 0,
            whiteouts: 0,
            errors: Vec::new(),
            mount_paths: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn mount_magic(
    modules: &[ScannedModule],
    staging_dir: &Path,
) -> Result<Vec<MountResult>> {
    info!(module_count = modules.len(), "magic mount starting");

    let mut root = build_node_tree(modules);

    let workdir = staging_dir.join("workdir");
    fs::create_dir_all(&workdir)
        .with_context(|| format!("create workdir: {}", workdir.display()))?;
    mount_tmpfs(&workdir, "ZeroMount")?;
    mount_private(&workdir)?;

    let mut stats = MountStats::new();

    // Collect child names first to avoid borrow conflict
    let child_names: Vec<String> = root.children.keys().cloned().collect();
    for name in child_names {
        let real_path = Path::new("/").join(&name);
        if let Some(child) = root.children.get_mut(&name) {
            if let Err(e) = apply_node_recursive(child, &real_path, &workdir, false, &mut stats) {
                let msg = format!("top-level /{}: {e}", name);
                warn!("{}", msg);
                stats.errors.push(msg);
                stats.failed += 1;
            }
        }
    }

    if let Err(e) = lazy_unmount(&workdir) {
        warn!(error = %e, "workdir lazy unmount failed");
    }
    let _ = fs::remove_dir(&workdir);

    info!(
        applied = stats.applied,
        failed = stats.failed,
        whiteouts = stats.whiteouts,
        "magic mount complete"
    );

    Ok(build_results(modules, &stats))
}

// ---------------------------------------------------------------------------
// Result aggregation
// ---------------------------------------------------------------------------

fn build_results(modules: &[ScannedModule], stats: &MountStats) -> Vec<MountResult> {
    // Group mount paths by module (best-effort: paths don't carry module info,
    // so we emit a single aggregate result per module that participated).
    modules
        .iter()
        .map(|m| {
            let paths: Vec<String> = stats
                .mount_paths
                .iter()
                .filter(|p| {
                    // A path belongs to this module if it was sourced from it.
                    // Since the tree merges modules, attribute all paths to every module
                    // that contributed files; the tree's first-module-wins means each
                    // path was actually produced by one module.
                    m.files
                        .iter()
                        .any(|f| p.ends_with(f.relative_path.to_string_lossy().as_ref()))
                })
                .cloned()
                .collect();
            let applied = paths.len() as u32;
            let module_errors: Vec<String> = stats
                .errors
                .iter()
                .filter(|e| e.contains(&m.id))
                .cloned()
                .collect();
            let failed = module_errors.len() as u32;

            MountResult {
                module_id: m.id.clone(),
                strategy_used: MountStrategy::MagicMount,
                success: failed == 0 && (applied > 0 || stats.applied > 0),
                rules_applied: applied,
                rules_failed: failed,
                error: if module_errors.is_empty() {
                    None
                } else {
                    Some(module_errors.join("; "))
                },
                mount_paths: paths,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Recursive traversal
// ---------------------------------------------------------------------------

fn apply_node_recursive(
    node: &mut Node,
    real_path: &Path,
    workdir: &Path,
    inside_tmpfs: bool,
    stats: &mut MountStats,
) -> Result<()> {
    match node.file_type {
        NodeFileType::Whiteout => {
            debug!(path = %real_path.display(), "whiteout (handled by parent tmpfs)");
            stats.whiteouts += 1;
        }

        NodeFileType::Symlink => {
            if !inside_tmpfs {
                warn!(
                    path = %real_path.display(),
                    "symlink outside tmpfs context, skipping"
                );
                return Ok(());
            }
            let module_path = match &node.module_path {
                Some(p) => p.clone(),
                None => bail!("symlink node without module_path: {}", real_path.display()),
            };
            let link_target = fs::read_link(&module_path)
                .with_context(|| format!("read_link: {}", module_path.display()))?;
            let wdest = workdir_dest(workdir, real_path);
            if let Some(parent) = wdest.parent() {
                fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&link_target, &wdest)
                .with_context(|| format!("symlink {} -> {}", wdest.display(), link_target.display()))?;
            copy_selinux_context(&module_path, &wdest);
            debug!(
                path = %real_path.display(),
                target = %link_target.display(),
                "symlink created in workdir"
            );
            stats.applied += 1;
        }

        NodeFileType::RegularFile => {
            let module_path = match &node.module_path {
                Some(p) => p.clone(),
                None => bail!("file node without module_path: {}", real_path.display()),
            };

            if inside_tmpfs {
                let wdest = workdir_dest(workdir, real_path);
                if let Some(parent) = wdest.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::File::create(&wdest)
                    .with_context(|| format!("touch: {}", wdest.display()))?;
                bind_mount(&module_path, &wdest)?;
                remount_readonly(&wdest)?;
                debug!(
                    src = %module_path.display(),
                    dest = %wdest.display(),
                    "file bind-mounted inside tmpfs"
                );
            } else {
                // Ensure mount point exists
                if let Some(parent) = real_path.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)?;
                    }
                }
                if !real_path.exists() {
                    fs::File::create(real_path)
                        .with_context(|| format!("create mount point: {}", real_path.display()))?;
                }
                bind_mount(&module_path, real_path)?;
                remount_readonly(real_path)?;
                stats.mount_paths.push(real_path.to_string_lossy().to_string());
                debug!(
                    src = %module_path.display(),
                    dest = %real_path.display(),
                    "file bind-mounted directly"
                );
            }
            stats.applied += 1;
        }

        NodeFileType::Directory => {
            if node.skip {
                debug!(path = %real_path.display(), "directory skipped");
                return Ok(());
            }

            let need_tmpfs = needs_tmpfs(node, real_path);

            if need_tmpfs && !inside_tmpfs && node.module_path.is_none() && !real_path.exists() {
                // Graceful degradation: tmpfs needed but directory doesn't exist
                // on stock and no module provides it. Skip children that triggered
                // tmpfs to avoid certain failure.
                warn!(
                    path = %real_path.display(),
                    "needs tmpfs but dir not on stock and no module_path, skipping children"
                );
                for child in node.children.values_mut() {
                    child.skip = true;
                }
                stats.failed += 1;
                return Ok(());
            }

            if need_tmpfs {
                debug!(path = %real_path.display(), "directory needs tmpfs");
                apply_tmpfs_directory(node, real_path, workdir, stats)?;
            } else {
                debug!(path = %real_path.display(), inside_tmpfs, "directory passthrough");
                let child_names: Vec<String> = node.children.keys().cloned().collect();
                for name in child_names {
                    let child_real = real_path.join(&name);
                    if let Some(child) = node.children.get_mut(&name) {
                        let result = apply_node_recursive(
                            child,
                            &child_real,
                            workdir,
                            inside_tmpfs,
                            stats,
                        );
                        if inside_tmpfs {
                            result?;
                        } else if let Err(e) = result {
                            let msg = format!("{}: {e}", child_real.display());
                            warn!("{}", msg);
                            stats.errors.push(msg);
                            stats.failed += 1;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tmpfs directory: self-bind, mirror, recurse, ro, MS_MOVE
// ---------------------------------------------------------------------------

fn apply_tmpfs_directory(
    node: &mut Node,
    real_path: &Path,
    workdir: &Path,
    stats: &mut MountStats,
) -> Result<()> {
    let wpath = workdir.join(real_path.strip_prefix("/").unwrap_or(real_path));

    // Create dir in workdir preserving permissions from real_path or module_path
    fs::create_dir_all(&wpath)
        .with_context(|| format!("mkdir workdir: {}", wpath.display()))?;

    let reference = if real_path.exists() {
        real_path.to_path_buf()
    } else {
        node.module_path.clone().unwrap_or_else(|| real_path.to_path_buf())
    };

    if let Ok(meta) = fs::metadata(&reference) {
        let c_wpath = path_to_cstring(&wpath)?;
        unsafe {
            libc::chmod(c_wpath.as_ptr(), meta.permissions().mode() as libc::mode_t);
            libc::chown(c_wpath.as_ptr(), meta.uid(), meta.gid());
        }
    }
    copy_selinux_context(&reference, &wpath);

    // Self-bind to create independent mount point
    bind_mount(&wpath, &wpath)?;

    // Mirror stock entries (unless this is a full replacement)
    if !node.replace {
        if let Err(e) = mirror_stock_entries(real_path, &wpath, node) {
            warn!(
                path = %real_path.display(),
                error = %e,
                "mirror_stock_entries failed, aborting tmpfs directory"
            );
            let _ = lazy_unmount(&wpath);
            bail!("mirror failed for {}: {e}", real_path.display());
        }
    }

    // Recurse children inside tmpfs
    let child_names: Vec<String> = node.children.keys().cloned().collect();
    for name in &child_names {
        let child_real = real_path.join(name);
        if let Some(child) = node.children.get_mut(name) {
            if let Err(e) = apply_node_recursive(child, &child_real, workdir, true, stats) {
                warn!(
                    path = %child_real.display(),
                    error = %e,
                    "child failed inside tmpfs, aborting directory"
                );
                let _ = lazy_unmount(&wpath);
                bail!("child {} failed inside tmpfs: {e}", child_real.display());
            }
        }
    }

    // Seal and move
    remount_readonly(&wpath)?;

    if let Err(e) = mount_move(&wpath, real_path) {
        warn!(
            wpath = %wpath.display(),
            real_path = %real_path.display(),
            error = %e,
            "MS_MOVE failed, cleaning up orphaned tmpfs"
        );
        let _ = lazy_unmount(&wpath);
        bail!("MS_MOVE failed for {}: {e}", real_path.display());
    }

    mount_private(real_path)?;
    stats.mount_paths.push(real_path.to_string_lossy().to_string());
    stats.applied += 1;

    debug!(path = %real_path.display(), "tmpfs directory mounted and moved");
    Ok(())
}

// ---------------------------------------------------------------------------
// Mirror stock filesystem entries
// ---------------------------------------------------------------------------

fn mirror_stock_entries(real_path: &Path, wpath: &Path, node: &Node) -> Result<()> {
    let entries = match fs::read_dir(real_path) {
        Ok(e) => e,
        Err(e) => {
            debug!(
                path = %real_path.display(),
                error = %e,
                "cannot read dir for mirror (may not exist on stock)"
            );
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip entries the module tree handles
        if node.children.contains_key(name_str.as_ref()) {
            continue;
        }

        // Skip entries hidden by a whiteout
        let hidden = node.children.values().any(|c| {
            c.file_type == NodeFileType::Whiteout && c.name == name_str.as_ref()
        });
        if hidden {
            debug!(name = %name_str, "skipping whiteout-hidden entry in mirror");
            continue;
        }

        let src = real_path.join(&name);
        let dst = wpath.join(&name);
        let meta = fs::symlink_metadata(&src)
            .with_context(|| format!("symlink_metadata: {}", src.display()))?;

        if meta.is_symlink() {
            let link_target = fs::read_link(&src)?;
            std::os::unix::fs::symlink(&link_target, &dst)
                .with_context(|| format!("mirror symlink: {}", dst.display()))?;
            copy_selinux_context(&src, &dst);
            debug!(name = %name_str, "mirrored symlink");
        } else if meta.is_dir() {
            fs::create_dir_all(&dst)?;
            let c_dst = path_to_cstring(&dst)?;
            unsafe {
                libc::chmod(c_dst.as_ptr(), meta.permissions().mode() as libc::mode_t);
                libc::chown(c_dst.as_ptr(), meta.uid(), meta.gid());
            }
            bind_mount_recursive(&src, &dst)?;
            copy_selinux_context(&src, &dst);
            debug!(name = %name_str, "mirrored directory via recursive bind");
        } else {
            fs::File::create(&dst)
                .with_context(|| format!("touch mirror: {}", dst.display()))?;
            bind_mount(&src, &dst)?;
            copy_selinux_context(&src, &dst);
            debug!(name = %name_str, "mirrored file via bind mount");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn workdir_dest(workdir: &Path, real_path: &Path) -> PathBuf {
    workdir.join(real_path.strip_prefix("/").unwrap_or(real_path))
}
