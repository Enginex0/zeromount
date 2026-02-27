pub mod dump;
mod kmsg;
mod rotating;
pub mod sysfs;

use std::path::Path;

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

use crate::core::config::LoggingConfig;

const VERBOSE_MARKER: &str = "/data/adb/zeromount/.verbose";

pub fn init(verbose_flag: bool, config: &LoggingConfig) -> Result<()> {
    let verbose = verbose_flag || config.verbose || Path::new(VERBOSE_MARKER).exists();
    let level = if verbose { Level::TRACE } else { Level::INFO };

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let log_dir = config.log_dir.to_string_lossy();
    // Verbose sessions generate ~10x more events; bump rotation budget to 5MB x 5 = 25MB
    let (max_size_mb, max_files) = if verbose { (5u64, 5usize) } else { (config.max_log_size_mb as u64, config.max_log_files as usize) };
    let max_size = max_size_mb * 1024 * 1024;

    let kmsg_layer = kmsg::KmsgLayer::new();
    let file_layer = rotating::RotatingFileLayer::new(&log_dir, max_size, max_files);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(kmsg_layer)
        .with(file_layer)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    Ok(())
}
