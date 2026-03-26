pub mod handlers;
pub mod webui_init;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zeromount", version = env!("CARGO_PKG_VERSION"), about = "KernelSU/APatch metamodule mount engine")]
pub struct Cli {
    /// Enable verbose logging (also triggered by .verbose file)
    #[arg(long, short, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Full mount pipeline (called by metamount.sh)
    Mount,
    /// Probe kernel capabilities, write detection JSON
    Detect,
    /// Engine state, modules, scenario
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Module operations
    Module {
        #[command(subcommand)]
        action: ModuleAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// VFS driver operations
    Vfs {
        #[command(subcommand)]
        action: VfsAction,
    },
    /// UID exclusion management
    Uid {
        #[command(subcommand)]
        action: UidAction,
    },
    /// Runtime logging control (kernel sysfs + .verbose marker)
    Log {
        #[command(subcommand)]
        action: LogAction,
    },
    /// External module config bridge
    Bridge {
        #[command(subcommand)]
        action: BridgeAction,
    },
    /// SUSFS feature toggles
    Susfs {
        feature: String,
        state: String,
    },
    /// Watch /data/adb/modules/ for changes (inotify with polling fallback)
    Watch,
    /// Performance tuning + input boost daemon (controlled by perf.enabled)
    Perf,
    /// One-shot property spoofing via resetprop
    #[command(name = "prop-watch")]
    PropWatch,
    /// Diagnostic dump
    Diag,
    /// Remove stale overlay mounts from previous runs
    CleanupStale,
    /// Batched WebUI init data (single JSON blob)
    #[command(name = "webui-init")]
    WebUiInit,
    /// Emoji app-level overrides (post-boot only — needs pm)
    Emoji {
        #[command(subcommand)]
        action: EmojiAction,
    },
    /// Vold app data emulation (post-boot only — needs pm)
    #[command(name = "vold-app-data")]
    VoldAppData,
    /// Auto-discover KSU mounts and register kernel umount paths (post-boot)
    #[command(name = "try-umount")]
    TryUmount,
    /// Device-wide bootloop guard
    Guard {
        #[command(subcommand)]
        action: GuardAction,
    },
    /// Sync module description to module.prop + KSU Manager (live update)
    #[command(name = "sync-description")]
    SyncDescription,
    /// Print version
    Version,
}

#[derive(Subcommand)]
pub enum EmojiAction {
    /// Apply app-level emoji overrides (Facebook, GBoard, GMS font provider)
    ApplyApps,
}

#[derive(Subcommand)]
pub enum LogAction {
    /// Enable kernel debug logging (sysfs=2, .verbose=touch)
    Enable,
    /// Disable kernel debug logging (sysfs=0, .verbose=remove)
    Disable,
    /// Set kernel debug level (0=off, 1=standard, 2=verbose)
    Level { level: u32 },
    /// Show current kernel debug level and .verbose state
    Status,
    /// Collect logs, dmesg, config, and diagnostics to /sdcard (stealth-named dir)
    Dump,
}

#[derive(Subcommand)]
pub enum ModuleAction {
    /// List modules with mount status
    List,
    /// Force rescan
    Scan {
        /// Rebuild partitions.conf after module install
        #[arg(long)]
        update_conf: bool,
        /// Clean VFS rules and SUSFS entries for an uninstalled module
        #[arg(long)]
        cleanup: Option<String>,
    },
    /// Hot-unload a module (unmount + remove VFS rules)
    Unload {
        /// Module ID (directory name under /data/adb/modules/)
        id: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Read a config value
    Get { key: String },
    /// Write a config value
    Set { key: String, value: String },
    /// Restore config from backup (bootloop recovery)
    Restore,
    /// Print compiled-in defaults as TOML (used by customize.sh)
    Defaults,
    /// Dump current config (TOML default, JSON with --json)
    Dump {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
pub enum VfsAction {
    /// Add VFS redirection rule
    Add {
        virtual_path: String,
        real_path: String,
    },
    /// Delete VFS rule
    Del { virtual_path: String },
    /// Clear all rules
    Clear,
    /// Enable VFS engine
    Enable,
    /// Disable VFS engine
    Disable,
    /// Flush dcache
    Refresh,
    /// List active rules
    List,
    /// Engine enabled state
    QueryStatus,
}

#[derive(Subcommand)]
pub enum BridgeAction {
    /// Write both external module configs from config.toml (install-time)
    Init,
    /// Write config.toml values to active external module's config.sh
    Write,
    /// Import changes from external module's config.sh into config.toml
    Reconcile {
        /// External module id: "susfs4ksu" or "brene"
        module_id: String,
    },
}

#[derive(Subcommand)]
pub enum UidAction {
    /// Exclude UID from redirection
    Block { uid: u32 },
    /// Include UID in redirection
    Unblock { uid: u32 },
}

#[derive(Subcommand)]
pub enum GuardAction {
    /// Check if guard has tripped (exit 1 if so)
    Check,
    /// Boot completion watchdog (blocks until done or timeout)
    #[command(name = "watch-boot")]
    WatchBoot,
    /// Zygote stability monitor (blocks for watch window)
    #[command(name = "watch-zygote")]
    WatchZygote,
    /// SystemUI stability monitor (runs continuously)
    #[command(name = "watch-systemui")]
    WatchSystemui,
    /// Clear recovery lockout
    #[command(name = "clear-lockout")]
    ClearLockout,
    /// Print guard status
    Status,
    /// Force self-disable + reboot
    Recover,
}
