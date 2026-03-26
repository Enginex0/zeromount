pub mod monitors;
pub mod recovery;

use std::path::Path;

use anyhow::Result;

use crate::cli::GuardAction;
use crate::core::config::ZeroMountConfig;

pub fn handle_guard(action: GuardAction) -> Result<()> {
    let config = ZeroMountConfig::load(None)?;

    if let GuardAction::Status = action {
        return print_status();
    }

    if !config.guard.enabled {
        tracing::debug!("guard disabled, skipping");
        return Ok(());
    }

    match action {
        GuardAction::Check => {
            if recovery::is_locked_out()
                || Path::new("/data/adb/modules/meta-zeromount/disable").exists()
            {
                std::process::exit(1);
            }
        }
        GuardAction::ClearLockout => {
            recovery::clear_lockout();
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
        GuardAction::Status => unreachable!(),
    }

    Ok(())
}

fn print_status() -> Result<()> {
    let bootcount = ZeroMountConfig::read_bootcount();
    let disabled = Path::new("/data/adb/modules/meta-zeromount/disable").exists();
    let lockout = recovery::is_locked_out();
    println!("bootcount: {bootcount}");
    println!("disabled: {disabled}");
    println!("recovery_lockout: {lockout}");
    Ok(())
}
