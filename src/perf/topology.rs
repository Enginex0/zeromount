use tracing::{debug, info, warn};

use super::sysfs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClusterRole {
    Little,
    Mid,
    Big,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CpuCluster {
    pub policy_path: String,
    pub cpus: Vec<u32>,
    pub max_freq_khz: u64,
    pub min_freq_khz: u64,
    pub available_freqs: Vec<u64>,
    pub governor: String,
    pub role: ClusterRole,
}

pub fn detect_clusters() -> Vec<CpuCluster> {
    let policy_dirs = sysfs::glob_dirs("/sys/devices/system/cpu/cpufreq/policy*");
    if policy_dirs.is_empty() {
        debug!("no cpufreq policy directories found");
        return Vec::new();
    }

    let mut clusters: Vec<CpuCluster> = policy_dirs
        .into_iter()
        .filter_map(|dir| {
            let result = parse_policy(&dir);
            if result.is_none() {
                warn!(dir = %dir, "cpufreq policy skipped, could not read scaling_max_freq");
            }
            result
        })
        .collect();

    clusters.sort_by_key(|c| c.max_freq_khz);
    assign_roles(&mut clusters);

    for c in &clusters {
        info!(
            role = ?c.role,
            cpus = ?c.cpus,
            max_khz = c.max_freq_khz,
            governor = %c.governor,
            "detected cluster"
        );
    }

    clusters
}

fn parse_policy(dir: &str) -> Option<CpuCluster> {
    let max_freq_khz = sysfs::sysfs_read_u64(&format!("{dir}/scaling_max_freq"))?;
    let min_freq_khz = sysfs::sysfs_read_u64(&format!("{dir}/scaling_min_freq")).unwrap_or(0);
    let governor = sysfs::sysfs_read(&format!("{dir}/scaling_governor")).unwrap_or_default();
    let cpus = parse_cpu_list(&sysfs::sysfs_read(&format!("{dir}/affected_cpus")).unwrap_or_default());
    let available_freqs = parse_freq_list(
        &sysfs::sysfs_read(&format!("{dir}/scaling_available_frequencies")).unwrap_or_default(),
    );

    Some(CpuCluster {
        policy_path: dir.to_string(),
        cpus,
        max_freq_khz,
        min_freq_khz,
        available_freqs,
        governor,
        role: ClusterRole::Big,
    })
}

fn parse_cpu_list(s: &str) -> Vec<u32> {
    s.split_whitespace()
        .filter_map(|tok| tok.parse().ok())
        .collect()
}

fn parse_freq_list(s: &str) -> Vec<u64> {
    let mut freqs: Vec<u64> = s
        .split_whitespace()
        .filter_map(|tok| tok.parse().ok())
        .collect();
    freqs.sort();
    freqs
}

fn assign_roles(clusters: &mut [CpuCluster]) {
    match clusters.len() {
        0 => {}
        1 => clusters[0].role = ClusterRole::Big,
        2 => {
            clusters[0].role = ClusterRole::Little;
            clusters[1].role = ClusterRole::Big;
        }
        n => {
            clusters[0].role = ClusterRole::Little;
            for c in &mut clusters[1..n - 1] {
                c.role = ClusterRole::Mid;
            }
            clusters[n - 1].role = ClusterRole::Big;
        }
    }
}

pub fn select_boost_freq(cluster: &CpuCluster) -> u64 {
    let pct: u64 = match cluster.role {
        ClusterRole::Big => 80,
        ClusterRole::Mid => 70,
        ClusterRole::Little => 60,
    };

    if !cluster.available_freqs.is_empty() {
        let idx = cluster.available_freqs.len() * pct as usize / 100;
        let idx = idx.min(cluster.available_freqs.len() - 1);
        return cluster.available_freqs[idx];
    }

    cluster.max_freq_khz * pct / 100
}
