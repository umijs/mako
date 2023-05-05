use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use super::{Plugin, Result};

pub struct PluginDriver {
    plugins: Vec<Box<dyn Plugin>>,
}

impl Default for PluginDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginDriver {
    pub fn new() -> Self {
        Self { plugins: vec![] }
    }

    fn check_plugin_exist(&self, name: &str) -> bool {
        self.plugins.iter().any(|p| p.name() == name)
    }

    /// register a new plugin
    ///
    /// * `plugin` - a plugin instance
    pub fn register<T: Plugin>(&mut self, plugin: T) {
        assert!(
            !self.check_plugin_exist(plugin.name()),
            "plugin {} already exist, please check your plugin name",
            plugin.name()
        );
        let mut insert_pos = self.plugins.len();
        let before = plugin.before();

        if let Some(before_pos) = match before.is_empty() {
            false => self.plugins.iter().position(|p| p.name() == before),
            true => None,
        } {
            insert_pos = before_pos;
        }

        self.plugins.insert(insert_pos, Box::new(plugin));
    }

    /// run hook in first mode and return first result
    ///
    /// * `executor` - a closure function that accept a plugin, use to call plugin method and return result
    pub fn run_hook_first<T, E>(&mut self, executor: E) -> Result<Option<T>>
    where
        E: Fn(&dyn Plugin) -> Result<Option<T>>,
    {
        for plugin in &self.plugins {
            let ret = executor(plugin.as_ref());

            if ret.is_ok() && ret.as_ref().unwrap().is_some() {
                return ret;
            }
        }

        Ok(None)
    }

    /// run hook in serial mode
    ///
    /// * `executor` - a closure function that accept a plugin, use to call plugin method and serial return result
    pub fn run_hook_serial<T, E>(&mut self, executor: E) -> Result<Option<T>>
    where
        E: Fn(&dyn Plugin, Result<Option<T>>) -> Result<Option<T>>,
    {
        let mut last_ret: Result<Option<T>> = Ok(None);

        for plugin in &self.plugins {
            last_ret = executor(plugin.as_ref(), last_ret);
        }

        last_ret
    }

    /// run hook in parallel mode
    ///
    /// * `executor` - a closure function that accept a plugin, use to call plugin method
    pub fn run_hook_parallel<E>(&mut self, executor: E)
    where
        E: Fn(&dyn Plugin) + Sync,
    {
        self.plugins
            .par_iter()
            .for_each(|plugin| executor(plugin.as_ref()));
    }
}
