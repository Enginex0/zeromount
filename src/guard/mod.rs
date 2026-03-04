pub mod markers;
pub mod monitors;
pub mod recovery;

use anyhow::Result;

use crate::cli::GuardAction;
use crate::core::config::ZeroMountConfig;

pub fn handle_guard(action: GuardAction) -> Result<()> {
    let config = ZeroMountConfig::load(None)?;

    if let GuardAction::Status = action {
        return print_status(&config);
    }

    if !config.guard.enabled {
        tracing::debug!("guard disabled, skipping");
        return Ok(());
    }

    match action {
        GuardAction::RecordPfd => {
            markers::record_marker("pfd", config.guard.marker_threshold, &config)?;
        }
        GuardAction::RecordSvc => {
            markers::record_marker("svc", config.guard.marker_threshold, &config)?;
        }
        GuardAction::Check => {
            if markers::any_triggered(config.guard.marker_threshold) {
                std::process::exit(1);
            }
        }
        GuardAction::Clear => {
            markers::clear_all()?;
        }
        GuardAction::WatchBoot => {
            monitors::wait_boot_completed(&config)?;
        }
        GuardAction::WatchZygote => {
            monitors::watch_zygote(&config)?;
        }
        GuardAction::WatchSystemui => {
            monitors::watch_systemui(&config)?;
        }
        GuardAction::Recover => {
            recovery::execute(&config);
        }
        GuardAction::Allow { name } => {
            let mut cfg = config;
            if !cfg.guard.allowed_modules.contains(&name) {
                cfg.guard.allowed_modules.push(name);
                cfg.save()?;
            }
        }
        GuardAction::Disallow { name } => {
            let mut cfg = config;
            cfg.guard.allowed_modules.retain(|m| m != &name);
            cfg.save()?;
        }
        GuardAction::Status => unreachable!(),
    }

    Ok(())
}

fn print_status(config: &ZeroMountConfig) -> Result<()> {
    let (pfd, svc) = markers::status();
    let threshold = config.guard.marker_threshold;
    println!("enabled: {}", config.guard.enabled);
    println!("markers: pfd={pfd}/{threshold} svc={svc}/{threshold}");
    println!("allowed_modules: {}", config.guard.allowed_modules.join(", "));
    if !config.guard.allowed_scripts.is_empty() {
        println!("allowed_scripts: {}", config.guard.allowed_scripts.join(", "));
    }
    Ok(())
}
