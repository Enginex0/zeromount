use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::core::types::{ModuleFileType, ScannedModule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeFileType {
    RegularFile,
    Directory,
    Symlink,
    Whiteout,
}

#[derive(Debug)]
pub struct Node {
    pub name: String,
    pub file_type: NodeFileType,
    pub children: HashMap<String, Node>,
    pub module_path: Option<PathBuf>,
    pub module_name: Option<String>,
    pub replace: bool,
    pub skip: bool,
}

impl Node {
    pub fn new_dir(name: String) -> Self {
        Self {
            name,
            file_type: NodeFileType::Directory,
            children: HashMap::new(),
            module_path: None,
            module_name: None,
            replace: false,
            skip: false,
        }
    }

    pub fn new_leaf(
        name: String,
        file_type: NodeFileType,
        module_path: PathBuf,
        module_name: String,
    ) -> Self {
        Self {
            name,
            file_type,
            children: HashMap::new(),
            module_path: Some(module_path),
            module_name: Some(module_name),
            replace: false,
            skip: false,
        }
    }

    /// Walk path components, creating intermediate Directory nodes as needed.
    /// Returns mutable ref to the deepest parent directory.
    fn ensure_parents(&mut self, components: &[&str]) -> &mut Node {
        let mut current = self;
        for &comp in components {
            current = current
                .children
                .entry(comp.to_string())
                .or_insert_with(|| Node::new_dir(comp.to_string()));
        }
        current
    }
}

fn map_file_type(mft: &ModuleFileType) -> NodeFileType {
    match mft {
        ModuleFileType::Regular => NodeFileType::RegularFile,
        ModuleFileType::Directory => NodeFileType::Directory,
        ModuleFileType::Symlink => NodeFileType::Symlink,
        ModuleFileType::WhiteoutCharDev
        | ModuleFileType::WhiteoutXattr
        | ModuleFileType::WhiteoutAufs => NodeFileType::Whiteout,
        ModuleFileType::OpaqueDir => NodeFileType::Directory,
        ModuleFileType::RedirectXattr => NodeFileType::RegularFile,
    }
}

pub fn build_node_tree(modules: &[ScannedModule]) -> Node {
    let mut root = Node::new_dir(String::new());

    for module in modules {
        for file in &module.files {
            let rel_str = file.relative_path.to_string_lossy();
            let components: Vec<&str> = rel_str.split('/').filter(|s| !s.is_empty()).collect();
            if components.is_empty() {
                continue;
            }

            let (leaf_name, parent_components) = components.split_last().unwrap();
            let parent = root.ensure_parents(parent_components);

            let node_type = map_file_type(&file.file_type);
            let source_path = module.path.join(&file.relative_path);
            let is_opaque = file.file_type == ModuleFileType::OpaqueDir;

            if let Some(existing) = parent.children.get_mut(*leaf_name) {
                if existing.file_type == NodeFileType::Directory
                    && node_type == NodeFileType::Directory
                {
                    // Directories merge across modules
                    if is_opaque && !existing.replace {
                        existing.replace = true;
                        debug!(
                            path = %rel_str,
                            module = %module.id,
                            "opaque dir flag set on merged directory"
                        );
                    }
                } else {
                    // First-module-wins for leaf nodes
                    debug!(
                        path = %rel_str,
                        existing_module = ?existing.module_name,
                        skipped_module = %module.id,
                        "leaf conflict: first-module-wins"
                    );
                }
            } else {
                let node = if node_type == NodeFileType::Directory {
                    let mut n = Node::new_dir(leaf_name.to_string());
                    n.module_path = Some(source_path);
                    n.module_name = Some(module.id.clone());
                    if is_opaque {
                        n.replace = true;
                    }
                    n
                } else {
                    Node::new_leaf(
                        leaf_name.to_string(),
                        node_type,
                        source_path,
                        module.id.clone(),
                    )
                };

                // Whiteouts with replace semantics don't apply here;
                // replace is only for OpaqueDir
                parent.children.insert(leaf_name.to_string(), node);
            }
        }
    }

    promote_partitions(&mut root);
    root
}

const PROMOTABLE_PARTITIONS: &[&str] = &["vendor", "system_ext", "product", "odm"];

fn promote_partitions(root: &mut Node) {
    let system = match root.children.get_mut("system") {
        Some(s) => s as *mut Node,
        None => return,
    };

    for &partition in PROMOTABLE_PARTITIONS {
        let real_path = Path::new("/system").join(partition);
        if !real_path.is_symlink() {
            continue;
        }

        // Safety: we're borrowing system mutably and root mutably, but system is a child of root.
        // We remove from system.children first, then insert into root.children.
        let system_ref = unsafe { &mut *system };
        if let Some(promoted) = system_ref.children.remove(partition) {
            debug!(
                partition,
                "promoting /system/{} to /{} (real path is symlink)", partition, partition
            );

            if let Some(existing) = root.children.get_mut(partition) {
                // Merge promoted children into existing top-level partition node
                for (name, child) in promoted.children {
                    existing.children.entry(name).or_insert(child);
                }
                if promoted.replace && !existing.replace {
                    existing.replace = true;
                }
            } else {
                root.children.insert(partition.to_string(), promoted);
            }
        }
    }
}

pub fn needs_tmpfs(node: &Node, real_path: &Path) -> bool {
    if node.replace {
        debug!(
            path = %real_path.display(),
            "needs_tmpfs: replace flag set"
        );
        return true;
    }

    for child in node.children.values() {
        if child.skip {
            continue;
        }

        let child_real = real_path.join(&child.name);

        if child.file_type == NodeFileType::Symlink {
            debug!(
                path = %child_real.display(),
                "needs_tmpfs: child is symlink"
            );
            return true;
        }

        if child.file_type == NodeFileType::Whiteout && child_real.exists() {
            debug!(
                path = %child_real.display(),
                "needs_tmpfs: whiteout targets existing path"
            );
            return true;
        }

        // New entry being added — no mount point exists on stock.
        // Applies to files AND directories: a new directory can't be
        // created on a read-only partition, so the parent needs tmpfs
        // to provide a writable surface for MS_MOVE targets.
        if child.module_path.is_some() && !child_real.exists() {
            debug!(
                path = %child_real.display(),
                child_type = ?child.file_type,
                "needs_tmpfs: new entry (not on stock)"
            );
            return true;
        }

        // Type mismatch between module entry and real filesystem
        if child_real.exists() {
            let real_is_dir = child_real.is_dir();
            let child_is_dir = child.file_type == NodeFileType::Directory;
            if real_is_dir != child_is_dir {
                debug!(
                    path = %child_real.display(),
                    real_is_dir,
                    child_type = ?child.file_type,
                    "needs_tmpfs: type mismatch"
                );
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{ModuleFile, ModuleProp, ScannedModule};

    fn make_module(id: &str, files: Vec<(&str, ModuleFileType)>) -> ScannedModule {
        ScannedModule {
            id: id.to_string(),
            path: PathBuf::from(format!("/data/adb/modules/{}", id)),
            files: files
                .into_iter()
                .map(|(path, ft)| ModuleFile {
                    relative_path: PathBuf::from(path),
                    file_type: ft,
                    source_module: id.to_string(),
                })
                .collect(),
            has_service_sh: false,
            has_post_fs_data_sh: false,
            prop: ModuleProp {
                id: id.to_string(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn single_module_builds_tree() {
        let modules = vec![make_module(
            "mod_a",
            vec![
                ("system/app/Foo/Foo.apk", ModuleFileType::Regular),
                ("system/app/Foo", ModuleFileType::Directory),
                ("system/app", ModuleFileType::Directory),
            ],
        )];

        let root = build_node_tree(&modules);
        assert!(root.children.contains_key("system"));
        let system = &root.children["system"];
        assert!(system.children.contains_key("app"));
        let app = &system.children["app"];
        assert!(app.children.contains_key("Foo"));
        let foo = &app.children["Foo"];
        assert!(foo.children.contains_key("Foo.apk"));
        assert_eq!(
            foo.children["Foo.apk"].file_type,
            NodeFileType::RegularFile
        );
    }

    #[test]
    fn first_module_wins_leaf_conflict() {
        let modules = vec![
            make_module(
                "mod_a",
                vec![("system/etc/hosts", ModuleFileType::Regular)],
            ),
            make_module(
                "mod_b",
                vec![("system/etc/hosts", ModuleFileType::Regular)],
            ),
        ];

        let root = build_node_tree(&modules);
        let hosts = &root.children["system"].children["etc"].children["hosts"];
        assert_eq!(hosts.module_name.as_deref(), Some("mod_a"));
    }

    #[test]
    fn directories_merge_across_modules() {
        let modules = vec![
            make_module(
                "mod_a",
                vec![("system/app/Foo/Foo.apk", ModuleFileType::Regular)],
            ),
            make_module(
                "mod_b",
                vec![("system/app/Bar/Bar.apk", ModuleFileType::Regular)],
            ),
        ];

        let root = build_node_tree(&modules);
        let app = &root.children["system"].children["app"];
        assert!(app.children.contains_key("Foo"));
        assert!(app.children.contains_key("Bar"));
    }

    #[test]
    fn opaque_dir_sets_replace() {
        let modules = vec![make_module(
            "mod_a",
            vec![("system/fonts", ModuleFileType::OpaqueDir)],
        )];

        let root = build_node_tree(&modules);
        let fonts = &root.children["system"].children["fonts"];
        assert!(fonts.replace);
        assert_eq!(fonts.file_type, NodeFileType::Directory);
    }

    #[test]
    fn whiteout_types_map_correctly() {
        let modules = vec![make_module(
            "mod_a",
            vec![
                ("system/app/Bloat", ModuleFileType::WhiteoutCharDev),
                ("system/app/Junk", ModuleFileType::WhiteoutXattr),
                ("system/app/Spam", ModuleFileType::WhiteoutAufs),
            ],
        )];

        let root = build_node_tree(&modules);
        let app = &root.children["system"].children["app"];
        assert_eq!(app.children["Bloat"].file_type, NodeFileType::Whiteout);
        assert_eq!(app.children["Junk"].file_type, NodeFileType::Whiteout);
        assert_eq!(app.children["Spam"].file_type, NodeFileType::Whiteout);
    }

    #[test]
    fn ensure_parents_creates_intermediates() {
        let mut root = Node::new_dir(String::new());
        let parent = root.ensure_parents(&["system", "app", "Foo"]);
        assert_eq!(parent.name, "Foo");
        assert_eq!(parent.file_type, NodeFileType::Directory);

        // Verify the full chain exists
        assert!(root.children.contains_key("system"));
        assert!(root.children["system"].children.contains_key("app"));
        assert!(root.children["system"].children["app"]
            .children
            .contains_key("Foo"));
    }

    #[test]
    fn needs_tmpfs_replace_flag() {
        let mut node = Node::new_dir("fonts".to_string());
        node.replace = true;
        assert!(needs_tmpfs(&node, Path::new("/system/fonts")));
    }

    #[test]
    fn needs_tmpfs_no_children_no_replace() {
        let node = Node::new_dir("app".to_string());
        assert!(!needs_tmpfs(&node, Path::new("/system/app")));
    }
}
