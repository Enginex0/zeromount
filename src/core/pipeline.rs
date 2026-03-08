use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use super::config::ZeroMountConfig;
use super::types::{
    CapabilityFlags, DetectionResult, ModuleStatus, MountPlan, MountResult, MountStrategy,
    RootManager, RuntimeState, ScannedModule, Scenario,
};


const MODULES_DIR: &str = "/data/adb/modules";
const STATUS_JSON_PATH: &str = "/data/adb/zeromount/.status.json";

pub struct MountController<S> {
    state: S,
}

// -- Typestate structs with embedded data --

pub struct Init {
    config: ZeroMountConfig,
    root_mgr: Box<dyn RootManager>,
}

pub struct Detected {
    config: ZeroMountConfig,
    root_mgr: Box<dyn RootManager>,
    detection: DetectionResult,
}

pub struct Planned {
    config: ZeroMountConfig,
    root_mgr: Box<dyn RootManager>,
    detection: DetectionResult,
    plan: MountPlan,
    modules: Vec<ScannedModule>,
}

pub struct Mounted {
    config: ZeroMountConfig,
    root_mgr: Box<dyn RootManager>,
    detection: DetectionResult,
    results: Vec<MountResult>,
    overlay_source: Option<String>,
}

pub struct Finalized {
    pub state: RuntimeState,
}

// -- Init --

impl MountController<Init> {
    pub fn new(config: ZeroMountConfig) -> Result<Self> {
        let root_mgr = crate::utils::platform::detect_root_manager()
            .context("root manager detection failed")?;
        Ok(MountController {
            state: Init { config, root_mgr },
        })
    }

    /// Probe kernel capabilities and determine scenario.
    /// Persists detection result to disk for later phases.
    pub fn detect(self) -> Result<MountController<Detected>> {
        info!("pipeline: detect phase");

        let result = crate::detect::detect_and_persist()
            .context("detection phase failed")?;

        info!(
            scenario = ?result.scenario,
            driver_version = ?result.driver_version,
            "detection complete"
        );

        Ok(MountController {
            state: Detected {
                config: self.state.config,
                root_mgr: self.state.root_mgr,
                detection: result,
            },
        })
    }
}

// -- Detected -> Planned (scan + plan in one transition) --

#[allow(dead_code)] // Typestate API surface for pipeline consumers
impl MountController<Detected> {
    pub fn detection(&self) -> &DetectionResult {
        &self.state.detection
    }

    pub fn scenario(&self) -> Scenario {
        self.state.detection.scenario
    }

    /// Scan modules directory and build mount plan in a single transition.
    /// Merges two logical operations because plan depends entirely on scan output.
    pub fn scan_and_plan(self) -> Result<MountController<Planned>> {
        info!("pipeline: scan + plan phase");

        // Scan
        let modules_dir = Path::new(MODULES_DIR);
        let modules = if modules_dir.exists() {
            crate::modules::scanner::scan_modules(modules_dir)
                .context("module scan failed")?
        } else {
            warn!("modules directory missing: {MODULES_DIR}");
            Vec::new()
        };

        info!(count = modules.len(), "modules scanned");

        // Plan — pass user strategy override so planner skips BFS when not needed
        let plan = crate::mount::planner::plan_mounts(
            &modules,
            self.state.detection.scenario,
            &self.state.detection.capabilities,
            self.state.config.user_strategy_override(),
        )
        .context("mount planning failed")?;

        info!(
            modules = plan.modules.len(),
            partitions = plan.partition_mounts.len(),
            "mount plan built"
        );

        Ok(MountController {
            state: Planned {
                config: self.state.config,
                root_mgr: self.state.root_mgr,
                detection: self.state.detection,
                plan,
                modules,
            },
        })
    }

    /// Consume into detection result without continuing the pipeline.
    pub fn into_detection(self) -> DetectionResult {
        self.state.detection
    }
}

// -- Planned -> Mounted --

#[allow(dead_code)] // Typestate API surface for pipeline consumers
impl MountController<Planned> {
    pub fn plan(&self) -> &MountPlan {
        &self.state.plan
    }

    pub fn modules(&self) -> &[ScannedModule] {
        &self.state.modules
    }

    /// Execute mount operations: dispatch to VFS, overlay, or magic mount.
    ///
    /// Strategy selection:
    ///   Full / SusfsFrontend / KernelOnly -> VFS default, user can override
    ///   SusfsOnly / None                  -> overlay default, user can force magic
    pub fn execute(self) -> Result<MountController<Mounted>> {
        info!("pipeline: execute phase");

        cleanup_previous_mounts();

        ZeroMountConfig::backup().unwrap_or_else(|e| {
            warn!("config backup failed (non-fatal): {e}");
        });

        let resolved_overlay_source = crate::mount::storage::resolve_overlay_source(
            &self.state.config.mount.overlay_source,
        );

        let scenario = self.state.detection.scenario;
        let config = &self.state.config;
        let user_override = config.user_strategy_override();

        // Snapshot stock OEM overlays BEFORE we create our own mounts.
        let results = match scenario {
            // VFS-capable kernels: default VFS, user can override to overlay/magic
            Scenario::Full | Scenario::SusfsFrontend | Scenario::KernelOnly => {
                match user_override {
                    Some(MountStrategy::Overlay) => {
                        info!("user override: overlay on VFS-capable kernel");
                        crate::mount::executor::manage_skip_mount_flags(
                            &self.state.modules,
                            self.state.root_mgr.mount_mode(),
                        );
                        let mut results = self.execute_overlay_or_magic(
                            &self.state.modules, &self.state.plan, config,
                        )?;

                        // VFS fallback for novel directories that overlay can't mount
                        if results.iter().any(|r| !r.success) {
                            if let Ok(driver) = crate::vfs::VfsDriver::open() {
                                let fail = results.iter().filter(|r| !r.success).count();
                                info!(fail, "VFS fallback for failed overlay mounts");
                                let executor = crate::vfs::VfsExecutor::new(driver);
                                if let Ok(mut vfs) = executor.execute(&self.state.plan, &self.state.modules) {
                                    results.append(&mut vfs);
                                }
                            }
                        }

                        Ok(results)
                    }
                    Some(MountStrategy::MagicMount) => {
                        info!("user override: magic mount on VFS-capable kernel");
                        crate::mount::executor::manage_skip_mount_flags(
                            &self.state.modules,
                            self.state.root_mgr.mount_mode(),
                        );
                        self.execute_magic(&self.state.modules, &self.state.plan, config)
                    }
                    _ => {
                        let capabilities = &self.state.detection.capabilities;
                        self.execute_vfs(&self.state.modules, &self.state.plan, capabilities, config)
                    }
                }
            }
            // No VFS driver: overlay or magic mount based on user config
            Scenario::SusfsOnly | Scenario::None => {
                crate::mount::executor::manage_skip_mount_flags(
                    &self.state.modules,
                    self.state.root_mgr.mount_mode(),
                );
                match user_override {
                    Some(MountStrategy::MagicMount) => {
                        info!("user preference: magic mount (no VFS available)");
                        self.execute_magic(&self.state.modules, &self.state.plan, config)
                    }
                    _ => {
                        self.execute_overlay_or_magic(&self.state.modules, &self.state.plan, config)
                    }
                }
            }
        }
        .context("mount execution failed")?;

        let succeeded = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        info!(succeeded, failed, "execution complete");

        Ok(MountController {
            state: Mounted {
                config: self.state.config,
                root_mgr: self.state.root_mgr,
                detection: self.state.detection,
                results,
                overlay_source: Some(resolved_overlay_source),
            },
        })
    }

    // VFS driver path: inject rules, SUSFS protections, enable engine
    fn execute_vfs(
        &self,
        modules: &[ScannedModule],
        plan: &MountPlan,
        _capabilities: &CapabilityFlags,
        config: &ZeroMountConfig,
    ) -> Result<Vec<MountResult>> {
        info!("executing via VFS driver");

        // Open driver
        let driver = match crate::vfs::VfsDriver::open() {
            Ok(d) => d,
            Err(e) => {
                warn!("VFS driver open failed, falling back: {e}");
                return self.execute_overlay_or_magic(modules, plan, config);
            }
        };

        let executor = crate::vfs::VfsExecutor::new(driver);
        executor.execute(plan, modules)
    }

    // Overlay path with magic mount fallback
    fn execute_overlay_or_magic(
        &self,
        modules: &[ScannedModule],
        plan: &MountPlan,
        config: &ZeroMountConfig,
    ) -> Result<Vec<MountResult>> {
        let caps = &self.state.detection.capabilities;
        if config.mount.overlay_preferred {
            info!("executing via overlay (preferred)");
            let overlay_result = crate::mount::executor::execute_plan(
                plan, modules, MountStrategy::Overlay, caps, &config.mount,
            );

            match overlay_result {
                Ok(ref results)
                    if !results.is_empty() && results.iter().all(|r| r.success) =>
                {
                    return overlay_result;
                }
                Ok(ref results) if results.iter().any(|r| r.success) => {
                    let ok = results.iter().filter(|r| r.success).count();
                    let fail = results.iter().filter(|r| !r.success).count();
                    warn!(ok, fail, "partial overlay success -- failed mounts left uncovered");
                    return overlay_result;
                }
                Ok(_) if !config.mount.magic_mount_fallback => {
                    return overlay_result;
                }
                Ok(_) => {
                    warn!("overlay produced no successful mounts, falling back to magic mount");
                }
                Err(e) if config.mount.magic_mount_fallback => {
                    warn!(error = %e, "overlay execution failed, falling back to magic mount");
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        self.execute_magic(modules, plan, config)
    }

    fn execute_magic(
        &self,
        modules: &[ScannedModule],
        plan: &MountPlan,
        config: &ZeroMountConfig,
    ) -> Result<Vec<MountResult>> {
        let caps = &self.state.detection.capabilities;
        crate::mount::executor::execute_plan(
            plan, modules, MountStrategy::MagicMount, caps, &config.mount,
        )
    }
}

// -- Mounted -> Finalized --

#[allow(dead_code)] // Typestate API surface for pipeline consumers
impl MountController<Mounted> {
    pub fn results(&self) -> &[MountResult] {
        &self.state.results
    }

    pub fn sweep_rogue_mounts(self) -> Self {
        let summary = crate::mount::hijack::sweep(
            self.state.detection.scenario,
            &self.state.detection.capabilities,
            &self.state.config.susfs,
            &self.state.results,
        );
        info!(
            "sweep: {} found, {} hijacked, {} skipped",
            summary.found, summary.hijacked, summary.skipped
        );
        self
    }

    /// Apply SUSFS protections, notify root manager, write status JSON.
    ///
    /// KSU09 ordering: BRENE -> description update -> status JSON -> notify-module-mounted (LAST).
    /// notify-module-mounted signals that all mounts are stable and ready for app launch.
    pub fn finalize(self) -> Result<MountController<Finalized>> {
        info!("pipeline: finalize phase");

        // 1. SUSFS protections (BRENE)
        let (hidden_paths, hidden_maps, font_infos, emoji_applied) = self.apply_susfs_protections();

        // 2. Update module description with status summary
        let summary = self.build_description_summary(&font_infos);
        if let Err(e) = self.state.root_mgr.update_description(&summary) {
            debug!("update_description failed (non-fatal): {e}");
        }

        // 3. Build RuntimeState and persist atomically (ME07: write tmp then rename)
        let mut runtime_state = self.build_runtime_state(&font_infos);
        runtime_state.hidden_path_count = hidden_paths;
        runtime_state.hidden_maps_count = hidden_maps;
        runtime_state.emoji_applied = emoji_applied;
        write_status_json_atomic(&runtime_state);

        // KSU09: notify-module-mounted is fired by metamount.sh after the binary
        // exits — removed from Rust to avoid double-signaling.

        info!("pipeline complete");

        Ok(MountController {
            state: Finalized {
                state: runtime_state,
            },
        })
    }

    fn apply_susfs_protections(&self) -> (u32, u32, Vec<crate::susfs::brene::FontModuleInfo>, bool) {
        if !self.state.config.susfs.enabled {
            debug!("SUSFS disabled in config, skipping protections");
            return (0, 0, Vec::new(), false);
        }

        let external_module = self.state.detection.capabilities.external_susfs_module;

        match crate::susfs::SusfsClient::probe() {
            Ok(client) => {
                let fonts_overlay_mounted = self.state.results.iter().any(|r| {
                    r.strategy_used == MountStrategy::Vfs
                    || r.strategy_used == MountStrategy::Font
                    || (r.success && r.mount_paths.iter().any(|p| p.contains("/system/fonts")))
                });

                let susfs_mode = self.state.detection.capabilities.susfs_mode;
                match crate::susfs::brene::apply_brene(&client, &self.state.config, fonts_overlay_mounted, susfs_mode, external_module) {
                    Ok(brene) => {
                        debug!(
                            paths = brene.paths_hidden,
                            maps = brene.maps_hidden,
                            fonts = brene.font_modules.len(),
                            emoji = brene.emoji_applied,
                            "BRENE applied"
                        );
                        (brene.paths_hidden, brene.maps_hidden, brene.font_modules, brene.emoji_applied)
                    }
                    Err(e) => {
                        warn!("BRENE application failed (non-fatal): {e}");
                        (0, 0, Vec::new(), false)
                    }
                }
            }
            Err(e) => {
                debug!("SUSFS probe failed, skipping protections: {e}");
                (0, 0, Vec::new(), false)
            }
        }
    }

    fn build_description_summary(&self, font_infos: &[crate::susfs::brene::FontModuleInfo]) -> String {
        // Overlay produces one MountResult per mount point — deduplicate module IDs.
        // Split on '+' handles multi-module mount points like "viperfxmod+clean".
        let mounted: std::collections::BTreeSet<&str> = self
            .state
            .results
            .iter()
            .filter(|r| r.success)
            .flat_map(|r| r.module_id.split('+'))
            .collect();

        // Font IDs from BRENE + executor (survives even if BRENE fails)
        let mut font_ids: std::collections::BTreeSet<&str> =
            font_infos.iter().map(|f| f.id.as_str()).collect();
        for r in &self.state.results {
            if r.strategy_used == MountStrategy::Font {
                font_ids.insert(&r.module_id);
            }
        }

        let vfs_only: Vec<&str> = mounted.iter()
            .filter(|id| !font_ids.contains(*id))
            .copied()
            .collect();

        let ds = crate::core::desc_strings::desc_strings(&self.state.config.ui.language);

        if vfs_only.is_empty() && font_ids.is_empty() {
            return format!("😴 {} | Mountless VFS-level Redirection. GHOST👻", ds.idle);
        }

        let total = vfs_only.len() + font_ids.len();
        let label = if total == 1 { ds.module_singular } else { ds.module_plural };
        let mut parts = Vec::new();

        if !vfs_only.is_empty() {
            parts.push(vfs_only.join(", "));
        }

        if !font_ids.is_empty() {
            let names: Vec<&str> = font_ids.iter().copied().collect();
            parts.push(format!("{}: {}", ds.font_prefix, names.join(", ")));
        }

        format!("✅ GHOST ⚡️ | {} {} | {}", total, label, parts.join(" | "))
    }

    fn build_runtime_state(&self, font_infos: &[crate::susfs::brene::FontModuleInfo]) -> RuntimeState {
        let det = &self.state.detection;
        let total_rules: u32 = self.state.results.iter().map(|r| r.rules_applied).sum();
        let total_failed: u32 = self.state.results.iter().map(|r| r.rules_failed).sum();

        let font_ids: Vec<&str> = font_infos.iter().map(|f| f.id.as_str()).collect();

        // Overlay produces one MountResult per mount point — merge by module ID
        // so viperfxmod with 5 mount points becomes a single entry.
        let mut merged: std::collections::BTreeMap<String, ModuleStatus> = std::collections::BTreeMap::new();
        for r in &self.state.results {
            if font_ids.contains(&r.module_id.as_str()) && r.rules_applied == 0 {
                continue;
            }
            let entry = merged.entry(r.module_id.clone()).or_insert_with(|| ModuleStatus {
                id: r.module_id.clone(),
                strategy: r.strategy_used,
                rules_applied: 0,
                rules_failed: 0,
                errors: Vec::new(),
                mount_paths: Vec::new(),
            });
            entry.rules_applied += r.rules_applied;
            entry.rules_failed += r.rules_failed;
            entry.mount_paths.extend(r.mount_paths.clone());
            entry.errors.extend(r.error.iter().cloned());
        }
        let mut modules: Vec<ModuleStatus> = merged.into_values().collect();

        // Font modules as first-class entries
        for fi in font_infos {
            // If module already in list (has VFS rules too), skip — it's a hybrid
            if modules.iter().any(|m| m.id == fi.id) {
                continue;
            }
            modules.push(ModuleStatus {
                id: fi.id.clone(),
                strategy: MountStrategy::Font,
                rules_applied: fi.redirect_count,
                rules_failed: 0,
                errors: Vec::new(),
                mount_paths: vec!["/system/fonts".to_string()],
            });
        }

        let active_strategy = modules
            .iter()
            .find(|m| m.strategy != MountStrategy::Font)
            .map(|m| m.strategy);

        let no_vfs = matches!(det.scenario, Scenario::None | Scenario::SusfsOnly);
        let degraded = total_failed > 0 || no_vfs;
        let degradation_reason = if det.scenario == Scenario::None {
            info!("degraded: no VFS driver, no SUSFS");
            Some("no VFS driver detected".to_string())
        } else if det.scenario == Scenario::SusfsOnly {
            info!(
                susfs_version = ?det.capabilities.susfs_version,
                "degraded: no VFS driver, SUSFS protections active"
            );
            Some("no VFS driver, SUSFS protections active".to_string())
        } else if total_failed > 0 {
            warn!(total_failed, "degraded: rules failed to apply");
            Some(format!("{total_failed} rules failed to apply"))
        } else {
            None
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        RuntimeState {
            scenario: det.scenario,
            capabilities: det.capabilities.clone(),
            engine_active: Some(!matches!(det.scenario, Scenario::None)),
            driver_version: det.driver_version,
            rule_count: total_rules,
            excluded_uid_count: 0,
            hidden_path_count: 0,
            hidden_maps_count: 0,
            susfs_version: det.capabilities.susfs_version.clone(),
            active_strategy,
            mount_source: match active_strategy {
                Some(MountStrategy::Vfs) => Some("VFS".into()),
                Some(MountStrategy::MagicMount) => Some("KSU".into()),
                Some(MountStrategy::Overlay) => self.state.overlay_source.clone(),
                _ if det.capabilities.vfs_driver => Some("VFS".into()),
                _ => self.state.overlay_source.clone(),
            },
            modules,
            font_modules: font_infos.iter().map(|f| f.id.clone()).collect(),
            timestamp,
            degraded,
            degradation_reason,
            root_manager: Some(self.state.root_mgr.name().to_string()),
            resolved_storage_mode: crate::mount::storage::get_resolved_storage_mode(),
            emoji_applied: false,
        }
    }
}

/// Tear down mounts from a previous pipeline run before re-mounting.
/// Reads .status.json for module list; missing file means first boot (no-op).
/// VFS modules use CLEAR_ALL so they don't need individual umount.
fn cleanup_previous_mounts() {
    cleanup_font_mounts();

    let status_path = Path::new(STATUS_JSON_PATH);
    let prev_state = match RuntimeState::read_status_file(status_path) {
        Ok(s) => s,
        Err(_) => {
            debug!("no previous status file, skipping mount cleanup");
            return;
        }
    };

    for module in &prev_state.modules {
        match module.strategy {
            MountStrategy::Overlay | MountStrategy::MagicMount => {
                for path in &module.mount_paths {
                    try_detach_mount(path);
                }
            }
            MountStrategy::Vfs | MountStrategy::Font => {}
        }
    }

    info!(modules = prev_state.modules.len(), "previous mounts cleaned up");
}

pub(crate) fn try_detach_mount(path: &str) -> bool {
    let c_path = match std::ffi::CString::new(path.as_bytes()) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let ret = unsafe { libc::umount2(c_path.as_ptr(), libc::MNT_DETACH) };
    if ret == 0 {
        return true;
    }
    let errno = std::io::Error::last_os_error();
    if errno.raw_os_error() != Some(libc::EINVAL) {
        debug!(path = %path, error = %errno, "umount failed (may already be gone)");
    }
    false
}

// Only detach font mounts that WE created (overlay/tmpfs).
// KSU bind mounts (f2fs/ext4/erofs) are left untouched.
fn cleanup_font_mounts() {
    let mountinfo = match std::fs::read_to_string("/proc/self/mountinfo") {
        Ok(m) => m,
        Err(_) => return,
    };

    let mut our_font_mounts: Vec<String> = Vec::new();
    let mut has_our_overlay = false;

    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 5 {
            continue;
        }
        let mount_point = fields[4];
        if !mount_point.starts_with("/system/fonts") {
            continue;
        }

        let fs_type = fields.iter()
            .position(|&f| f == "-")
            .and_then(|i| fields.get(i + 1))
            .copied()
            .unwrap_or("");

        match fs_type {
            "overlay" | "tmpfs" => {
                if mount_point == "/system/fonts" {
                    has_our_overlay = true;
                } else {
                    our_font_mounts.push(mount_point.to_string());
                }
            }
            _ => {
                debug!(mount_point, fs_type, "preserving KSU font bind mount");
            }
        }
    }

    for path in our_font_mounts.iter().rev() {
        try_detach_mount(path);
    }

    if has_our_overlay {
        try_detach_mount("/system/fonts");
        debug!("cleaned up stale font overlay on /system/fonts");
    }

    if !our_font_mounts.is_empty() {
        info!(count = our_font_mounts.len(), "cleaned up stale font mounts");
    }
}


// ME07: Atomic write -- write to .tmp then rename to avoid partial reads
fn write_status_json_atomic(state: &RuntimeState) {
    let status_path = Path::new(STATUS_JSON_PATH);
    let tmp_path = status_path.with_extension("json.tmp");

    if let Some(parent) = status_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    match state.write_status_file(&tmp_path) {
        Ok(()) => {
            if let Err(e) = std::fs::rename(&tmp_path, status_path) {
                warn!("atomic rename failed, trying direct write: {e}");
                let _ = state.write_status_file(status_path);
            }
        }
        Err(e) => {
            warn!("failed to write status JSON: {e}");
        }
    }
}

// -- Finalized accessors --

#[allow(dead_code)] // Typestate API surface for pipeline consumers
impl MountController<Finalized> {
    pub fn state(&self) -> &RuntimeState {
        &self.state.state
    }

    pub fn into_state(self) -> RuntimeState {
        self.state.state
    }
}

// -- Convenience functions --

/// Run the entire mount pipeline: detect -> scan_and_plan -> execute -> finalize.
pub fn run_full_pipeline(config: ZeroMountConfig) -> Result<RuntimeState> {
    let state = MountController::new(config)?
        .detect()?
        .scan_and_plan()?
        .execute()?
        .sweep_rogue_mounts()
        .finalize()?
        .into_state();
    Ok(state)
}

/// Bootloop-aware pipeline entry point (ME15).
/// Checks bootcount before running; enters safe mode if threshold exceeded.
pub fn run_pipeline_with_bootloop_guard(config: ZeroMountConfig) -> Result<RuntimeState> {
    if ZeroMountConfig::check_bootloop()? {
        warn!("bootloop detected — safe mode (zero rules, no mounts)");

        let _ = crate::utils::platform::write_description_to_module_prop(
            "🛡\u{fe0f} Safe mode — mounts skipped due to repeated boot failures"
        );

        if let Ok(mgr) = crate::utils::platform::detect_root_manager() {
            if let Err(e) = mgr.notify_module_mounted() {
                warn!("notify-module-mounted failed in safe mode: {e}");
            }
        } else {
            debug!("root manager not detected in safe mode, skipping notification");
        }

        return Ok(RuntimeState {
            degraded: true,
            degradation_reason: Some("bootloop detected — safe mode active".into()),
            ..RuntimeState::default()
        });
    }

    ZeroMountConfig::increment_bootcount()?;

    let state = run_full_pipeline(config)?;

    // Pipeline succeeded — clear bootcount so a normal reboot won't
    // false-trigger the shell guard (which bails at count > 0).
    ZeroMountConfig::reset_bootcount().unwrap_or_else(|e| {
        warn!("bootcount reset failed (non-fatal): {e}");
    });

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::RootMountMode;

    // Minimal RootManager for tests -- all operations are no-ops.
    struct TestRootManager;

    impl RootManager for TestRootManager {
        fn name(&self) -> &str { "test" }
        fn base_dir(&self) -> &Path { Path::new("/tmp/test") }
        fn busybox_path(&self) -> std::path::PathBuf { "/tmp/busybox".into() }
        fn susfs_binary_paths(&self) -> Vec<std::path::PathBuf> { vec![] }
        fn update_description(&self, _text: &str) -> Result<()> { Ok(()) }
        fn notify_module_mounted(&self) -> Result<()> { Ok(()) }
        fn mount_mode(&self) -> RootMountMode { RootMountMode::Metamodule }
    }

    #[test]
    fn typestate_compile_check() {
        // Verify typestate chain compiles. Direct construction since new()
        // would fail without KSU/APatch on dev machines.
        let _ctrl = MountController {
            state: Init {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
            },
        };
    }

    #[test]
    fn runtime_state_degraded_on_none_scenario() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::None,
                    capabilities: CapabilityFlags::default(),
                    driver_version: None,
                    timestamp: 0,
                },
                results: Vec::new(),
                overlay_source: None,
            },
        };

        let state = ctrl.build_runtime_state(&[]);
        assert!(state.degraded);
        assert_eq!(
            state.degradation_reason.as_deref(),
            Some("no VFS driver detected")
        );
        assert_eq!(state.scenario, Scenario::None);
    }

    #[test]
    fn runtime_state_counts_rules() {
        let results = vec![
            MountResult {
                module_id: "mod_a".into(),
                strategy_used: MountStrategy::Vfs,
                success: true,
                rules_applied: 10,
                rules_failed: 0,
                error: None,
                mount_paths: vec!["/system/bin".into()],
            },
            MountResult {
                module_id: "mod_b".into(),
                strategy_used: MountStrategy::Overlay,
                success: true,
                rules_applied: 5,
                rules_failed: 2,
                error: Some("partial failure".into()),
                mount_paths: vec!["/vendor/lib64".into()],
            },
        ];

        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(1),
                    timestamp: 0,
                },
                results,
                overlay_source: None,
            },
        };

        let state = ctrl.build_runtime_state(&[]);
        assert_eq!(state.rule_count, 15);
        assert!(state.degraded);
        assert_eq!(state.modules.len(), 2);
        assert_eq!(state.driver_version, Some(1));
    }

    #[test]
    fn description_summary_format() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(1),
                    timestamp: 0,
                },
                results: vec![
                    MountResult {
                        module_id: "a".into(),
                        strategy_used: MountStrategy::Vfs,
                        success: true,
                        rules_applied: 1,
                        rules_failed: 0,
                        error: None,
                        mount_paths: vec![],
                    },
                    MountResult {
                        module_id: "b".into(),
                        strategy_used: MountStrategy::Vfs,
                        success: false,
                        rules_applied: 0,
                        rules_failed: 1,
                        error: Some("fail".into()),
                        mount_paths: vec![],
                    },
                ],
                overlay_source: None,
            },
        };

        let desc = ctrl.build_description_summary(&Vec::new());
        assert_eq!(desc, "✅ GHOST ⚡️ | 1 Module | a");
    }

    #[test]
    fn description_idle_when_no_modules() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(1),
                    timestamp: 0,
                },
                results: Vec::new(),
                overlay_source: None,
            },
        };

        let desc = ctrl.build_description_summary(&[]);
        assert!(desc.contains("Idle"));
        assert!(desc.contains("GHOST"));
        assert!(desc.contains("Mountless VFS-level Redirection"));
    }

    #[test]
    fn description_multiple_modules() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(1),
                    timestamp: 0,
                },
                results: vec![
                    MountResult {
                        module_id: "lsposed".into(),
                        strategy_used: MountStrategy::Vfs,
                        success: true,
                        rules_applied: 10,
                        rules_failed: 0,
                        error: None,
                        mount_paths: vec![],
                    },
                    MountResult {
                        module_id: "shamiko".into(),
                        strategy_used: MountStrategy::Vfs,
                        success: true,
                        rules_applied: 3,
                        rules_failed: 0,
                        error: None,
                        mount_paths: vec![],
                    },
                ],
                overlay_source: None,
            },
        };

        let desc = ctrl.build_description_summary(&[]);
        assert_eq!(desc, "✅ GHOST ⚡️ | 2 Modules | lsposed, shamiko");
    }

    #[test]
    fn description_includes_font_modules() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(1),
                    timestamp: 0,
                },
                results: vec![MountResult {
                    module_id: "clean".into(),
                    strategy_used: MountStrategy::Vfs,
                    success: true,
                    rules_applied: 2,
                    rules_failed: 0,
                    error: None,
                    mount_paths: vec![],
                }],
                overlay_source: None,
            },
        };

        let fonts = vec![crate::susfs::brene::FontModuleInfo { id: "viperfxmod".into(), redirect_count: 5 }];
        let desc = ctrl.build_description_summary(&fonts);
        assert_eq!(desc, "✅ GHOST ⚡️ | 2 Modules | clean | Font: viperfxmod");
    }

    #[test]
    fn clean_run_not_degraded() {
        let ctrl = MountController {
            state: Mounted {
                config: ZeroMountConfig::default(),
                root_mgr: Box::new(TestRootManager),
                detection: DetectionResult {
                    scenario: Scenario::Full,
                    capabilities: CapabilityFlags::default(),
                    driver_version: Some(2),
                    timestamp: 100,
                },
                results: vec![MountResult {
                    module_id: "clean".into(),
                    strategy_used: MountStrategy::Vfs,
                    success: true,
                    rules_applied: 5,
                    rules_failed: 0,
                    error: None,
                    mount_paths: vec!["/system/app".into()],
                }],
                overlay_source: None,
            },
        };

        let state = ctrl.build_runtime_state(&[]);
        assert!(!state.degraded);
        assert!(state.degradation_reason.is_none());
        assert_eq!(state.engine_active, Some(true));
        assert_eq!(state.rule_count, 5);
    }
}
