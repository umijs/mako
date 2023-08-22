use std::collections::HashMap;
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use swc_ecma_ast::EsVersion;
use thiserror::Error;

#[derive(Deserialize, Debug)]
pub struct OutputConfig {
    pub path: PathBuf,
    pub mode: OutputMode,
    #[serde(rename(deserialize = "esVersion"))]
    pub es_version: EsVersion,
}

#[derive(Deserialize, Debug)]
pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
    pub extensions: Vec<String>,
}

pub type Providers = HashMap<String, (String, String)>;

#[derive(Deserialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum Mode {
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "production")]
    Production,
}

#[derive(Deserialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum OutputMode {
    #[serde(rename = "bundle")]
    Bundle,
    #[serde(rename = "minifish")]
    MinifishPrebuild,
}

// TODO:
// 1. node specific runtime
#[derive(Deserialize, Debug, PartialEq, Eq)]
pub enum Platform {
    #[serde(rename = "browser")]
    Browser,
    #[serde(rename = "node")]
    Node,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value().unwrap().get_name().fmt(f)
    }
}

#[derive(Deserialize, Debug)]
pub enum DevtoolConfig {
    /// Generate separate sourcemap file
    #[serde(rename = "source-map")]
    SourceMap,
    /// Generate inline sourcemap
    #[serde(rename = "inline-source-map")]
    InlineSourceMap,
    #[serde(rename = "none")]
    None,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub enum ModuleIdStrategy {
    #[serde(rename = "hashed")]
    Hashed,
    #[serde(rename = "named")]
    Named,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub enum CodeSplittingStrategy {
    #[serde(rename = "bigVendor")]
    BigVendor,
    #[serde(rename = "depPerChunk")]
    DepPerChunk,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub entry: HashMap<String, PathBuf>,
    pub output: OutputConfig,
    pub resolve: ResolveConfig,
    pub manifest: bool,
    #[serde(rename = "manifestConfig")]
    #[serde(deserialize_with = "deserialize_key_to_snake_style")]
    pub manifest_config: HashMap<String, String>,
    pub mode: Mode,
    pub minify: bool,
    pub devtool: DevtoolConfig,
    pub externals: HashMap<String, String>,
    pub providers: Providers,
    pub copy: Vec<String>,
    #[serde(rename = "publicPath")]
    pub public_path: String,
    #[serde(rename = "inlineLimit")]
    pub inline_limit: usize,
    pub targets: HashMap<String, usize>,
    pub platform: Platform,
    #[serde(rename = "moduleIdStrategy")]
    pub module_id_strategy: ModuleIdStrategy,
    pub define: HashMap<String, Value>,
    pub stats: bool,
    pub mdx: bool,
    // temp solution
    pub hmr: bool,
    #[serde(rename = "hmrPort")]
    pub hmr_port: String,
    #[serde(rename = "hmrHost")]
    pub hmr_host: String,
    #[serde(rename = "codeSplitting")]
    pub code_splitting: CodeSplittingStrategy,
    // temp flag
    #[serde(rename = "extractCss")]
    pub extract_css: bool,
}

const CONFIG_FILE: &str = "mako.config.json";
const DEFAULT_CONFIG: &str = r#"
{
    "entry": {},
    "output": { "path": "dist", "mode": "bundle", "esVersion": "es2022" },
    "resolve": { "alias": {}, "extensions": ["js", "jsx", "ts", "tsx"] },
    "mode": "development",
    "minify": true,
    "devtool": "source-map",
    "externals": {},
    "copy": ["public"],
    "providers": {},
    "publicPath": "/",
    "inlineLimit": 10000,
    "targets": { "chrome": 80 },
    "define": {},
    "manifest": false,
    "manifestConfig": { "fileName": "asset-manifest.json", "basePath": "" },
    "stats": false,
    "mdx": false,
    "platform": "browser",
    "hmr": true,
    "hmrHost": "127.0.0.1",
    "hmrPort": "3000",
    "moduleIdStrategy": "named",
    "codeSplitting": "bigVendor",
    "extractCss": false
}
"#;

// TODO:
// - support .ts file
// - add validation

impl Config {
    pub fn new(
        root: &Path,
        default_config: Option<&str>,
        cli_config: Option<&str>,
    ) -> Result<Self, config::ConfigError> {
        let abs_config_file = root.join(CONFIG_FILE);
        let abs_config_file = abs_config_file.to_str().unwrap();
        let c = config::Config::builder();
        // default config
        let c = c.add_source(config::File::from_str(
            DEFAULT_CONFIG,
            config::FileFormat::Json,
        ));
        // default config from args
        let c = if let Some(default_config) = default_config {
            c.add_source(config::File::from_str(
                default_config,
                config::FileFormat::Json,
            ))
        } else {
            c
        };
        // user config
        let c = c.add_source(config::File::with_name(abs_config_file).required(false));
        // cli config
        let c = if let Some(cli_config) = cli_config {
            c.add_source(config::File::from_str(cli_config, config::FileFormat::Json))
        } else {
            c
        };

        let c = c.build()?;
        let mut ret = c.try_deserialize::<Config>();
        // normalize & check
        if let Ok(config) = &mut ret {
            if config.output.path.is_relative() {
                config.output.path = root.join(config.output.path.to_string_lossy().to_string());
            }

            let mode = format!("\"{}\"", config.mode);

            config
                .define
                .entry("NODE_ENV".to_string())
                .or_insert_with(|| serde_json::Value::String(mode));

            if config.public_path != "runtime" && !config.public_path.ends_with('/') {
                panic!("public_path must end with '/' or be 'runtime'");
            }

            // let entry_length = cc.entry.len();
            // if entry_length != 1 {
            //     panic!(
            //         "Only one entry is allowed, but {} entries are found",
            //         entry_length
            //     );
            // }
        }
        ret
    }
}

impl Default for Config {
    fn default() -> Self {
        let c = config::Config::builder();
        let c = c.add_source(config::File::from_str(
            DEFAULT_CONFIG,
            config::FileFormat::Json,
        ));
        let c = c.build().unwrap();
        c.try_deserialize::<Config>().unwrap()
    }
}

// 将驼峰风格转化成蛇形风格
fn from_camel_case_to_snake_case(camel_case_str: &str) -> String {
    let mut result = String::new();
    for (index, char) in camel_case_str.char_indices() {
        if char.is_uppercase() {
            if index > 0 {
                result.push('_');
            }
            result.extend(char.to_lowercase());
        } else {
            result.push(char);
        }
    }
    result
}

// 自定义序列化函数，提供给 serde 消费
fn deserialize_key_to_snake_style<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let camel_case_map: HashMap<String, String> = HashMap::deserialize(deserializer)?;
    let snake_case_map: HashMap<String, String> = camel_case_map
        .into_iter()
        .map(|(k, v)| (from_camel_case_to_snake_case(&k), v))
        .collect();
    Ok(snake_case_map)
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("define value '{0}' is not an Expression")]
    InvalidateDefineConfig(String),
}

#[cfg(test)]
mod tests {
    use crate::config::{Config, Mode, Platform};

    #[test]
    fn test_config() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(&current_dir.join("test/config/normal"), None, None).unwrap();
        println!("{:?}", config);
        assert_eq!(config.platform, Platform::Node);
    }

    #[test]
    fn test_config_args_default() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(
            &current_dir.join("test/config/normal"),
            Some(r#"{"mode":"production"}"#),
            None,
        )
        .unwrap();
        println!("{:?}", config);
        assert_eq!(config.mode, Mode::Production);
    }

    #[test]
    fn test_config_cli_args() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(
            &current_dir.join("test/config/normal"),
            None,
            Some(r#"{"platform":"browser"}"#),
        )
        .unwrap();
        println!("{:?}", config);
        assert_eq!(config.platform, Platform::Browser);
    }

    #[test]
    #[should_panic(expected = "public_path must end with '/' or be 'runtime'")]
    fn test_config_invalid_public_path() {
        let current_dir = std::env::current_dir().unwrap();
        Config::new(
            &current_dir.join("test/config/normal"),
            None,
            Some(r#"{"publicPath":"abc"}"#),
        )
        .unwrap();
    }

    #[test]
    fn test_config_deserialize() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(&current_dir.join("test/config/deserialize"), None, None).unwrap();
        let file_name = config.manifest_config.get("file_name");
        assert_eq!(config.hmr_port, "4000");
        assert_eq!(file_name.unwrap().as_str(), "test.json");
    }
}
