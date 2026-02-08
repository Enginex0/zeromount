use std::collections::HashMap;
use std::path::Path;

use tracing::warn;

use crate::core::types::{ModuleFileType, ScannedModule};

/// Single-pass conflict detection: log when multiple modules provide the same file.
/// Returns the number of conflicts found.
pub fn detect_conflicts(modules: &[ScannedModule]) -> u32 {
    let mut file_map: HashMap<&Path, Vec<&str>> = HashMap::new();

    for module in modules {
        for file in &module.files {
            match file.file_type {
                ModuleFileType::Regular | ModuleFileType::Symlink => {
                    file_map
                        .entry(file.relative_path.as_path())
                        .or_default()
                        .push(&module.id);
                }
                _ => {}
            }
        }
    }

    let mut conflict_count = 0u32;
    for (path, providers) in &file_map {
        if providers.len() > 1 {
            warn!(
                path = %path.display(),
                modules = %providers.join(", "),
                "file conflict: multiple modules provide same path"
            );
            conflict_count += 1;
        }
    }

    if conflict_count > 0 {
        warn!(count = conflict_count, "file conflicts detected");
    }

    conflict_count
}
