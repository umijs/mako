use config;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, str::FromStr};

#[derive(Debug, Deserialize)]
pub struct OutputConfig {
    pub path: String,
}

// #[derive(Debug, Deserialize)]
// pub enum Mode {
//     Development,
//     _Production,
// }

#[derive(Debug, Deserialize)]
pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
    pub extensions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub entry: HashMap<String, String>,
    pub output: OutputConfig,
    pub root: String,
    // pub mode: Mode,
    pub resolve: ResolveConfig,
    pub externals: HashMap<String, String>,
    // The limit of the size of the file to be converted to base64
    pub data_url_limit: usize,
}

impl Config {
    pub fn from_str(s: &str) -> Result<Self, config::ConfigError> {
        let s = config::Config::builder()
            .add_source(config::File::from_str(
                &Self::default_str(),
                config::FileFormat::Json,
            ))
            .add_source(config::File::from_str(s, config::FileFormat::Json))
            .build()?;
        s.try_deserialize()
    }

    pub fn default_str() -> String {
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        format!(
            r#"
{{
    "entry": {{}},
    "output": {{ "path": "dist" }},
    "root": "{}",
    "resolve": {{ "alias": {{}}, "extensions": ["js", "jsx", "ts", "tsx"] }},
    "externals": {{}},
    "data_url_limit": 8192
}}
            "#,
            cwd,
        )
    }

    pub fn normalize(&mut self) {
        self.output.path = PathBuf::from_str(&self.root)
            .unwrap()
            .join(&self.output.path)
            .to_string_lossy()
            .to_string();

        let entry_length = self.entry.len();
        if entry_length != 1 {
            panic!(
                "Only one entry is allowed, but {} entries are found",
                entry_length
            );
        }
    }
}

pub fn get_first_entry_value(entry: &HashMap<String, String>) -> Result<&str, &'static str> {
    match entry.values().next() {
        Some(value) => Ok(value.as_str()),
        None => Err("Entry is empty"),
    }
}
