use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::{ConfigAction, LogAction, ModuleAction, UidAction, VfsAction};

pub fn handle_mount(post_boot: bool) -> Result<()> {
    let _lock = match crate::utils::lock::acquire_instance_lock()? {
        Some(guard) => guard,
        None => {
            warn!("another zeromount instance is running, exiting");
            return Ok(());
        }
    };

    if post_boot {
        tracing::info!("post-boot tasks started");

        let config = crate::core::config::ZeroMountConfig::load(None)?;
        let state = crate::core::pipeline::run_pipeline_with_bootloop_guard(config)?;
        tracing::info!(
            scenario = ?state.scenario,
            rules = state.rule_count,
            modules = state.modules.len(),
            degraded = state.degraded,
            "pipeline finished"
        );

        return Ok(());
    }

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

pub fn handle_status(json: bool) -> Result<()> {
    let status_path = std::path::Path::new("/data/adb/zeromount/.status.json");
    let mut state = crate::core::types::RuntimeState::read_status_file(status_path)
        .unwrap_or_default();

    // Backfill from .detection.json when .status.json is missing or has bare defaults
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

    // Augment cached state with live kernel data when the driver is reachable
    if let Ok(driver) = crate::vfs::VfsDriver::open() {
        state.capabilities.vfs_driver = true;
        if let Ok(v) = driver.get_version() {
            state.driver_version = Some(v);
        }
        if let Ok(Some(s)) = driver.get_status() {
            state.engine_active = Some(s.enabled);
        }
    }

    // Detect root manager live rather than relying on cached state
    if let Ok(mgr) = crate::utils::platform::detect_root_manager() {
        state.root_manager = Some(mgr.name().to_string());
    }

    if json {
        let out = serde_json::to_string_pretty(&state)?;
        println!("{out}");
    } else {
        println!("scenario: {:?}", state.scenario);
        println!("engine_active: {:?}", state.engine_active);
        println!("modules: {}", state.modules.len());
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
            config.save(None)?;
            println!("ok");
        }
        ConfigAction::Restore => {
            let restored = crate::core::config::ZeroMountConfig::restore_backup()?;
            restored.save(None)?;
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
    config.save(None)?;
    println!("ok");
    Ok(())
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
