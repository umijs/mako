use std::collections::HashMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};

use mako_core::clap::ValueEnum;
use mako_core::colored::Colorize;
use mako_core::serde::Deserialize;
use mako_core::serde_json::Value;
use mako_core::swc_ecma_ast::EsVersion;
use mako_core::thiserror::Error;
use mako_core::twox_hash::XxHash64;
use mako_core::{clap, config, thiserror};
use serde::Serialize;

#[derive(Deserialize, Serialize, Debug)]
pub struct OutputConfig {
    pub path: PathBuf,
    pub mode: OutputMode,
    #[serde(rename(deserialize = "esVersion"))]
    pub es_version: EsVersion,
    pub meta: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ManifestConfig {
    #[serde(rename(deserialize = "fileName"))]
    pub file_name: String,
    #[serde(rename(deserialize = "basePath"))]
    pub base_path: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ResolveConfig {
    pub alias: HashMap<String, String>,
    pub extensions: Vec<String>,
}

// format: HashMap<identifier, (import_source, specifier)>
// e.g.
// { "process": ("process", "") }
// { "Buffer": ("buffer", "Buffer") }
pub type Providers = HashMap<String, (String, String)>;

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum Mode {
    #[serde(rename = "development")]
    Development,
    #[serde(rename = "production")]
    Production,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum OutputMode {
    #[serde(rename = "bundle")]
    Bundle,
    #[serde(rename = "minifish")]
    MinifishPrebuild,
}

// TODO:
// 1. node specific runtime
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
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

#[derive(Deserialize, Serialize, Debug)]
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

#[derive(Deserialize, Serialize, Debug)]
pub struct LessConfig {
    pub theme: HashMap<String, String>,
    #[serde(rename(deserialize = "lesscPath"))]
    pub lessc_path: String,
    #[serde(rename(deserialize = "javascriptEnabled"))]
    pub javascript_enabled: bool,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum ModuleIdStrategy {
    #[serde(rename = "hashed")]
    Hashed,
    #[serde(rename = "named")]
    Named,
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum CodeSplittingStrategy {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "none")]
    None,
}
#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum TreeShakeStrategy {
    #[serde(rename = "basic")]
    Basic,
    #[serde(rename = "advanced")]
    Advanced,
    #[serde(rename = "none")]
    None,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Px2RemConfig {
    pub root: f64,
    #[serde(rename = "propBlackList")]
    pub prop_black_list: Vec<String>,
    #[serde(rename = "propWhiteList")]
    pub prop_white_list: Vec<String>,
    #[serde(rename = "selectorBlackList")]
    pub selector_black_list: Vec<String>,
    #[serde(rename = "selectorWhiteList")]
    pub selector_white_list: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum TransformImportStyle {
    Built(String),
    Source(bool),
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransformImportConfig {
    pub library_name: String,
    pub library_directory: Option<String>,
    pub style: Option<TransformImportStyle>,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ExternalAdvancedSubpathConverter {
    PascalCase,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum ExternalAdvancedSubpathTarget {
    Empty,
    Tpl(String),
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ExternalAdvancedSubpathRule {
    pub regex: String,
    #[serde(with = "external_target_format")]
    pub target: ExternalAdvancedSubpathTarget,
    #[serde(rename = "targetConverter")]
    pub target_converter: Option<ExternalAdvancedSubpathConverter>,
}

/**
 * custom formatter for convert $EMPTY to enum, because rename is not supported for $ symbol
 * @see https://serde.rs/custom-date-format.html
 */
mod external_target_format {
    use serde::{self, Deserialize, Deserializer, Serializer};

    use super::ExternalAdvancedSubpathTarget;

    pub fn serialize<S>(v: &ExternalAdvancedSubpathTarget, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match v {
            ExternalAdvancedSubpathTarget::Empty => serializer.serialize_str("$EMPTY"),
            ExternalAdvancedSubpathTarget::Tpl(s) => serializer.serialize_str(s),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ExternalAdvancedSubpathTarget, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = String::deserialize(deserializer)?;

        if v == "$EMPTY" {
            Ok(ExternalAdvancedSubpathTarget::Empty)
        } else {
            Ok(ExternalAdvancedSubpathTarget::Tpl(v))
        }
    }
}
#[derive(Deserialize, Serialize, Debug)]
pub struct ExternalAdvancedSubpath {
    pub exclude: Option<Vec<String>>,
    pub rules: Vec<ExternalAdvancedSubpathRule>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ExternalAdvanced {
    pub root: String,
    pub subpath: ExternalAdvancedSubpath,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum ExternalConfig {
    Basic(String),
    Advanced(ExternalAdvanced),
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub entry: HashMap<String, PathBuf>,
    pub output: OutputConfig,
    pub resolve: ResolveConfig,
    pub manifest: bool,
    pub manifest_config: ManifestConfig,
    pub mode: Mode,
    pub minify: bool,
    pub devtool: DevtoolConfig,
    pub externals: HashMap<String, ExternalConfig>,
    pub providers: Providers,
    pub copy: Vec<String>,
    pub public_path: String,
    pub inline_limit: usize,
    pub targets: HashMap<String, f32>,
    pub platform: Platform,
    pub module_id_strategy: ModuleIdStrategy,
    pub define: HashMap<String, Value>,
    pub stats: bool,
    pub mdx: bool,
    pub less: LessConfig,
    // temp solution
    pub hmr: bool,
    pub hmr_port: String,
    pub hmr_host: String,
    pub code_splitting: CodeSplittingStrategy,
    pub px2rem: bool,
    #[serde(rename = "px2remConfig")]
    pub px2rem_config: Px2RemConfig,
    pub hash: bool,
    pub tree_shake: TreeShakeStrategy,
    #[serde(rename = "autoCSSModules")]
    pub auto_css_modules: bool,
    #[serde(rename = "ignoreCSSParserErrors")]
    pub ignore_css_parser_errors: bool,
    pub dynamic_import_to_require: bool,
    pub umd: String,
    pub write_to_disk: bool,
    pub transform_import: Vec<TransformImportConfig>,
    pub dev_eval: bool,
}

pub(crate) fn hash_config(c: &Config) -> u64 {
    let mut hasher = XxHash64::default();
    hasher.write(serde_json::to_string(c).unwrap().as_bytes());
    hasher.finish()
}

const CONFIG_FILE: &str = "mako.config.json";
const DEFAULT_CONFIG: &str = r#"
{
    "entry": {},
    "output": { "path": "dist", "mode": "bundle", "esVersion": "es2022", "meta": false },
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
    "less": { "theme": {}, "lesscPath": "", javascriptEnabled: true },
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
    "codeSplitting": "none",
    "hash": false,
    "px2rem": false,
    "px2remConfig": { "root": 100, "propBlackList": [], "propWhiteList": [], "selectorBlackList": [], "selectorWhiteList": [] },
    "treeShake": "basic",
    "autoCSSModules": false,
    "ignoreCSSParserErrors": false,
    "dynamicImportToRequire": false,
    "umd": "none",
    "writeToDisk": true,
    "transformImport": [],
    "devEval": true
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
            config::FileFormat::Json5,
        ));
        // default config from args
        let c = if let Some(default_config) = default_config {
            c.add_source(config::File::from_str(
                default_config,
                config::FileFormat::Json5,
            ))
        } else {
            c
        };
        // user config
        let c = c.add_source(config::File::with_name(abs_config_file).required(false));
        // cli config
        let c = if let Some(cli_config) = cli_config {
            c.add_source(config::File::from_str(
                cli_config,
                config::FileFormat::Json5,
            ))
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

            let node_env_config_opt = config.define.get("NODE_ENV");
            if let Some(node_env_config) = node_env_config_opt {
                if node_env_config.as_str() != Some(config.mode.to_string().as_str()) {
                    let warn_message = format!(
                        "{}: The configuration of {} conflicts with current {} and will be overwritten as {} ",
                        "warning".to_string().yellow(),
                        "NODE_ENV".to_string().yellow(),
                        "mode".to_string().yellow(),
                        config.mode.to_string().red()
                    );
                    println!("{}", warn_message);
                }
            }

            let mode = format!("\"{}\"", config.mode);
            config
                .define
                .insert("NODE_ENV".to_string(), serde_json::Value::String(mode));

            if config.public_path != "runtime" && !config.public_path.ends_with('/') {
                panic!("public_path must end with '/' or be 'runtime'");
            }

            // 暂不支持 remote external
            // 如果 config.externals 中有值是以「script 」开头，则 panic 报错
            let basic_external_values = config
                .externals
                .values()
                .filter_map(|v| match v {
                    ExternalConfig::Basic(b) => Some(b),
                    _ => None,
                })
                .collect::<Vec<_>>();

            for v in basic_external_values {
                if v.starts_with("script ") {
                    panic!(
                        "remote external is not supported yet, but we found {}",
                        v.to_string().red()
                    );
                }
            }

            if config.entry.is_empty() {
                let file_paths = vec!["src/index.tsx", "src/index.ts", "index.tsx", "index.ts"];
                for file_path in file_paths {
                    let file_path = root.join(file_path);
                    if file_path.exists() {
                        config.entry.insert("index".to_string(), file_path);
                        break;
                    }
                }
            }

            config.entry = config
                .entry
                .clone()
                .into_iter()
                .map(|(k, v)| (k, root.join(v).canonicalize().unwrap()))
                .collect();

            // support relative alias
            config.resolve.alias = config
                .resolve
                .alias
                .clone()
                .into_iter()
                .map(|(k, v)| {
                    let v = if v.starts_with('.') {
                        root.join(v).to_string_lossy().to_string()
                    } else {
                        v
                    };
                    (k, v)
                })
                .collect();
        }
        ret
    }
}

impl Default for Config {
    fn default() -> Self {
        let c = config::Config::builder();
        let c = c.add_source(config::File::from_str(
            DEFAULT_CONFIG,
            config::FileFormat::Json5,
        ));
        let c = c.build().unwrap();
        c.try_deserialize::<Config>().unwrap()
    }
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
    fn test_node_env_conflicts_with_mode() {
        let current_dir = std::env::current_dir().unwrap();
        let config = Config::new(
            &current_dir.join("test/config/node-env"),
            None,
            Some(r#"{"mode":"development"}"#),
        )
        .unwrap();
        assert_eq!(
            config.define.get("NODE_ENV"),
            Some(&serde_json::Value::String("\"development\"".to_string()))
        );
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
}
