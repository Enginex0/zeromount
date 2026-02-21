pub mod boost;
pub mod input;
pub mod recovery;
pub mod sysfs;
pub mod topology;
pub mod tuning;

use anyhow::Result;
use tracing::{info, warn};

pub fn run_perf() -> Result<()> {
    info!("performance system starting");

    let clusters = topology::detect_clusters();
    if clusters.is_empty() {
        warn!("no cpufreq policies found, skipping all tuning");
        return Ok(());
    }

    let applied = tuning::apply_static_tuning(&clusters)?;
    info!(tunables = applied, "static tuning applied");

    let touch_devices = input::detect_touchscreens();
    if touch_devices.is_empty() {
        info!("no touchscreens detected, running static tuning only");
        return Ok(());
    }

    let cluster_boosts: Vec<(String, u64)> = clusters
        .iter()
        .map(|c| (c.policy_path.clone(), topology::select_boost_freq(c)))
        .collect();

    for (path, freq) in &cluster_boosts {
        info!(policy = %path, boost_khz = freq, "boost target");
    }

    let freq_guard = recovery::FreqGuard::capture(&clusters);

    info!("entering input boost daemon");
    boost::run_boost_loop(&touch_devices, &cluster_boosts, &freq_guard)?;

    info!("performance daemon exiting");
    Ok(())
}
