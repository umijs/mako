use super::Plugin;

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
    pub fn register<T: 'static + Plugin>(&mut self, plugin: T) {
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
    pub fn run_hook_first<T, E>(&mut self, mut executor: E) -> Option<T>
    where
        E: FnMut(&dyn Plugin) -> Option<T>,
    {
        for plugin in &self.plugins {
            let ret = executor(plugin.as_ref());

            if ret.is_some() {
                return ret;
            }
        }

        None
    }

    /// run hook in serial mode
    ///
    /// * `executor` - a closure function that accept a plugin, use to call plugin method
    pub fn run_hook_serial<E>(&mut self, mut executor: E)
    where
        E: FnMut(&dyn Plugin),
    {
        for plugin in &self.plugins {
            executor(plugin.as_ref());
        }
    }

    /// run hook in parallel mode
    ///
    /// * `executor` - a closure function that accept a plugin, use to call plugin method
    pub fn run_hook_parallel<E>(&mut self, mut executor: E)
    where
        E: FnMut(&dyn Plugin),
    {
        self.plugins
            .iter()
            .for_each(|plugin| executor(plugin.as_ref()));
    }
}
