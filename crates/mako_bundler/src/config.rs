use std::collections::HashMap;

pub struct OutputConfig {
    pub path: String,
}

pub enum Mode {
    Development,
    _Production,
}

pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
}

pub struct Config {
    pub entry: HashMap<String, String>,
    pub output: OutputConfig,
    pub root: String,
    pub mode: Mode,
    pub resolve: ResolveConfig,
    pub externals: HashMap<String, String>,
    // The limit of the size of the file to be converted to base64
    pub data_url_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            entry: HashMap::new(),
            output: OutputConfig {
                path: "dist".to_string(),
            },
            root: std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            mode: Mode::Development,
            resolve: ResolveConfig {
                alias: HashMap::new(),
            },
            externals: HashMap::new(),
            data_url_limit: 8192,
        }
    }
}

pub fn get_first_entry_value(entry: &HashMap<String, String>) -> Result<&str, &'static str> {
    match entry.values().next() {
        Some(value) => Ok(value.as_str()),
        None => Err("Entry is empty"),
    }
}
