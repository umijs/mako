pub mod plugin_driver;

pub trait Plugin {
    /// define plugin name
    ///
    /// Note: it is recommended to prefix a namespace to avoid name conflicts, such as `mako:plugin-xxx`
    fn name(&self) -> &str;

    /// let plugin run before other plugin
    fn before(&self) -> &str {
        ""
    }

    // write other plugin hooks here
    fn example_method(&self, _prefix: String) -> Option<String> {
        None
    }
}
