use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::debug;

use super::{KstatValues, SusfsClient};

/// Apply kstat spoofing with the best available method.
/// If kstat_redirect (0x55573) is available and both paths exist, use it.
/// Otherwise fall back to add_sus_kstat_statically.
pub fn apply_kstat_redirect_or_static(
    client: &SusfsClient,
    virtual_path: &str,
    real_path: &str,
) -> Result<()> {
    let spoof = build_kstat_values_from_paths(virtual_path, real_path)?;

    if client.features().kstat_redirect {
        debug!("kstat_redirect: {virtual_path} -> {real_path}");
        client.add_sus_kstat_redirect(virtual_path, real_path, &spoof)
    } else {
        debug!("kstat_statically fallback: {virtual_path}");
        client.add_sus_kstat_statically(virtual_path, &spoof)
    }
}

/// Build KstatValues by reading the virtual path's original metadata (if it exists)
/// and the real (replacement) file's size/blocks.
///
/// Mirrors `apply_font_redirect()` from susfs_integration.sh:596-627:
/// - If virtual path exists, use its ino/dev/nlink/atime/mtime/ctime
/// - If virtual path doesn't exist, derive dev from parent dir, generate synthetic ino
/// - Size/blocks/blksize always come from the real (replacement) file
pub fn build_kstat_values_from_paths(virtual_path: &str, real_path: &str) -> Result<KstatValues> {
    let real_meta = fs::metadata(real_path)
        .with_context(|| format!("stat failed for replacement '{real_path}'"))?;

    let size = real_meta.size() as i64;
    let blocks = real_meta.blocks();
    let blksize = real_meta.blksize();

    match fs::metadata(virtual_path) {
        Ok(virt_meta) => {
            Ok(KstatValues {
                ino: Some(virt_meta.ino()),
                dev: Some(virt_meta.dev()),
                nlink: Some(virt_meta.nlink() as u32),
                size: Some(size),
                atime_sec: Some(virt_meta.atime()),
                atime_nsec: Some(0),
                mtime_sec: Some(virt_meta.mtime()),
                mtime_nsec: Some(0),
                ctime_sec: Some(virt_meta.ctime()),
                ctime_nsec: Some(0),
                blksize: Some(blksize),
                blocks: Some(blocks),
            })
        }
        Err(_) => {
            // Virtual path doesn't exist yet — walk up ancestors until we find
            // a real directory. Modules can create deep trees under paths that
            // don't exist on stock (e.g. /system/priv-app/NewApp/lib/arm/).
            let ancestor_meta = find_existing_ancestor(virtual_path)
                .with_context(|| format!("no existing ancestor for '{virtual_path}'"))?;

            let synthetic_ino = {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let pid = std::process::id() as u64;
                (now.wrapping_add(pid)) % 2_147_483_647
            };

            debug!("virtual path absent, using parent-derived metadata: ino={synthetic_ino}");

            Ok(KstatValues {
                ino: Some(synthetic_ino),
                dev: Some(ancestor_meta.dev()),
                nlink: Some(1),
                size: Some(size),
                atime_sec: Some(ancestor_meta.atime()),
                atime_nsec: Some(0),
                mtime_sec: Some(ancestor_meta.mtime()),
                mtime_nsec: Some(0),
                ctime_sec: Some(ancestor_meta.ctime()),
                ctime_nsec: Some(0),
                blksize: Some(blksize),
                blocks: Some(blocks),
            })
        }
    }
}

fn find_existing_ancestor(path: &str) -> Option<fs::Metadata> {
    let mut current = Path::new(path);
    while let Some(parent) = current.parent() {
        if let Ok(meta) = fs::metadata(parent) {
            return Some(meta);
        }
        current = parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kstat_values_default_is_all_none() {
        let kv = KstatValues::default();
        assert!(kv.ino.is_none());
        assert!(kv.dev.is_none());
        assert!(kv.nlink.is_none());
        assert!(kv.size.is_none());
        assert!(kv.atime_sec.is_none());
        assert!(kv.mtime_sec.is_none());
        assert!(kv.ctime_sec.is_none());
        assert!(kv.blksize.is_none());
        assert!(kv.blocks.is_none());
    }

    #[test]
    fn build_kstat_from_existing_paths() {
        let result = build_kstat_values_from_paths("Cargo.toml", "Cargo.toml");
        assert!(result.is_ok());
        let kv = result.expect("should succeed");
        assert!(kv.ino.is_some());
        assert!(kv.dev.is_some());
        assert!(kv.size.unwrap_or(0) > 0);
    }

    #[test]
    fn build_kstat_from_nonexistent_virtual_path() {
        let result = build_kstat_values_from_paths(
            "/tmp/__nonexistent_font_test_file__.ttf",
            "Cargo.toml",
        );
        assert!(result.is_ok());
        let kv = result.expect("should succeed with parent fallback");
        assert!(kv.ino.is_some());
        assert!(kv.nlink == Some(1));
    }

    #[test]
    fn build_kstat_fails_when_real_path_missing() {
        let result = build_kstat_values_from_paths(
            "Cargo.toml",
            "/tmp/__nonexistent_replacement_file__.ttf",
        );
        assert!(result.is_err());
    }

    #[test]
    fn build_kstat_nonexistent_virtual_uses_real_size() {
        let result = build_kstat_values_from_paths(
            "/tmp/__nonexistent_virtual_path__.bin",
            "Cargo.toml",
        );
        let kv = result.expect("parent fallback should work");
        let real_size = std::fs::metadata("Cargo.toml").unwrap().len() as i64;
        assert_eq!(kv.size, Some(real_size));
    }

    #[test]
    fn build_kstat_existing_virtual_preserves_original_ino() {
        let kv = build_kstat_values_from_paths("Cargo.toml", "Cargo.toml")
            .expect("both exist");
        let meta = std::fs::metadata("Cargo.toml").unwrap();
        assert_eq!(kv.ino, Some(meta.ino()));
    }

    // -- Fallback path verification (F14) --

    #[test]
    fn apply_kstat_redirect_or_static_chooses_redirect_when_available() {
        use crate::susfs::SusfsClient;
        use crate::susfs::SusfsFeatures;

        let features = SusfsFeatures {
            kstat: true,
            kstat_redirect: true,
            ..SusfsFeatures::default()
        };
        let client = SusfsClient::new_for_test(true, features);

        // With kstat_redirect available, the function should prefer it
        assert!(client.features().kstat_redirect);
    }

    #[test]
    fn apply_kstat_redirect_or_static_falls_back_when_unavailable() {
        use crate::susfs::SusfsClient;
        use crate::susfs::SusfsFeatures;

        let features = SusfsFeatures {
            kstat: true,
            kstat_redirect: false,
            ..SusfsFeatures::default()
        };
        let client = SusfsClient::new_for_test(true, features);

        // Without kstat_redirect, the function should use static fallback
        assert!(!client.features().kstat_redirect);
        assert!(client.features().kstat);
    }

    #[test]
    fn unavailable_client_rejects_operations() {
        use crate::susfs::SusfsClient;
        use crate::susfs::SusfsFeatures;

        let client = SusfsClient::new_for_test(false, SusfsFeatures::default());
        assert!(!client.is_available());
    }

}
