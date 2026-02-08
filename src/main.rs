mod cli;
mod core;
mod detect;
mod logging;
mod modules;
mod mount;
mod susfs;
mod utils;
mod vfs;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Commands};

const VERSION: &str = "2.0.0";
const MODULE_PROP_PATH: &str = "/data/adb/modules/zeromount/module.prop";

fn read_version_from_prop() -> String {
    let content = match std::fs::read_to_string(MODULE_PROP_PATH) {
        Ok(c) => c,
        Err(_) => return VERSION.to_string(),
    };
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("version=") {
            let v = v.trim();
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

    if let Err(e) = utils::process::camouflage() {
        eprintln!("camouflage failed (non-fatal): {e}");
    }

    logging::init(cli.verbose)?;

    match cli.command {
        Commands::Mount { post_boot } => cli::handlers::handle_mount(post_boot),
        Commands::Detect => cli::handlers::handle_detect(),
        Commands::Status { json } => cli::handlers::handle_status(json),
        Commands::Module { action } => cli::handlers::handle_module(action),
        Commands::Config { action } => cli::handlers::handle_config(action),
        Commands::Vfs { action } => cli::handlers::handle_vfs(action),
        Commands::Uid { action } => cli::handlers::handle_uid(action),
        Commands::Susfs { feature, state } => cli::handlers::handle_susfs(&feature, &state),
        Commands::Diag => cli::handlers::handle_diag(),
        Commands::Version => {
            println!("zeromount v{}", read_version_from_prop());
            Ok(())
        }
    }
}
