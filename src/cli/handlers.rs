use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::{ConfigAction, LogAction, ModuleAction, UidAction, VfsAction};

pub fn handle_mount() -> Result<()> {
    let _lock = match crate::utils::lock::acquire_instance_lock()? {
        Some(guard) => guard,
        None => {
            warn!("another zeromount instance is running, exiting");
            return Ok(());
        }
    };

    tracing::info!("mount pipeline started");

    let config = crate::core::config::ZeroMountConfig::load(None)?;
    let state = crate::core::pipeline::run_pipeline_with_bootloop_guard(config)?;

    tracing::info!(
        scenario = ?state.scenario,
        rules = state.rule_count,
        modules = state.modules.len(),
        degraded = state.degraded,
        "pipeline finished"
    );

    Ok(())
}

pub fn handle_log(action: LogAction) -> Result<()> {
    match action {
        LogAction::Enable => crate::logging::sysfs::enable(),
        LogAction::Disable => crate::logging::sysfs::disable(),
        LogAction::Level { level } => crate::logging::sysfs::set_level(level),
        LogAction::Status => crate::logging::sysfs::status(),
    }
}

pub fn handle_detect() -> Result<()> {
    let result = crate::detect::detect_and_persist()?;

    println!("scenario: {:?}", result.scenario);
    if let Some(ver) = result.driver_version {
        println!("driver_version: v{}", ver);
    }
    println!("vfs_driver: {}", result.capabilities.vfs_driver);
    println!("susfs: {}", result.capabilities.susfs_available);
    if result.capabilities.susfs_available {
        if let Some(ref v) = result.capabilities.susfs_version {
            println!("  version: {}", v);
        }
        println!("  kstat: {}", result.capabilities.susfs_kstat);
        println!("  path: {}", result.capabilities.susfs_path);
        println!("  maps: {}", result.capabilities.susfs_maps);
        println!("  open_redirect: {}", result.capabilities.susfs_open_redirect);
        println!("  kstat_redirect: {}", result.capabilities.susfs_kstat_redirect);
        println!("  open_redirect_all: {}", result.capabilities.susfs_open_redirect_all);
    }
    println!("overlay: {}", result.capabilities.overlay_supported);
    println!("tmpfs_xattr: {}", result.capabilities.tmpfs_xattr);

    tracing::debug!(scenario = ?result.scenario, "detection complete");
    Ok(())
}

pub fn build_runtime_status() -> crate::core::types::RuntimeState {
    let status_path = std::path::Path::new("/data/adb/zeromount/.status.json");
    let mut state = crate::core::types::RuntimeState::read_status_file(status_path)
        .unwrap_or_default();

    if !state.capabilities.vfs_driver && !state.capabilities.susfs_available {
        match crate::detect::load_detection() {
            Ok(det) => {
                debug!(
                    scenario = ?det.scenario,
                    susfs = det.capabilities.susfs_available,
                    "backfilling status from .detection.json"
                );
                state.scenario = det.scenario;
                state.capabilities = det.capabilities;
                state.driver_version = det.driver_version;
            }
            Err(e) => {
                warn!("no .status.json or .detection.json available: {e}");
            }
        }
    }

    if let Ok(driver) = crate::vfs::VfsDriver::open() {
        state.capabilities.vfs_driver = true;
        if let Ok(v) = driver.get_version() {
            state.driver_version = Some(v);
        }
        if let Ok(Some(s)) = driver.get_status() {
            state.engine_active = Some(s.enabled);
        }
    }

    if let Ok(mgr) = crate::utils::platform::detect_root_manager() {
        state.root_manager = Some(mgr.name().to_string());
    }

    state
}

pub fn handle_status(json: bool) -> Result<()> {
    let state = build_runtime_status();

    if json {
        let out = serde_json::to_string_pretty(&state)?;
        println!("{out}");
    } else {
        println!("scenario: {:?}", state.scenario);
        println!("engine_active: {:?}", state.engine_active);
        println!("modules: {}", state.modules.len());
        if !state.font_modules.is_empty() {
            println!("font_modules: {}", state.font_modules.join(", "));
        }
    }
    Ok(())
}

pub fn handle_module(action: ModuleAction) -> Result<()> {
    let modules_dir = Path::new("/data/adb/modules");
    match action {
        ModuleAction::List => {
            let status_path = Path::new("/data/adb/zeromount/.status.json");
            let state = crate::core::types::RuntimeState::read_status_file(status_path)
                .unwrap_or_default();
            if state.modules.is_empty() {
                println!("no modules loaded");
            } else {
                for m in &state.modules {
                    println!(
                        "{}: strategy={:?} rules={}/{} paths={}",
                        m.id,
                        m.strategy,
                        m.rules_applied,
                        m.rules_applied + m.rules_failed,
                        m.mount_paths.len()
                    );
                }
            }
        }
        ModuleAction::Scan { update_conf, cleanup } => {
            if let Some(module_id) = cleanup {
                tracing::debug!("cleaning rules for uninstalled module: {module_id}");
                // VFS clear for specific module would need kernel support;
                // for now, full clear + re-mount is the safe path
                if let Ok(driver) = crate::vfs::VfsDriver::open() {
                    let _ = driver.clear_all();
                    tracing::debug!("cleared VFS rules after module uninstall");
                }
            }

            let modules = crate::modules::scanner::scan_modules(modules_dir)?;
            println!("scan complete: {} modules", modules.len());
            for m in &modules {
                println!("  {} ({} files)", m.id, m.files.len());
            }

            if update_conf {
                tracing::debug!("partitions.conf rebuild requested");
            }
        }
    }
    Ok(())
}

pub fn handle_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Defaults => {
            let defaults = crate::core::config::ZeroMountConfig::default();
            let toml = toml::to_string_pretty(&defaults)?;
            print!("{toml}");
            return Ok(());
        }
        _ => {}
    }
    let mut config = crate::core::config::ZeroMountConfig::load(None)?;
    match action {
        ConfigAction::Get { key } => {
            match config.get(&key) {
                Some(value) => println!("{value}"),
                None => anyhow::bail!("unknown key: {key}"),
            }
        }
        ConfigAction::Set { key, value } => {
            config.set(&key, &value)?;
            config.save()?;
            println!("ok");
        }
        ConfigAction::Restore => {
            let restored = crate::core::config::ZeroMountConfig::restore_backup()?;
            restored.save()?;
            println!("config restored from backup");
        }
        ConfigAction::Dump { json } => {
            if json {
                let json_str = serde_json::to_string(&config)?;
                println!("{json_str}");
            } else {
                let toml_str = toml::to_string_pretty(&config)?;
                print!("{toml_str}");
            }
        }
        ConfigAction::Defaults => unreachable!(),
    }
    Ok(())
}

pub fn handle_vfs(action: VfsAction) -> Result<()> {
    let driver = crate::vfs::VfsDriver::open()
        .context("cannot open /dev/zeromount -- is the kernel module loaded?")?;

    match action {
        VfsAction::Add { virtual_path, real_path } => {
            let vp = Path::new(&virtual_path);
            let rp = Path::new(&real_path);
            let is_dir = rp.is_dir();
            driver.add_rule(vp, rp, is_dir)?;
            println!("ok");
        }
        VfsAction::Del { virtual_path } => {
            // del_rule needs both paths; use virtual_path for both since
            // the kernel matches on virtual_path
            let vp = Path::new(&virtual_path);
            driver.del_rule(vp, vp)?;
            println!("ok");
        }
        VfsAction::Clear => {
            driver.clear_all()?;
            println!("ok");
        }
        VfsAction::Enable => {
            driver.enable()?;
            println!("ok");
        }
        VfsAction::Disable => {
            driver.disable()?;
            println!("ok");
        }
        VfsAction::Refresh => {
            driver.refresh()?;
            println!("ok");
        }
        VfsAction::List => {
            let list = driver.get_list()?;
            if list.is_empty() {
                println!("no rules");
            } else {
                print!("{list}");
            }
        }
        VfsAction::QueryStatus => {
            match driver.get_status()? {
                Some(status) => {
                    println!(
                        "engine: {} rules: {}",
                        if status.enabled { "active" } else { "inactive" },
                        status.rule_count
                    );
                }
                None => println!("engine: unknown (GET_STATUS not supported by kernel)"),
            }
        }
    }
    Ok(())
}

pub fn handle_uid(action: UidAction) -> Result<()> {
    let driver = crate::vfs::VfsDriver::open()
        .context("cannot open /dev/zeromount -- is the kernel module loaded?")?;

    match action {
        UidAction::Block { uid } => {
            driver.add_uid(uid)?;
            println!("ok");
        }
        UidAction::Unblock { uid } => {
            driver.del_uid(uid)?;
            println!("ok");
        }
    }
    Ok(())
}

pub fn handle_susfs(feature: &str, state: &str) -> Result<()> {
    let mut config = crate::core::config::ZeroMountConfig::load(None)?;

    // Map CLI feature names to config keys
    let key = match feature {
        "kstat" => "susfs.kstat",
        "path" | "path_hide" => "susfs.path_hide",
        "maps" | "maps_hide" => "susfs.maps_hide",
        "redirect" | "open_redirect" => "susfs.open_redirect",
        "enabled" => "susfs.enabled",
        "log" => "brene.susfs_log",
        _ => anyhow::bail!("unknown SUSFS feature: {feature} (try: kstat, path, maps, redirect, enabled, log)"),
    };

    config.set(key, state)?;
    config.save()?;
    println!("ok");
    Ok(())
}

pub fn handle_susfs_retry(wait: bool) -> Result<()> {
    if wait {
        if !wait_for_sdcard(Duration::from_secs(120)) {
            warn!("sdcard not available after 120s, skipping SUSFS retry");
            return Ok(());
        }
    }

    let _lock = match crate::utils::lock::acquire_instance_lock()? {
        Some(guard) => guard,
        None => {
            warn!("another zeromount instance is running, skipping SUSFS retry");
            return Ok(());
        }
    };

    tracing::info!("deferred SUSFS retry started");

    let mut config = crate::core::config::ZeroMountConfig::load(None)?;
    if let Err(e) = crate::susfs::brene::import_susfs_config(&mut config) {
        warn!("SUSFS config import failed: {e}");
    }

    if !config.susfs.enabled {
        tracing::info!("SUSFS disabled in config, skipping deferred retry");
        return Ok(());
    }

    let client = match crate::susfs::SusfsClient::probe() {
        Ok(c) if c.is_available() => c,
        Ok(_) => {
            tracing::info!("SUSFS not available, skipping retry");
            return Ok(());
        }
        Err(e) => {
            warn!("SUSFS probe failed: {e}");
            return Ok(());
        }
    };

    client.ensure_root_paths();

    // Only apply per-module protections for modules the boot pipeline mounted
    let status_path = Path::new("/data/adb/zeromount/.status.json");
    let boot_module_ids: Vec<String> = crate::core::types::RuntimeState::read_status_file(status_path)
        .map(|s| s.modules.iter().map(|m| m.id.clone()).collect())
        .unwrap_or_default();

    // Path hiding only — kstat already succeeded at boot.
    // Skip kstat to avoid phantom file failures from VFS-redirected directories.
    let modules_dir = Path::new("/data/adb/modules");
    if modules_dir.exists() && !boot_module_ids.is_empty() {
        if let Ok(modules) = crate::modules::scanner::scan_modules(modules_dir) {
            for module in &modules {
                if !module.files.is_empty() && boot_module_ids.contains(&module.id) {
                    crate::vfs::executor::apply_module_susfs_protections(
                        &client, module, Some(&config.susfs), true, false,
                    );
                }
            }
        }
    }

    let susfs_mode = crate::detect::load_detection()
        .map(|d| d.capabilities.susfs_mode)
        .unwrap_or(crate::core::types::SusfsMode::Absent);
    match crate::susfs::brene::apply_brene_deferred(&client, &config, susfs_mode) {
        Ok(brene) => {
            tracing::info!(
                paths = brene.paths_hidden,
                "deferred SUSFS retry complete"
            );

            if let Ok(mut state) = crate::core::types::RuntimeState::read_status_file(status_path) {
                state.hidden_path_count = brene.paths_hidden;
                let tmp_path = status_path.with_extension("json.tmp");
                if let Ok(()) = state.write_status_file(&tmp_path) {
                    let _ = std::fs::rename(&tmp_path, status_path);
                }
            }
        }
        Err(e) => {
            warn!("deferred BRENE failed: {e}");
        }
    }

    Ok(())
}

pub fn handle_perf() -> Result<()> {
    let config = crate::core::config::ZeroMountConfig::load(None)?;
    if !config.perf.enabled {
        tracing::info!("perf.enabled = false, exiting");
        return Ok(());
    }
    crate::perf::run_perf()
}

pub fn handle_watch() -> Result<()> {
    let modules_dir = Path::new("/data/adb/modules");
    tracing::info!("starting module watcher on {}", modules_dir.display());

    let mut state = crate::core::types::RuntimeState::read_status_file(
        Path::new("/data/adb/zeromount/.status.json"),
    )
    .unwrap_or_default();

    crate::detect::watcher::start_module_watcher(modules_dir, || {
        tracing::info!("module change detected, updating status");
        crate::detect::watcher::touch_status_timestamp(&mut state);
        Ok(())
    })
}

// inotify constants (linux/inotify.h)
const IN_CREATE: u32 = 0x0000_0100;
const IN_MOVED_TO: u32 = 0x0000_0080;

const SDCARD_TARGETS: &[&str] = &[
    "/data/media/0/Android/data",
    "/storage/emulated/0/Android/data",
    "/sdcard/Android/data",
];

// Always exists early in boot — raw emulated storage before FUSE mount
const SDCARD_WATCH_DIR: &str = "/data/media/0";

fn sdcard_available() -> bool {
    SDCARD_TARGETS.iter().any(|p| Path::new(p).exists())
}

/// Block until sdcard paths appear. Uses inotify on /data/media/0 for instant
/// reaction to Android/ directory creation; falls back to 250ms polling.
fn wait_for_sdcard(timeout: Duration) -> bool {
    if sdcard_available() {
        debug!("sdcard already available");
        return true;
    }

    tracing::info!("waiting for sdcard decryption via inotify on {SDCARD_WATCH_DIR}");

    let watch_dir = Path::new(SDCARD_WATCH_DIR);
    if !watch_dir.exists() {
        warn!("{SDCARD_WATCH_DIR} not present, falling back to polling");
        return poll_for_sdcard(timeout);
    }

    // SAFETY: inotify_init1 flags are valid constants; returns fd or -1 (checked below).
    let fd = unsafe { libc::inotify_init1(libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        warn!("inotify_init1 failed, falling back to polling");
        return poll_for_sdcard(timeout);
    }

    let c_path = match std::ffi::CString::new(SDCARD_WATCH_DIR) {
        Ok(p) => p,
        Err(_) => {
            // SAFETY: fd is a valid open file descriptor from inotify_init1 above.
            unsafe { libc::close(fd); }
            return poll_for_sdcard(timeout);
        }
    };

    // SAFETY: fd is valid from inotify_init1; CString is non-null NUL-terminated.
    let wd = unsafe {
        libc::inotify_add_watch(fd, c_path.as_ptr(), IN_CREATE | IN_MOVED_TO)
    };
    if wd < 0 {
        // SAFETY: fd is a valid open file descriptor from inotify_init1 above.
        unsafe { libc::close(fd); }
        warn!("inotify_add_watch failed, falling back to polling");
        return poll_for_sdcard(timeout);
    }

    let deadline = Instant::now() + timeout;
    let result = loop {
        if crate::utils::signal::shutdown_requested() {
            debug!("shutdown requested, aborting sdcard wait");
            break false;
        }

        if sdcard_available() {
            break true;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break false;
        }

        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let timeout_ms = remaining.as_millis().min(1000) as i32;
        // SAFETY: pfd is a valid pollfd struct; fd is a valid open inotify descriptor.
        let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() != Some(libc::EINTR) {
                warn!("poll error in sdcard wait: {err}");
            }
            continue;
        }

        if ret > 0 && (pfd.revents & libc::POLLIN) != 0 {
            let mut buf = [0u8; 4096];
            // SAFETY: fd is a valid open inotify descriptor; buf is a stack-allocated array.
            unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()); }
        }
    };

    // SAFETY: fd is a valid open file descriptor from inotify_init1 above.
    unsafe { libc::close(fd); }

    if result {
        tracing::info!("sdcard decrypted, proceeding with SUSFS retry");
    }
    result
}

fn poll_for_sdcard(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if crate::utils::signal::shutdown_requested() {
            return false;
        }
        if sdcard_available() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

pub fn handle_diag() -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    println!("zeromount v{version} diagnostic dump");

    // Root manager
    match crate::utils::platform::detect_root_manager() {
        Ok(mgr) => {
            println!("root_manager: {}", mgr.name());
            println!("base_dir: {}", mgr.base_dir().display());
            println!("busybox: {}", mgr.busybox_path().display());
        }
        Err(e) => println!("root_manager: not detected ({e})"),
    }

    // VFS driver probe
    match crate::vfs::VfsDriver::open() {
        Ok(driver) => {
            match driver.get_version() {
                Ok(v) => println!("vfs driver: v{v}"),
                Err(e) => println!("vfs driver: open but GET_VERSION failed ({e})"),
            }
            match driver.get_status() {
                Ok(Some(s)) => println!(
                    "vfs engine: {} ({} rules)",
                    if s.enabled { "active" } else { "inactive" },
                    s.rule_count
                ),
                Ok(None) => println!("vfs engine: GET_STATUS not supported"),
                Err(e) => println!("vfs engine: query failed ({e})"),
            }
        }
        Err(e) => println!("vfs driver: not available ({e})"),
    }

    // SUSFS probe
    match crate::susfs::SusfsClient::probe() {
        Ok(client) => {
            if client.is_available() {
                println!("susfs: {} (features: {:?})", client.version().unwrap_or("unknown"), client.features());
            } else {
                println!("susfs: not available");
            }
        }
        Err(e) => println!("susfs: probe failed ({e})"),
    }

    // Detection result
    match crate::detect::load_detection() {
        Ok(det) => println!("scenario: {:?} (from last detect)", det.scenario),
        Err(_) => println!("scenario: unknown (run `zeromount detect` first)"),
    }

    // Config
    let config = crate::core::config::ZeroMountConfig::load(None).unwrap_or_default();
    println!("storage_mode: {:?}", config.mount.storage_mode);
    println!("uname_mode: {:?}", config.uname.mode);

    Ok(())
}

pub fn handle_cleanup_stale() -> Result<()> {
    tracing::info!("stale overlay cleanup started");
    match crate::mount::cleanup::cleanup_stale_overlays() {
        Ok(n) => {
            tracing::info!(cleaned = n, "stale overlay cleanup complete");
            Ok(())
        }
        Err(e) => {
            tracing::warn!(error = %e, "stale overlay cleanup failed");
            Ok(())
        }
    }
}
