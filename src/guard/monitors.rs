use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::Result;

use crate::core::config::GuardConfig;
use crate::core::config::ZeroMountConfig;

pub fn wait_boot_completed(config: &ZeroMountConfig) -> Result<bool> {
    let timeout = config.guard.boot_timeout_secs;
    for _ in 0..timeout {
        if boot_completed() {
            tracing::info!("boot completed, guard boot watchdog exiting");
            return Ok(true);
        }
        thread::sleep(Duration::from_secs(1));
    }

    tracing::error!(timeout, "boot completion timeout — triggering recovery");
    super::recovery::execute(config);
}

pub fn watch_zygote(config: &ZeroMountConfig) -> Result<bool> {
    let cfg = &config.guard;
    let window = cfg.zygote_watch_secs;
    let poll = cfg.zygote_poll_secs;
    let max_restarts = cfg.zygote_max_restarts;

    let iterations = window / poll.max(1);
    let mut prev_pid = find_pid("zygote64").or_else(|| find_pid("zygote"));
    let mut restarts = 0u32;

    for _ in 0..iterations {
        thread::sleep(Duration::from_secs(poll as u64));
        let cur = find_pid("zygote64").or_else(|| find_pid("zygote"));

        if cur != prev_pid {
            restarts += 1;
            tracing::warn!(restarts, max_restarts, old = ?prev_pid, new = ?cur, "zygote PID changed");
            if restarts >= max_restarts {
                tracing::error!("zygote crash loop detected — triggering recovery");
                super::recovery::execute(config);
            }
        }
        prev_pid = cur;
    }

    tracing::info!(restarts, "zygote watch window complete");
    Ok(true)
}

pub fn watch_systemui(config: &ZeroMountConfig) -> Result<bool> {
    let cfg = &config.guard;

    if !cfg.systemui_monitor_enabled {
        tracing::info!("SystemUI monitor disabled via config");
        return Ok(true);
    }

    for _ in 0..cfg.boot_timeout_secs {
        if boot_completed() {
            break;
        }
        thread::sleep(Duration::from_secs(1));
    }

    loop {
        if !watch_systemui_window(cfg, config) {
            return Ok(false);
        }
    }
}

fn watch_systemui_window(cfg: &GuardConfig, full_config: &ZeroMountConfig) -> bool {
    let window = cfg.systemui_watch_secs;
    let poll = cfg.systemui_poll_secs;
    let max_restarts = cfg.systemui_max_restarts;
    let absent_timeout = cfg.systemui_absent_timeout_secs;

    let iterations = window / poll.max(1);
    let mut prev_pid = find_pid("com.android.systemui");
    let mut restarts = 0u32;
    let mut absent_ticks = 0u32;

    for _ in 0..iterations {
        thread::sleep(Duration::from_secs(poll as u64));
        let cur = find_pid("com.android.systemui");

        match cur {
            None => {
                absent_ticks += poll;
                if absent_ticks >= absent_timeout {
                    tracing::error!(absent_ticks, "SystemUI absent too long — triggering recovery");
                    super::recovery::execute(full_config);
                }
            }
            Some(pid) => {
                absent_ticks = 0;
                if prev_pid.is_some() && Some(pid) != prev_pid {
                    restarts += 1;
                    tracing::warn!(restarts, max_restarts, "SystemUI PID changed");
                    if restarts >= max_restarts {
                        tracing::error!("SystemUI crash loop detected — triggering recovery");
                        super::recovery::execute(full_config);
                    }
                }
            }
        }
        prev_pid = cur;
    }
    true
}

fn find_pid(name: &str) -> Option<u32> {
    let entries = fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        let fname = entry.file_name();
        let fname_str = fname.to_string_lossy();
        if !fname_str.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            continue;
        }

        let cmdline_path = entry.path().join("cmdline");
        if let Ok(data) = fs::read(&cmdline_path) {
            let cmdline = String::from_utf8_lossy(&data);
            let proc_name = cmdline.split('\0').next().unwrap_or("");
            if proc_name == name {
                return fname_str.parse().ok();
            }
        }
    }
    None
}

fn boot_completed() -> bool {
    Command::new("getprop")
        .arg("sys.boot_completed")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or(false, |s| s.trim() == "1")
}
