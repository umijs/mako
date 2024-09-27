mod analyze;
mod code_splitting;
mod dev_server;
mod devtool;
mod duplicate_package_checker;
mod experimental;
mod external;
mod generic_usize;
mod hmr;
mod inline_css;
mod macros;
mod manifest;
mod minifish;
mod mode;
mod module_id_strategy;
mod optimization;
mod output;
mod progress;
mod provider;
mod px2rem;
mod react;
mod resolve;
mod rsc_client;
mod rsc_server;
mod stats;
mod transform_import;
mod tree_shaking;
mod umd;
mod watch;

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub use analyze::AnalyzeConfig;
use anyhow::{anyhow, Result};
pub use code_splitting::*;
use colored::Colorize;
use config;
pub use dev_server::{deserialize_dev_server, DevServerConfig};
pub use devtool::{deserialize_devtool, DevtoolConfig};
pub use duplicate_package_checker::{
    deserialize_check_duplicate_package, DuplicatePackageCheckerConfig,
};
use experimental::ExperimentalConfig;
pub use external::{
    ExternalAdvanced, ExternalAdvancedSubpath, ExternalAdvancedSubpathConverter,
    ExternalAdvancedSubpathRule, ExternalAdvancedSubpathTarget, ExternalConfig,
};
pub use generic_usize::GenericUsizeDefault;
pub use hmr::{deserialize_hmr, HmrConfig};
pub use inline_css::{deserialize_inline_css, InlineCssConfig};
pub use manifest::{deserialize_manifest, ManifestConfig};
use miette::{miette, ByteOffset, Diagnostic, NamedSource, SourceOffset, SourceSpan};
pub use minifish::{deserialize_minifish, MinifishConfig};
pub use mode::Mode;
pub use module_id_strategy::ModuleIdStrategy;
pub use optimization::{deserialize_optimization, OptimizationConfig};
use output::get_default_chunk_loading_global;
pub use output::{CrossOriginLoading, OutputConfig, OutputMode};
pub use progress::{deserialize_progress, ProgressConfig};
pub use provider::Providers;
pub use px2rem::{deserialize_px2rem, Px2RemConfig};
pub use react::{ReactConfig, ReactRuntimeConfig};
pub use resolve::ResolveConfig;
pub use rsc_client::{deserialize_rsc_client, LogServerComponent, RscClientConfig};
pub use rsc_server::{deserialize_rsc_server, RscServerConfig};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use stats::{deserialize_stats, StatsConfig};
use thiserror::Error;
pub use transform_import::{TransformImportConfig, TransformImportStyle};
pub use tree_shaking::{deserialize_tree_shaking, TreeShakingStrategy};
pub use umd::{deserialize_umd, Umd};
pub use watch::WatchConfig;

use crate::features::node::Node;

#[derive(Debug, Diagnostic)]
#[diagnostic(code("mako.config.json parsed failed"))]
struct ConfigParseError {
    #[source_code]
    src: NamedSource,
    #[label("Error here.")]
    span: SourceSpan,
    message: String,
}

impl std::error::Error for ConfigParseError {}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

fn validate_mako_config(abs_config_file: String) -> miette::Result<()> {
    if Path::new(&abs_config_file).exists() {
        let content = std::fs::read_to_string(abs_config_file.clone())
            .map_err(|e| miette!("Failed to read file '{}': {}", &abs_config_file, e))?;
        let result: Result<Value, serde_json::Error> = serde_json::from_str(&content);
        if let Err(e) = result {
            let line = e.line();
            let column = e.column();
            let start = SourceOffset::from_location(&content, line, column);
            let span = SourceSpan::new(start, (1 as ByteOffset).into());
            return Err(ConfigParseError {
                src: NamedSource::new("mako.config.json", content),
                span,
                message: e.to_string(),
            }
            .into());
        }
    }
    Ok(())
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
pub enum Platform {
    #[serde(rename = "browser")]
    Browser,
    #[serde(rename = "node")]
    Node,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub entry: HashMap<String, PathBuf>,
    pub output: OutputConfig,
    pub resolve: ResolveConfig,
    #[serde(deserialize_with = "deserialize_manifest", default)]
    pub manifest: Option<ManifestConfig>,
    pub mode: Mode,
    pub minify: bool,
    #[serde(deserialize_with = "deserialize_devtool")]
    pub devtool: Option<DevtoolConfig>,
    pub externals: HashMap<String, ExternalConfig>,
    pub providers: Providers,
    pub copy: Vec<String>,
    pub public_path: String,
    pub inline_limit: usize,
    pub inline_excludes_extensions: Vec<String>,
    pub targets: HashMap<String, f32>,
    pub platform: Platform,
    pub module_id_strategy: ModuleIdStrategy,
    pub define: HashMap<String, Value>,
    pub analyze: Option<AnalyzeConfig>,
    pub stats: Option<StatsConfig>,
    pub mdx: bool,
    #[serde(deserialize_with = "deserialize_hmr")]
    pub hmr: Option<HmrConfig>,
    #[serde(deserialize_with = "deserialize_dev_server")]
    pub dev_server: Option<DevServerConfig>,
    #[serde(deserialize_with = "deserialize_code_splitting", default)]
    pub code_splitting: Option<CodeSplitting>,
    #[serde(deserialize_with = "deserialize_px2rem", default)]
    pub px2rem: Option<Px2RemConfig>,
    #[serde(deserialize_with = "deserialize_progress", default)]
    pub progress: Option<ProgressConfig>,
    pub hash: bool,
    #[serde(rename = "_treeShaking", deserialize_with = "deserialize_tree_shaking")]
    pub _tree_shaking: Option<TreeShakingStrategy>,
    #[serde(rename = "autoCSSModules")]
    pub auto_css_modules: bool,
    #[serde(rename = "ignoreCSSParserErrors")]
    pub ignore_css_parser_errors: bool,
    pub dynamic_import_to_require: bool,
    #[serde(deserialize_with = "deserialize_umd", default)]
    pub umd: Option<Umd>,
    pub cjs: bool,
    pub write_to_disk: bool,
    pub transform_import: Vec<TransformImportConfig>,
    pub chunk_parallel: bool,
    pub clean: bool,
    pub node_polyfill: bool,
    pub ignores: Vec<String>,
    #[serde(
        rename = "_minifish",
        deserialize_with = "deserialize_minifish",
        default
    )]
    pub _minifish: Option<MinifishConfig>,
    #[serde(rename = "optimizePackageImports")]
    pub optimize_package_imports: bool,
    pub emotion: bool,
    pub flex_bugs: bool,
    #[serde(deserialize_with = "deserialize_optimization")]
    pub optimization: Option<OptimizationConfig>,
    pub react: ReactConfig,
    pub emit_assets: bool,
    #[serde(rename = "cssModulesExportOnlyLocales")]
    pub css_modules_export_only_locales: bool,
    #[serde(
        rename = "inlineCSS",
        deserialize_with = "deserialize_inline_css",
        default
    )]
    pub inline_css: Option<InlineCssConfig>,
    #[serde(
        rename = "rscServer",
        deserialize_with = "deserialize_rsc_server",
        default
    )]
    pub rsc_server: Option<RscServerConfig>,
    #[serde(
        rename = "rscClient",
        deserialize_with = "deserialize_rsc_client",
        default
    )]
    pub rsc_client: Option<RscClientConfig>,
    pub experimental: ExperimentalConfig,
    pub watch: WatchConfig,
    pub use_define_for_class_fields: bool,
    pub emit_decorator_metadata: bool,
    #[serde(
        rename = "duplicatePackageChecker",
        deserialize_with = "deserialize_check_duplicate_package",
        default
    )]
    pub check_duplicate_package: Option<DuplicatePackageCheckerConfig>,
}

const CONFIG_FILE: &str = "mako.config.json";
const DEFAULT_CONFIG: &str = include_str!("./config/mako.config.default.json");

impl Config {
    pub fn new(
        root: &Path,
        default_config: Option<&str>,
        cli_config: Option<&str>,
    ) -> Result<Self> {
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
        // validate user config
        validate_mako_config(abs_config_file.to_string())
            .map_err(|e| anyhow!("{}", format!("{:?}", e)))?;
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
            // normalize output
            if config.output.path.is_relative() {
                config.output.path = root.join(config.output.path.to_string_lossy().to_string());
            }

            if config.output.chunk_loading_global.is_empty() {
                config.output.chunk_loading_global =
                    get_default_chunk_loading_global(config.umd.clone(), root);
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

            if config.cjs && config.umd.is_some() {
                return Err(anyhow!("cjs and umd cannot be used at the same time",));
            }

            if config.hmr.is_some() && config.dev_server.is_none() {
                return Err(anyhow!("hmr can only be used with devServer",));
            }

            if config.inline_css.is_some() && config.umd.is_none() {
                return Err(anyhow!("inlineCSS can only be used with umd",));
            }

            let mode = format!("\"{}\"", config.mode);
            config
                .define
                .insert("NODE_ENV".to_string(), serde_json::Value::String(mode));

            if config.public_path != "runtime" && !config.public_path.ends_with('/') {
                return Err(anyhow!("public_path must end with '/' or be 'runtime'"));
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
                    return Err(anyhow!(
                        "remote external is not supported yet, but we found {}",
                        v.to_string().red()
                    ));
                }
            }

            // support default entries
            if config.entry.is_empty() {
                let file_paths = vec!["src/index.tsx", "src/index.ts", "index.tsx", "index.ts"];
                for file_path in file_paths {
                    let file_path = root.join(file_path);
                    if file_path.exists() {
                        config.entry.insert("index".to_string(), file_path);
                        break;
                    }
                }
                if config.entry.is_empty() {
                    return Err(anyhow!("Entry is empty"));
                }
            }

            // normalize entry
            let entry_tuples = config
                .entry
                .clone()
                .into_iter()
                .map(|(k, v)| {
                    if let Ok(entry_path) = root.join(v).canonicalize() {
                        Ok((k, entry_path))
                    } else {
                        Err(anyhow!("entry:{} not found", k,))
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            config.entry = entry_tuples.into_iter().collect();

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

            // dev 环境下不产生 hash, prod 环境下根据用户配置
            if config.mode == Mode::Development {
                config.hash = false;
            }

            // configure node platform
            Node::modify_config(config);
        }
        ret.map_err(|e| anyhow!("{}: {}", "config error".red(), e.to_string().red()))
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

    #[test]
    fn test_node_platform() {
        let current_dir = std::env::current_dir().unwrap();
        let config =
            Config::new(&current_dir.join("test/config/node-platform"), None, None).unwrap();
        assert_eq!(
            config.targets.get("node"),
            Some(&14.0),
            "use node targets by default if platform is node",
        );
        assert!(
            config.ignores.iter().any(|i| i.contains("|fs|")),
            "ignore Node.js standard library by default if platform is node",
        );
    }
}
