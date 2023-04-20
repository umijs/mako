use config;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct OutputConfig {
    pub path: PathBuf,
}

// #[derive(Debug, Deserialize)]
// pub enum Mode {
//     Development,
//     _Production,
// }

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
    pub extensions: Vec<String>,
}

#[derive(Eq, PartialEq, Debug, Deserialize, Serialize)]
pub struct Config {
    pub entry: HashMap<String, PathBuf>,
    pub output: OutputConfig,
    pub root: PathBuf,
    // pub mode: Mode,
    pub resolve: ResolveConfig,
    pub externals: HashMap<String, String>,
    // The limit of the size of the file to be converted to base64
    pub data_url_limit: usize,
}

impl Config {
    pub fn from_literal_str(s: &str) -> Result<Self, config::ConfigError> {
        let conf = config::Config::builder()
            .add_source(config::File::from_str(
                &Self::default_str(),
                config::FileFormat::Json,
            ))
            .add_source(config::File::from_str(s, config::FileFormat::Json))
            .build()?;
        conf.try_deserialize()
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
        if self.output.path.is_relative() {
            self.output.path = self.root.join(&self.output.path)
        };

        let entry_length = self.entry.len();
        if entry_length != 1 {
            panic!(
                "Only one entry is allowed, but {} entries are found",
                entry_length
            );
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_literal_str(Config::default_str().as_str()).unwrap()
    }
}

pub fn get_first_entry_value(entry: &HashMap<String, PathBuf>) -> Result<&PathBuf, Error> {
    match entry.values().next() {
        Some(value) => Ok(value),
        None => Err(Error::new(ErrorKind::NotFound, "No entry found".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, ResolveConfig};
    use maplit::hashmap;
    use std::path::PathBuf;

    #[test]
    fn test_config_serialize() {
        let config: Config = Config {
            entry: hashmap! {
                "index".to_string() => PathBuf::from("index.tsx")
            },
            resolve: ResolveConfig {
                alias: hashmap! {
                    "react".to_string() => "React".to_string(),
                },
                extensions: vec!["tsx".to_string()],
            },
            root: PathBuf::from("/root"),
            ..Default::default()
        };
        let s = serde_json::to_string_pretty(&config).unwrap();
        assert_eq!(
            s,
            r#"{
  "entry": {
    "index": "index.tsx"
  },
  "output": {
    "path": "dist"
  },
  "root": "/root",
  "resolve": {
    "alias": {
      "react": "React"
    },
    "extensions": [
      "tsx"
    ]
  },
  "externals": {},
  "data_url_limit": 8192
}"#
        )
    }

    #[test]
    fn test_config_deserialize() {
        let expected: Config = Config {
            externals: hashmap! {
                "react".to_string() => "React".to_string(),
            },
            ..Default::default()
        };
        let s = serde_json::to_string_pretty::<Config>(&expected).unwrap();

        let config: Config = serde_json::from_str(&s).unwrap();

        assert_eq!(config, expected);
    }
}
