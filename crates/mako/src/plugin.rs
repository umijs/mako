use std::sync::Arc;

use anyhow::Result;

use crate::config::Config;

pub trait Plugin {
    fn name(&self) -> &str;
    fn modify_config(&self, _config: &mut Config) -> Result<()> {
        Ok(())
    }
    fn load(&self) -> Result<Option<String>> {
        Ok(None)
    }
}

pub struct PluginDriver {
    plugins: Vec<Arc<dyn Plugin>>,
}
impl PluginDriver {
    pub fn new(plugins: Vec<Arc<dyn Plugin>>) -> Self {
        Self { plugins }
    }
    pub fn modify_config(&self, config: &mut Config) -> Result<()> {
        for plugin in &self.plugins {
            plugin.modify_config(config)?;
        }
        Ok(())
    }
    #[allow(dead_code)]
    pub fn load(&self) -> Result<Option<String>> {
        for plugin in &self.plugins {
            let ret = plugin.load()?;
            if ret.is_some() {
                return Ok(ret);
            }
        }
        Ok(None)
    }
}
