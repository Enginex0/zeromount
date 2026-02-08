mod kmsg;
mod rotating;

use std::path::Path;

use anyhow::Result;
use tracing::Level;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

const VERBOSE_MARKER: &str = "/data/adb/zeromount/.verbose";
const LOG_DIR: &str = "/data/adb/zeromount/logs";
const MAX_LOG_SIZE: u64 = 512 * 1024; // 512KB per file
const MAX_LOG_FILES: usize = 3;

pub fn init(verbose_flag: bool) -> Result<()> {
    let verbose = verbose_flag || Path::new(VERBOSE_MARKER).exists();
    let level = if verbose { Level::TRACE } else { Level::INFO };

    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    let kmsg_layer = kmsg::KmsgLayer::new();
    let file_layer = rotating::RotatingFileLayer::new(LOG_DIR, MAX_LOG_SIZE, MAX_LOG_FILES);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(kmsg_layer)
        .with(file_layer)
        .init();

    Ok(())
}
