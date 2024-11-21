use std::collections::hash_map::Entry::{Occupied, Vacant};

use anyhow::{anyhow, Result};
use tracing::warn;

use crate::config::ModuleFederationConfig;
use crate::plugin::Plugin;

pub struct ModuleFederationPlugin {
    pub config: ModuleFederationConfig,
}

impl ModuleFederationPlugin {
    pub fn new(config: ModuleFederationConfig) -> Self {
        Self { config }
    }
}

impl Plugin for ModuleFederationPlugin {
    fn name(&self) -> &str {
        "module_federation"
    }

    fn modify_config(
        &self,
        config: &mut crate::config::Config,
        root: &std::path::Path,
        _args: &crate::compiler::Args,
    ) -> Result<()> {
        if let Some(exposes) = self.config.exposes.as_ref() {
            for (name, import) in exposes.iter() {
                match config.entry.entry(name.to_string()) {
                    Occupied(_) => {
                        warn!("mf exposed name {} is duplcated with entry.", name);
                    }
                    Vacant(vacant_entry) => {
                        if let Ok(entry_path) = root.join(import).canonicalize() {
                            vacant_entry.insert(entry_path);
                        } else {
                            return Err(anyhow!("mf exposed file :{} not found", import));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
