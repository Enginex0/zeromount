mod bridge;
mod cli;
mod core;
mod detect;
mod guard;
mod logging;
mod modules;
mod mount;
mod susfs;
mod perf;
mod prop;
mod utils;
mod vfs;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MODULE_PROP_PATH: &str = "/data/adb/modules/meta-zeromount/module.prop";

fn read_version_from_prop() -> String {
    let content = match std::fs::read_to_string(MODULE_PROP_PATH) {
        Ok(c) => c,
        Err(_) => return VERSION.to_string(),
    };
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("version=") {
            let v = v.trim().strip_prefix('v').unwrap_or(v.trim());
            if !v.is_empty() {
                return v.to_string();
            }
        }
    }
    VERSION.to_string()
}

fn main() -> Result<()> {
    // Parse args first (clap copies into owned Strings), then camouflage
    // the process before any visible work (R08: fixes BUG-L3).
    let cli = Cli::parse();

    std::panic::set_hook(Box::new(|info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("unknown panic");

        let loc = info
            .location()
            .map(|l| format!(" ({}:{})", l.file(), l.line()))
            .unwrap_or_default();

        let desc = format!("❌ Crashed: {msg}{loc}");
        eprintln!("zeromount panic: {msg}{loc}");
        let _ = utils::platform::write_description_to_module_prop(&desc);
    }));

    if let Err(e) = utils::process::camouflage() {
        eprintln!("camouflage failed (non-fatal): {e}");
    }

    let config = core::config::ZeroMountConfig::load(None)?;
    logging::init(cli.verbose, &config.logging)?;
    utils::signal::register_shutdown_handler();

    let is_mount = matches!(cli.command, Commands::Mount);

    let result = match cli.command {
        Commands::Mount => cli::handlers::handle_mount(),
        Commands::Detect => cli::handlers::handle_detect(),
        Commands::Status { json } => cli::handlers::handle_status(json),
        Commands::Module { action } => cli::handlers::handle_module(action),
        Commands::Config { action } => cli::handlers::handle_config(action),
        Commands::Vfs { action } => cli::handlers::handle_vfs(action),
        Commands::Uid { action } => cli::handlers::handle_uid(action),
        Commands::Log { action } => cli::handlers::handle_log(action),
        Commands::Bridge { action } => cli::handlers::handle_bridge(action),
        Commands::Susfs { feature, state } => cli::handlers::handle_susfs(&feature, &state),
        Commands::Watch => cli::handlers::handle_watch(),
        Commands::Perf => cli::handlers::handle_perf(),
        Commands::PropWatch => cli::handlers::handle_prop_watch(),
        Commands::Diag => cli::handlers::handle_diag(),
        Commands::CleanupStale => cli::handlers::handle_cleanup_stale(),
        Commands::Emoji { action } => cli::handlers::handle_emoji(action),
        Commands::VoldAppData => cli::handlers::handle_vold_app_data(),
        Commands::TryUmount => cli::handlers::handle_try_umount(),
        Commands::WebUiInit => cli::webui_init::handle_webui_init(),
        Commands::Guard { action } => guard::handle_guard(action),
        Commands::SyncDescription => cli::handlers::handle_sync_description(),
        Commands::Version => {
            println!("zeromount v{}", read_version_from_prop());
            Ok(())
        }
    };

    if let Err(ref e) = result {
        if is_mount {
            let desc = format!("❌ Mount failed: {e:#}");
            let _ = utils::platform::write_description_to_module_prop(&desc);
        }
    }

    result
}
