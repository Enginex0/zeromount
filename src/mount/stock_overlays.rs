use std::collections::HashMap;

use tracing::{debug, info, warn};

pub struct StockOverlay {
    pub mount_point: String,
    #[allow(dead_code)]
    pub peer_group_id: u32,
    /// Base filesystem s_dev (kernel-internal format) to spoof overlay stat to
    pub base_dev: u32,
}

pub fn collect_stock_overlays() -> Vec<StockOverlay> {
    let content = match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(c) => c,
        Err(e) => {
            info!(error = %e, "mountinfo read failed, skipping stock overlay collection");
            return Vec::new();
        }
    };

    let mounts = parse_all_mounts(&content);
    let id_map: HashMap<u32, usize> = mounts
        .iter()
        .enumerate()
        .map(|(i, m)| (m.mount_id, i))
        .collect();

    debug!(
        lines = content.lines().count(),
        mounts = mounts.len(),
        "mountinfo parsed for stock overlay scan"
    );

    let mut results = Vec::new();
    for (i, mount) in mounts.iter().enumerate() {
        if mount.fs_type != "overlay" {
            continue;
        }
        if !has_oem_lowerdir(&mount.super_opts) {
            debug!(path = %mount.mount_point, "overlay skipped: not OEM lowerdir");
            continue;
        }

        let base_dev = find_base_dev(&mounts, &id_map, i);
        let overlay_dev = make_dev(mount.dev_major, mount.dev_minor);

        debug!(
            path = %mount.mount_point,
            overlay_dev = overlay_dev,
            base_dev = base_dev,
            parent_mount_id = mount.parent_id,
            "stock OEM overlay found"
        );

        results.push(StockOverlay {
            mount_point: mount.mount_point.clone(),
            peer_group_id: mount.peer_group_id,
            base_dev,
        });
    }

    if results.is_empty() {
        debug!("no stock OEM overlays found in mountinfo");
    } else {
        info!(count = results.len(), "stock OEM overlays collected");
    }
    results
}

struct MountEntry {
    mount_id: u32,
    parent_id: u32,
    dev_major: u32,
    dev_minor: u32,
    mount_point: String,
    peer_group_id: u32,
    fs_type: String,
    super_opts: String,
}

fn parse_all_mounts(content: &str) -> Vec<MountEntry> {
    content
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let sep = fields.iter().position(|&f| f == "-")?;

            let mount_id: u32 = fields.first()?.parse().ok()?;
            let parent_id: u32 = fields.get(1)?.parse().ok()?;
            let (major, minor) = fields.get(2)?.split_once(':')?;
            let mount_point = (*fields.get(4)?).to_string();
            let fs_type = (*fields.get(sep + 1)?).to_string();
            let super_opts = fields.get(sep + 3).unwrap_or(&"").to_string();

            let peer_group_id = fields[5..sep]
                .iter()
                .find_map(|f| f.strip_prefix("shared:"))
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            Some(MountEntry {
                mount_id,
                parent_id,
                dev_major: major.parse().ok()?,
                dev_minor: minor.parse().ok()?,
                mount_point,
                peer_group_id,
                fs_type,
                super_opts,
            })
        })
        .collect()
}

/// Walk up the mount tree from an overlay to find the first non-overlay ancestor.
/// Returns that ancestor's s_dev in kernel-internal format (major << 20 | minor).
fn find_base_dev(
    mounts: &[MountEntry],
    id_map: &HashMap<u32, usize>,
    overlay_idx: usize,
) -> u32 {
    let mut current = &mounts[overlay_idx];

    for depth in 0..20 {
        if let Some(&parent_idx) = id_map.get(&current.parent_id) {
            let parent = &mounts[parent_idx];
            if parent.fs_type != "overlay" {
                debug!(
                    child = %mounts[overlay_idx].mount_point,
                    ancestor = %parent.mount_point,
                    ancestor_dev = format!("{}:{}", parent.dev_major, parent.dev_minor),
                    depth = depth + 1,
                    "base filesystem found"
                );
                return make_dev(parent.dev_major, parent.dev_minor);
            }
            current = parent;
        } else {
            break;
        }
    }

    // Fallback: root mount
    if let Some(root) = mounts.iter().find(|m| m.parent_id == 0 || m.mount_id == 1) {
        warn!(
            overlay = %mounts[overlay_idx].mount_point,
            "fell through to root mount for base_dev"
        );
        return make_dev(root.dev_major, root.dev_minor);
    }

    warn!(overlay = %mounts[overlay_idx].mount_point, "could not determine base_dev");
    0
}

/// Kernel-internal dev_t: (major << 20) | minor
fn make_dev(major: u32, minor: u32) -> u32 {
    (major << 20) | minor
}

fn has_oem_lowerdir(super_opts: &str) -> bool {
    for opt in super_opts.split(',') {
        if let Some(lowerdir) = opt.strip_prefix("lowerdir=") {
            return lowerdir.contains("mi_ext/")
                || lowerdir.contains("prism/")
                || lowerdir.contains("optics/")
                || lowerdir.contains("/my_")
                || lowerdir.starts_with("my_")
                || lowerdir.contains("pangu/")
                || lowerdir.contains("cust/")
                || lowerdir.contains("/odm/overlay")
                || lowerdir.contains("reserve/");
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_xiaomi_mi_ext() {
        let content = "95 49 0:34 / /product/overlay ro,relatime shared:26 - overlay overlay ro,seclabel,lowerdir=/mnt/vendor/mi_ext/product/overlay/:/product/overlay\n\
                        49 1 253:5 /product /product ro,relatime shared:2 - erofs /dev/block/dm-5 ro,seclabel";
        let overlays = collect_from_content(content);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].mount_point, "/product/overlay");
        assert_eq!(overlays[0].peer_group_id, 26);
        // base_dev should be 253:5 = (253 << 20) | 5
        assert_eq!(overlays[0].base_dev, make_dev(253, 5));
    }

    #[test]
    fn parse_samsung_prism() {
        let content = "100 49 0:40 / /system/priv-app ro,relatime shared:30 - overlay overlay ro,seclabel,lowerdir=/prism/system/priv-app:/system/priv-app\n\
                        49 1 253:5 / / ro,relatime shared:1 - erofs /dev/block/dm-5 ro,seclabel";
        let overlays = collect_from_content(content);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].mount_point, "/system/priv-app");
        assert_eq!(overlays[0].base_dev, make_dev(253, 5));
    }

    #[test]
    fn parse_oppo_my_product() {
        let content = "110 49 0:50 / /product/app ro,relatime shared:35 - overlay overlay ro,seclabel,lowerdir=/my_product/app:/product/app\n\
                        49 1 253:5 / / ro,relatime shared:1 - erofs /dev/block/dm-5 ro,seclabel";
        let overlays = collect_from_content(content);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].mount_point, "/product/app");
    }

    #[test]
    fn skip_zeromount_overlay() {
        let content = "151 36 0:89 / /system/bin rw,relatime shared:40 - overlay KSU ro,seclabel,lowerdir=/mnt/abc123/clean/system/bin:/system/bin\n\
                        36 1 253:5 / / ro,relatime shared:1 - erofs /dev/block/dm-5 ro,seclabel";
        let overlays = collect_from_content(content);
        assert!(overlays.is_empty());
    }

    #[test]
    fn skip_non_overlay() {
        let content = "36 35 253:5 / / ro,relatime shared:1 - erofs /dev/block/dm-5 ro,seclabel";
        let overlays = collect_from_content(content);
        assert!(overlays.is_empty());
    }

    #[test]
    fn nested_overlay_walks_to_base() {
        // /product is overlay (parent=1), /product/overlay is overlay (parent=49)
        // Should walk up: 95 -> 49 (overlay) -> 1 (erofs)
        let content = "1 0 253:5 / / ro,relatime shared:1 - erofs /dev/block/dm-5 ro,seclabel\n\
                        49 1 0:42 / /product ro,relatime shared:2 - overlay overlay ro,seclabel,lowerdir=/mi_ext/product/:/product\n\
                        95 49 0:34 / /product/overlay ro,relatime shared:26 - overlay overlay ro,seclabel,lowerdir=/mi_ext/product/overlay/:/product/overlay";
        let overlays = collect_from_content(content);
        assert_eq!(overlays.len(), 2);
        // Both should resolve to the root erofs s_dev
        for o in &overlays {
            assert_eq!(o.base_dev, make_dev(253, 5));
        }
    }

    #[test]
    fn oem_pattern_mi_ext() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/mnt/vendor/mi_ext/product/app/:/product/app"));
    }

    #[test]
    fn oem_pattern_mi_ext_relative() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=mi_ext/product/app:/product/app"));
    }

    #[test]
    fn oem_pattern_prism() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/prism/system/app:/system/app"));
    }

    #[test]
    fn oem_pattern_optics() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/optics/overlay:/product/overlay"));
    }

    #[test]
    fn oem_pattern_oppo_my() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/my_product/app:/product/app"));
    }

    #[test]
    fn oem_pattern_pangu() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/pangu/product/app:/product/app"));
    }

    #[test]
    fn oem_pattern_cust() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/cust/product/overlay:/product/overlay"));
    }

    #[test]
    fn oem_pattern_odm_overlay() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/odm/overlay:/product/overlay"));
    }

    #[test]
    fn oem_pattern_reserve() {
        assert!(has_oem_lowerdir("ro,seclabel,lowerdir=/reserve/product/app:/product/app"));
    }

    #[test]
    fn non_oem_pattern() {
        assert!(!has_oem_lowerdir("ro,seclabel,lowerdir=/mnt/abc123/clean/system/bin:/system/bin"));
    }

    #[test]
    fn make_dev_format() {
        assert_eq!(make_dev(0, 34), 34);
        assert_eq!(make_dev(253, 5), (253 << 20) | 5);
        assert_eq!(make_dev(253, 5) >> 20, 253);
        assert_eq!(make_dev(253, 5) & ((1 << 20) - 1), 5);
    }

    // Test helper: collect from content string (bypasses /proc/self/mountinfo)
    fn collect_from_content(content: &str) -> Vec<StockOverlay> {
        let mounts = parse_all_mounts(content);
        let id_map: HashMap<u32, usize> = mounts
            .iter()
            .enumerate()
            .map(|(i, m)| (m.mount_id, i))
            .collect();

        let mut results = Vec::new();
        for (i, mount) in mounts.iter().enumerate() {
            if mount.fs_type != "overlay" {
                continue;
            }
            if !has_oem_lowerdir(&mount.super_opts) {
                continue;
            }
            results.push(StockOverlay {
                mount_point: mount.mount_point.clone(),
                peer_group_id: mount.peer_group_id,
                base_dev: find_base_dev(&mounts, &id_map, i),
            });
        }
        results
    }
}
