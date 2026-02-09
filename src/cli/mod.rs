pub mod handlers;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zeromount", version = "2.0.0", about = "KernelSU/APatch metamodule mount engine")]
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
    Mount {
        /// Run post-boot tasks instead of mount pipeline (called by service.sh)
        #[arg(long)]
        post_boot: bool,
    },
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
    /// SUSFS feature toggles
    Susfs {
        feature: String,
        state: String,
    },
    /// Diagnostic dump
    Diag,
    /// Print version
    Version,
}

#[derive(Subcommand)]
pub enum LogAction {
    /// Enable kernel debug logging (sysfs=1, .verbose=touch)
    Enable,
    /// Disable kernel debug logging (sysfs=0, .verbose=remove)
    Disable,
    /// Set kernel debug level (0=off, 1=standard, 2=verbose)
    Level { level: u32 },
    /// Show current kernel debug level and .verbose state
    Status,
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
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Read a config value
    Get { key: String },
    /// Write a config value
    Set { key: String, value: String },
    /// Restore config from backup (bootloop recovery)
    Restore,
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
pub enum UidAction {
    /// Exclude UID from redirection
    Block { uid: u32 },
    /// Include UID in redirection
    Unblock { uid: u32 },
}
