use tracing::{debug, warn};

use super::sysfs;
use super::topology::CpuCluster;

struct SavedFreq {
    policy_path: String,
    original_min_freq: String,
}

pub struct FreqGuard {
    saved: Vec<SavedFreq>,
}

impl FreqGuard {
    pub fn capture(clusters: &[CpuCluster]) -> Self {
        let saved: Vec<SavedFreq> = clusters
            .iter()
            .filter_map(|c| {
                let path = format!("{}/scaling_min_freq", c.policy_path);
                let freq = sysfs::sysfs_read(&path)?;
                debug!(policy = %c.policy_path, freq = %freq, "captured original min_freq");
                Some(SavedFreq {
                    policy_path: c.policy_path.clone(),
                    original_min_freq: freq,
                })
            })
            .collect();

        debug!(count = saved.len(), "frequency snapshot captured");

        Self { saved }
    }

    pub fn restore(&self) {
        for s in &self.saved {
            let path = format!("{}/scaling_min_freq", s.policy_path);
            match sysfs::sysfs_write(&path, &s.original_min_freq) {
                Ok(true) => debug!(policy = %s.policy_path, freq = %s.original_min_freq, "restored"),
                Ok(false) => warn!(policy = %s.policy_path, "restore skipped, path missing"),
                Err(e) => warn!(policy = %s.policy_path, %e, "restore failed"),
            }
        }
    }
}

impl Drop for FreqGuard {
    fn drop(&mut self) {
        self.restore();
    }
}
