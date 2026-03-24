use std::path::Path;

use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::{BridgeAction, ConfigAction, EmojiAction, LogAction, ModuleAction, UidAction, VfsAction};

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
        LogAction::Dump => crate::logging::dump::execute_dump(),
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
        println!("  kstat_redirect: {}", result.capabilities.susfs_kstat_redirect);
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

            let config = crate::core::config::ZeroMountConfig::load(None)
                .unwrap_or_default();
            let scan_opts = crate::modules::scanner::ScanOptions {
                exclude_hosts: config.mount.exclude_hosts_modules,
                blacklist: &config.mount.module_blacklist,
            };
            let modules = crate::modules::scanner::scan_modules(modules_dir, &scan_opts)?;
            println!("scan complete: {} modules", modules.len());
            for m in &modules {
                println!("  {} ({} files)", m.id, m.files.len());
            }

            if update_conf {
                tracing::debug!("partitions.conf rebuild requested");
            }
        }
        ModuleAction::Unload { id } => {
            let status_path = Path::new("/data/adb/zeromount/.status.json");
            let mut state = crate::core::types::RuntimeState::read_status_file(status_path)
                .unwrap_or_default();

            let module = state.modules.iter().find(|m| m.id == id);
            let Some(module) = module else {
                println!("{{\"error\":\"module_not_found\",\"id\":\"{id}\"}}");
                return Ok(());
            };

            let strategy = module.strategy;
            let mount_paths = module.mount_paths.clone();
            let mut removed = 0u32;

            match strategy {
                crate::core::types::MountStrategy::Vfs => {
                    if let Ok(driver) = crate::vfs::VfsDriver::open() {
                        let module_prefix = format!("/data/adb/modules/{id}/");
                        if let Ok(list) = driver.get_list() {
                            for line in list.lines() {
                                if let Some(idx) = line.find("->") {
                                    let source = line[..idx].trim();
                                    let target = line[idx + 2..].trim();
                                    if source.starts_with(&module_prefix) {
                                        let vp = Path::new(target);
                                        if driver.del_rule(vp, vp).is_ok() {
                                            removed += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                crate::core::types::MountStrategy::Overlay
                | crate::core::types::MountStrategy::MagicMount => {
                    for path in &mount_paths {
                        if crate::core::pipeline::try_detach_mount(path) {
                            removed += 1;
                        }
                    }
                }
                crate::core::types::MountStrategy::Font => {
                    crate::core::pipeline::try_detach_mount("/system/fonts");
                    removed = 1;
                }
            }

            state.modules.retain(|m| m.id != id);
            state.font_modules.retain(|f| f != &id);
            let _ = state.write_status_file(status_path);

            println!(
                "{{\"removed\":{removed},\"strategy\":\"{strategy:?}\"}}",
            );
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
        "enabled" => "susfs.enabled",
        "log" => "brene.susfs_log",
        _ => anyhow::bail!("unknown SUSFS feature: {feature} (try: kstat, path, maps, enabled, log)"),
    };

    config.set(key, state)?;
    config.save()?;
    println!("ok");
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

pub fn handle_prop_watch() -> Result<()> {
    crate::prop::run_prop_watch()
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

pub fn handle_vold_app_data() -> Result<()> {
    let config = crate::core::config::ZeroMountConfig::load(None)?;
    if !config.brene.emulate_vold_app_data {
        tracing::info!("vold_app_data: disabled in config, skipping");
        return Ok(());
    }
    let Ok(client) = crate::susfs::SusfsClient::probe() else {
        tracing::warn!("vold_app_data: SUSFS unavailable");
        return Ok(());
    };
    // Root paths must be set before add_sus_path works (susfs v1.5.8+)
    if let Err(e) = client.set_sdcard_root_path("/sdcard") {
        tracing::warn!("vold_app_data: set_sdcard_root_path failed: {e}");
    }
    if let Err(e) = client.set_android_data_root_path("/sdcard/Android/data") {
        tracing::warn!("vold_app_data: set_android_data_root_path failed: {e}");
    }
    let count = crate::susfs::brene::emulate_vold_app_data(&client);
    tracing::info!("vold_app_data: {count} paths hidden");
    Ok(())
}

pub fn handle_try_umount() -> Result<()> {
    let config = crate::core::config::ZeroMountConfig::load(None)?;
    if !config.brene.try_umount {
        tracing::info!("try_umount: disabled in config, skipping");
        return Ok(());
    }
    let Ok(client) = crate::susfs::SusfsClient::probe() else {
        tracing::warn!("try_umount: SUSFS unavailable");
        return Ok(());
    };
    let count = crate::susfs::brene::try_umount_ksu_mounts(&client, config.brene.hide_sus_mounts);
    tracing::info!("try_umount: {count} paths registered");
    Ok(())
}

const SENTINEL_PATH: &str = "/data/adb/zeromount/flags/external_susfs";

fn read_sentinel_module() -> crate::core::types::ExternalSusfsModule {
    use crate::core::types::ExternalSusfsModule;

    std::fs::read_to_string(SENTINEL_PATH)
        .ok()
        .and_then(|s| match s.trim() {
            "susfs4ksu" => Some(ExternalSusfsModule::Susfs4ksu),
            "brene" => Some(ExternalSusfsModule::Brene),
            _ => None,
        })
        .unwrap_or(ExternalSusfsModule::None)
}

fn parse_module_id(id: &str) -> Result<crate::core::types::ExternalSusfsModule> {
    use crate::core::types::ExternalSusfsModule;

    match id {
        "susfs4ksu" => Ok(ExternalSusfsModule::Susfs4ksu),
        "brene" => Ok(ExternalSusfsModule::Brene),
        "none" => Ok(ExternalSusfsModule::None),
        _ => anyhow::bail!("unknown module id: {id} (expected: susfs4ksu, brene, none)"),
    }
}

pub fn handle_bridge(action: BridgeAction) -> Result<()> {
    match action {
        BridgeAction::Init => {
            let config = crate::core::config::ZeroMountConfig::load(None)?;
            crate::bridge::init_external_configs(&config)?;
            tracing::info!("bridge init complete");
            println!("ok");
        }
        BridgeAction::Write => {
            let config = crate::core::config::ZeroMountConfig::load(None)?;
            let module = read_sentinel_module();
            crate::bridge::write_to_external(&config, module)?;
            tracing::info!(module = ?module, "bridge write complete");
            println!("ok");
        }
        BridgeAction::Reconcile { module_id } => {
            let module = parse_module_id(&module_id)?;
            let mut config = crate::core::config::ZeroMountConfig::load(None)?;
            let changed = crate::bridge::reconcile_from_external(module, &mut config)?;
            if changed {
                config.save()?;
                tracing::info!(module = ?module, "bridge reconcile saved config changes");
            }
            println!("{}", if changed { "changed" } else { "unchanged" });
        }
    }
    Ok(())
}

pub fn handle_sync_description() -> Result<()> {
    let config = crate::core::config::ZeroMountConfig::load(None)?;
    let ds = crate::core::desc_strings::desc_strings(&config.ui.language);

    let module_ids = match crate::vfs::VfsDriver::open() {
        Ok(driver) => {
            let list = driver.get_list().unwrap_or_default();
            extract_module_ids_from_vfs(&list)
        }
        Err(_) => Vec::new(),
    };

    let desc = if module_ids.is_empty() {
        format!("😴 {} | Mountless VFS-level Redirection. GHOST👻", ds.idle)
    } else {
        let label = if module_ids.len() == 1 { ds.module_singular } else { ds.module_plural };
        format!("✅ GHOST ⚡️ | {} {} | {}", module_ids.len(), label, module_ids.join(", "))
    };

    crate::utils::platform::write_description_to_module_prop(&desc)?;

    // Live KSU/APatch Manager update via ksud override
    let ksud = if Path::new("/data/adb/ksu/bin/ksud").exists() {
        "/data/adb/ksu/bin/ksud"
    } else if Path::new("/data/adb/ap/bin/ksud").exists() {
        "/data/adb/ap/bin/ksud"
    } else {
        "ksud"
    };

    let _ = std::process::Command::new(ksud)
        .args(["module", "config", "set", "override.description", &desc])
        .env("KSU_MODULE", "meta-zeromount")
        .output();

    println!("{desc}");
    Ok(())
}

fn extract_module_ids_from_vfs(list: &str) -> Vec<String> {
    use std::collections::BTreeSet;
    let prefix = "/data/adb/modules/";
    let mut ids = BTreeSet::new();
    for line in list.lines() {
        for part in line.split("->") {
            let trimmed = part.trim();
            if let Some(pos) = trimmed.find(prefix) {
                let after = &trimmed[pos + prefix.len()..];
                if let Some(slash) = after.find('/') {
                    let id = &after[..slash];
                    if !id.is_empty() && id != "meta-zeromount" {
                        ids.insert(id.to_string());
                    }
                }
            }
        }
    }
    ids.into_iter().collect()
}

pub fn handle_emoji(action: EmojiAction) -> Result<()> {
    match action {
        EmojiAction::ApplyApps => {
            let config = crate::core::config::ZeroMountConfig::load(None)?;
            if !config.emoji.enabled {
                tracing::info!("emoji: disabled in config, skipping app overrides");
                return Ok(());
            }
            let result = crate::susfs::emoji::apply_emoji_app_overrides();
            tracing::info!(
                "emoji app overrides: fb={}/{}, gboard={}, gms={}",
                result.fb_succeeded, result.fb_total, result.gboard_ok, result.gms_ok
            );
            Ok(())
        }
    }
}
