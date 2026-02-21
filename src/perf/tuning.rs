use anyhow::Result;
use tracing::{debug, info};

use super::sysfs;
use super::topology::CpuCluster;

pub fn apply_static_tuning(clusters: &[CpuCluster]) -> Result<u32> {
    let mut applied = 0;

    applied += tune_scheduler()?;
    applied += tune_cpufreq_governors(clusters)?;
    applied += tune_vm()?;
    applied += tune_io()?;

    info!(tunables = applied, "static tuning complete");
    Ok(applied)
}

fn tune_scheduler() -> Result<u32> {
    let tunables: &[(&str, &str)] = &[
        ("/proc/sys/kernel/sched_migration_cost_ns", "250000"),
        ("/proc/sys/kernel/sched_min_granularity_ns", "2000000"),
        ("/proc/sys/kernel/sched_wakeup_granularity_ns", "2000000"),
        ("/proc/sys/kernel/sched_child_runs_first", "1"),
    ];

    let mut count = 0;
    for (path, value) in tunables {
        if sysfs::procfs_write(path, value)? {
            count += 1;
        }
    }
    debug!(count, "scheduler tunables applied");
    Ok(count)
}

fn tune_cpufreq_governors(clusters: &[CpuCluster]) -> Result<u32> {
    let mut count = 0;
    for cluster in clusters {
        if cluster.governor != "schedutil" {
            debug!(
                policy = %cluster.policy_path,
                governor = %cluster.governor,
                "skipping rate_limit_us, governor is not schedutil"
            );
            continue;
        }

        let rate_path = format!("{}/schedutil/rate_limit_us", cluster.policy_path);
        if sysfs::sysfs_write(&rate_path, "2000")? {
            count += 1;
        }
    }
    debug!(count, "cpufreq governor tunables applied");
    Ok(count)
}

fn tune_vm() -> Result<u32> {
    let tunables: &[(&str, &str)] = &[
        ("/proc/sys/vm/swappiness", "100"),
        ("/proc/sys/vm/dirty_background_ratio", "5"),
        ("/proc/sys/vm/dirty_ratio", "15"),
        ("/proc/sys/vm/dirty_writeback_centisecs", "300"),
        ("/proc/sys/vm/vfs_cache_pressure", "80"),
        ("/proc/sys/vm/page-cluster", "0"),
    ];

    let mut count = 0;
    for (path, value) in tunables {
        if sysfs::procfs_write(path, value)? {
            count += 1;
        }
    }
    debug!(count, "vm tunables applied");
    Ok(count)
}

fn tune_io() -> Result<u32> {
    let mut count = 0;

    let block_dirs = collect_block_devices();
    for dir in &block_dirs {
        let sched_path = format!("{dir}/queue/scheduler");
        if let Some(available) = sysfs::sysfs_read(&sched_path) {
            let target = if available.contains("mq-deadline") {
                "mq-deadline"
            } else if available.contains("deadline") {
                "deadline"
            } else {
                continue;
            };
            if sysfs::sysfs_write(&sched_path, target)? {
                count += 1;
            }
        }

        let ra_path = format!("{dir}/queue/read_ahead_kb");
        if sysfs::sysfs_write(&ra_path, "64")? {
            count += 1;
        }
    }

    debug!(count, devices = block_dirs.len(), "io tunables applied");
    Ok(count)
}

fn collect_block_devices() -> Vec<String> {
    let mut devices = Vec::new();
    devices.extend(sysfs::glob_dirs("/sys/block/mmcblk*"));
    devices.extend(sysfs::glob_dirs("/sys/block/sd*"));
    devices
}
