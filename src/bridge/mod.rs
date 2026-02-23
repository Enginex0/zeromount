pub mod brene;
pub mod susfs4ksu;
pub mod translate;

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::config::ZeroMountConfig;
use crate::core::types::ExternalSusfsModule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeValues {
    pub module: String,
    pub values: HashMap<String, String>,
}

// Install-time: write both external module configs from zeromount's config
pub fn init_external_configs(config: &ZeroMountConfig) -> Result<()> {
    init_module_config(config, Path::new(susfs4ksu::BASE_DIR), ModuleKind::Susfs4ksu)?;
    init_module_config(config, Path::new(brene::BASE_DIR), ModuleKind::Brene)?;
    Ok(())
}

// WebUI toggle: write to the active external module's config.sh
pub fn write_to_external(config: &ZeroMountConfig, module: ExternalSusfsModule) -> Result<()> {
    match module {
        ExternalSusfsModule::None => Ok(()),
        ExternalSusfsModule::Susfs4ksu => {
            let dir = Path::new(susfs4ksu::BASE_DIR);
            susfs4ksu::write_config(dir, config)
                .context("writing susfs4ksu config.sh")
        }
        ExternalSusfsModule::Brene => {
            let dir = Path::new(brene::BASE_DIR);
            brene::write_config(dir, config)
                .context("writing BRENE config.sh")
        }
    }
}

// Boot-time: read external config.sh, diff against config.toml, import changes
pub fn reconcile_from_external(
    module: ExternalSusfsModule,
    config: &mut ZeroMountConfig,
) -> Result<bool> {
    match module {
        ExternalSusfsModule::None => Ok(false),
        ExternalSusfsModule::Susfs4ksu => {
            let dir = Path::new(susfs4ksu::BASE_DIR);
            let keys = susfs4ksu::read_config(dir)
                .context("reading susfs4ksu config.sh for reconcile")?;
            let changed = susfs4ksu::apply_keys_to_config(&keys, config);
            if changed {
                tracing::info!("reconciled config changes from susfs4ksu");
            }
            Ok(changed)
        }
        ExternalSusfsModule::Brene => {
            let dir = Path::new(brene::BASE_DIR);
            let keys = brene::read_config(dir)
                .context("reading BRENE config.sh for reconcile")?;
            let changed = brene::apply_keys_to_config(&keys, config);
            if changed {
                tracing::info!("reconciled config changes from BRENE");
            }
            Ok(changed)
        }
    }
}

// WebUI init: read raw key-value pairs from external module for display
pub fn read_bridge_values(module: ExternalSusfsModule) -> Result<Option<BridgeValues>> {
    match module {
        ExternalSusfsModule::None => Ok(None),
        ExternalSusfsModule::Susfs4ksu => {
            let dir = Path::new(susfs4ksu::BASE_DIR);
            let values = susfs4ksu::read_config(dir)
                .context("reading susfs4ksu config.sh for bridge values")?;
            Ok(Some(BridgeValues {
                module: "susfs4ksu".to_string(),
                values,
            }))
        }
        ExternalSusfsModule::Brene => {
            let dir = Path::new(brene::BASE_DIR);
            let values = brene::read_config(dir)
                .context("reading BRENE config.sh for bridge values")?;
            Ok(Some(BridgeValues {
                module: "brene".to_string(),
                values,
            }))
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ModuleKind {
    Susfs4ksu,
    Brene,
}

fn init_module_config(config: &ZeroMountConfig, dir: &Path, kind: ModuleKind) -> Result<()> {
    std::fs::create_dir_all(dir)
        .with_context(|| format!("creating {}", dir.display()))?;

    let config_path = dir.join(match kind {
        ModuleKind::Susfs4ksu => susfs4ksu::CONFIG_FILE,
        ModuleKind::Brene => brene::CONFIG_FILE,
    });

    if config_path.exists() {
        // Merge: preserve user values per Section 7 algorithm
        let existing = match kind {
            ModuleKind::Susfs4ksu => susfs4ksu::read_config(dir)?,
            ModuleKind::Brene => brene::read_config(dir)?,
        };
        match kind {
            ModuleKind::Susfs4ksu => susfs4ksu::merge_config(dir, config, &existing)?,
            ModuleKind::Brene => brene::merge_config(dir, config, &existing)?,
        }
        tracing::debug!(dir = %dir.display(), "merged existing external config");
    } else {
        // Fresh: write zeromount defaults
        match kind {
            ModuleKind::Susfs4ksu => susfs4ksu::write_config(dir, config)?,
            ModuleKind::Brene => brene::write_config(dir, config)?,
        }
        tracing::debug!(dir = %dir.display(), "wrote fresh external config");
    }

    // Create empty txt files if not present — never overwrite user data
    match kind {
        ModuleKind::Susfs4ksu => susfs4ksu::ensure_txt_files(dir)?,
        ModuleKind::Brene => brene::ensure_txt_files(dir)?,
    }

    Ok(())
}
