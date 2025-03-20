use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    values: HashMap<String, String>,
}

// global config path is ~/.utoo/config.toml
// local config path is .utoo/config.toml
impl Config {
    pub fn load(global: bool) -> Result<Self> {
        if global {
            return Self::load_from_path(&Self::global_config_path()?);
        }

        let mut config = Self::load_from_path(&Self::global_config_path()?)?;
        if let Ok(local_config) = Self::load_from_path(&Self::local_config_path()?) {
            // merge config values
            config.values.extend(local_config.values);
        }
        Ok(config)
    }

    pub fn set(&mut self, key: &str, value: String, global: bool) -> Result<()> {
        self.values.insert(key.to_string(), value);
        self.save(global)
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        Ok(self.values.get(key).cloned())
    }

    fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, global: bool) -> Result<()> {
        let path = if global {
            Self::global_config_path()?
        } else {
            Self::local_config_path()?
        };

        // ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    fn global_config_path() -> Result<PathBuf> {
        Ok(dirs::home_dir()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory not found")
            })?
            .join(".utoo/config.toml"))
    }

    fn local_config_path() -> Result<PathBuf> {
        Ok(std::env::current_dir()?.join(".utoo.toml"))
    }
}
