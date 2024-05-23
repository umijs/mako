use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

use mako_core::anyhow::{anyhow, Result};
use mako_core::clap::ValueEnum;
use mako_core::colored::Colorize;
use mako_core::regex::Regex;
use mako_core::serde::{Deserialize, Deserializer};
use mako_core::serde_json::Value;
use mako_core::swc_ecma_ast::EsVersion;
use mako_core::thiserror::Error;
use mako_core::{clap, config, thiserror};
use miette::{miette, ByteOffset, Diagnostic, NamedSource, SourceOffset, SourceSpan};
use serde::Serialize;

use crate::features::node::Node;
use crate::generate::optimize_chunk;
use crate::{plugins, visitors};

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

/**
 * a macro to create deserialize function that allow false value for optional struct
 */
macro_rules! create_deserialize_fn {
    ($fn_name:ident, $struct_type:ty) => {
        pub fn $fn_name<'de, D>(deserializer: D) -> Result<Option<$struct_type>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let value: serde_json::Value = serde_json::Value::deserialize(deserializer)?;

            match value {
                // allow false value for optional struct
                serde_json::Value::Bool(false) => Ok(None),
                // try deserialize
                serde_json::Value::Object(obj) => Ok(Some(
                    serde_json::from_value::<$struct_type>(serde_json::Value::Object(obj))
                        .map_err(serde::de::Error::custom)?,
                )),
                serde_json::Value::String(s) => Ok(Some(
                    serde_json::from_value::<$struct_type>(serde_json::Value::String(s.clone()))
                        .map_err(serde::de::Error::custom)?,
                )),
                _ => Err(serde::de::Error::custom(format!(
                    "invalid `{}` value: {}",
                    stringify!($fn_name).replace("deserialize_", ""),
                    value
                ))),
            }
        }
    };
}
create_deserialize_fn!(deserialize_hmr, HmrConfig);
create_deserialize_fn!(deserialize_manifest, ManifestConfig);
create_deserialize_fn!(deserialize_code_splitting, CodeSplittingStrategy);
create_deserialize_fn!(deserialize_px2rem, Px2RemConfig);
create_deserialize_fn!(deserialize_umd, String);
create_deserialize_fn!(deserialize_devtool, DevtoolConfig);
create_deserialize_fn!(deserialize_tree_shaking, TreeShakingStrategy);
create_deserialize_fn!(deserialize_optimization, OptimizationConfig);
create_deserialize_fn!(deserialize_minifish, MinifishConfig);
create_deserialize_fn!(deserialize_inline_css, InlineCssConfig);
create_deserialize_fn!(deserialize_rsc_client, RscClientConfig);
create_deserialize_fn!(deserialize_rsc_server, RscServerConfig);
create_deserialize_fn!(deserialize_stats, StatsConfig);

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub path: PathBuf,
    pub mode: OutputMode,
    pub es_version: EsVersion,
    pub meta: bool,
    pub chunk_loading_global: String,
    pub preserve_modules: bool,
    pub preserve_modules_root: PathBuf,
    pub skip_write: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ManifestConfig {
    #[serde(
        rename(deserialize = "fileName"),
        default = "plugins::manifest::default_manifest_file_name"
    )]
    pub file_name: String,
    #[serde(rename(deserialize = "basePath"), default)]
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
    #[serde(rename = "bundless")]
    Bundless,
}

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
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum ModuleIdStrategy {
    #[serde(rename = "hashed")]
    Hashed,
    #[serde(rename = "named")]
    Named,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct StatsConfig {
    pub modules: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum CodeSplittingStrategy {
    #[serde(rename = "auto")]
    Auto,
    #[serde(untagged)]
    Advanced(OptimizeChunkOptions),
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug)]
pub enum TreeShakingStrategy {
    #[serde(rename = "basic")]
    Basic,
    #[serde(rename = "advanced")]
    Advanced,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Px2RemConfig {
    #[serde(default = "visitors::css_px2rem::default_root")]
    pub root: f64,
    #[serde(rename = "propBlackList", default)]
    pub prop_blacklist: Vec<String>,
    #[serde(rename = "propWhiteList", default)]
    pub prop_whitelist: Vec<String>,
    #[serde(rename = "selectorBlackList", default)]
    pub selector_blacklist: Vec<String>,
    #[serde(rename = "selectorWhiteList", default)]
    pub selector_whitelist: Vec<String>,
    #[serde(rename = "minPixelValue", default)]
    pub min_pixel_value: f64,
}

impl Default for Px2RemConfig {
    fn default() -> Self {
        Px2RemConfig {
            root: visitors::css_px2rem::default_root(),
            prop_blacklist: vec![],
            prop_whitelist: vec![],
            selector_blacklist: vec![],
            selector_whitelist: vec![],
            min_pixel_value: 0.0,
        }
    }
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
    #[serde(rename = "type")]
    pub module_type: Option<String>,
    pub script: Option<String>,
    pub subpath: Option<ExternalAdvancedSubpath>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum ExternalConfig {
    Basic(String),
    Advanced(ExternalAdvanced),
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InjectItem {
    pub from: String,
    pub named: Option<String>,
    pub namespace: Option<bool>,
    pub exclude: Option<String>,
    pub include: Option<String>,
    pub prefer_require: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum ReactRuntimeConfig {
    #[serde(rename = "automatic")]
    Automatic,
    #[serde(rename = "classic")]
    Classic,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ReactConfig {
    pub pragma: String,
    #[serde(rename = "importSource")]
    pub import_source: String,
    pub runtime: ReactRuntimeConfig,
    #[serde(rename = "pragmaFrag")]
    pub pragma_frag: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MinifishConfig {
    pub mapping: HashMap<String, String>,
    pub meta_path: Option<PathBuf>,
    pub inject: Option<HashMap<String, InjectItem>>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationConfig {
    pub skip_modules: Option<bool>,
    pub concatenate_modules: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct InlineCssConfig {}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RscServerConfig {
    pub client_component_tpl: String,
    #[serde(rename = "emitCSS")]
    pub emit_css: bool,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, ValueEnum, Clone)]
pub enum LogServerComponent {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "ignore")]
    Ignore,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RscClientConfig {
    pub log_server_component: LogServerComponent,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExperimentalConfig {
    pub webpack_syntax_validate: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WatchConfig {
    pub ignore_paths: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HmrConfig {
    pub host: String,
    pub port: u16,
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
    pub targets: HashMap<String, f32>,
    pub platform: Platform,
    pub module_id_strategy: ModuleIdStrategy,
    pub define: HashMap<String, Value>,
    pub stats: Option<StatsConfig>,
    pub mdx: bool,
    #[serde(deserialize_with = "deserialize_hmr")]
    pub hmr: Option<HmrConfig>,
    #[serde(deserialize_with = "deserialize_code_splitting", default)]
    pub code_splitting: Option<CodeSplittingStrategy>,
    #[serde(deserialize_with = "deserialize_px2rem", default)]
    pub px2rem: Option<Px2RemConfig>,
    pub hash: bool,
    #[serde(rename = "_treeShaking", deserialize_with = "deserialize_tree_shaking")]
    pub _tree_shaking: Option<TreeShakingStrategy>,
    #[serde(rename = "autoCSSModules")]
    pub auto_css_modules: bool,
    #[serde(rename = "ignoreCSSParserErrors")]
    pub ignore_css_parser_errors: bool,
    pub dynamic_import_to_require: bool,
    #[serde(deserialize_with = "deserialize_umd", default)]
    pub umd: Option<String>,
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
}

#[allow(dead_code)]
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub enum OptimizeAllowChunks {
    #[serde(rename = "all")]
    All,
    #[serde(rename = "entry")]
    Entry,
    #[serde(rename = "async")]
    #[default]
    Async,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeChunkOptions {
    #[serde(default = "optimize_chunk::default_min_size")]
    pub min_size: usize,
    pub groups: Vec<OptimizeChunkGroup>,
}

impl Default for OptimizeChunkOptions {
    fn default() -> Self {
        OptimizeChunkOptions {
            min_size: optimize_chunk::default_min_size(),
            groups: vec![],
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptimizeChunkGroup {
    pub name: String,
    #[serde(default)]
    pub allow_chunks: OptimizeAllowChunks,
    #[serde(default = "optimize_chunk::default_min_chunks")]
    pub min_chunks: usize,
    #[serde(default = "optimize_chunk::default_min_size")]
    pub min_size: usize,
    #[serde(default = "optimize_chunk::default_max_size")]
    pub max_size: usize,
    #[serde(default)]
    pub priority: i8,
    #[serde(default, with = "optimize_test_format")]
    pub test: Option<Regex>,
}

impl Default for OptimizeChunkGroup {
    fn default() -> Self {
        OptimizeChunkGroup {
            name: String::default(),
            allow_chunks: OptimizeAllowChunks::default(),
            min_chunks: optimize_chunk::default_min_chunks(),
            min_size: optimize_chunk::default_min_size(),
            max_size: optimize_chunk::default_max_size(),
            test: None,
            priority: i8::default(),
        }
    }
}
/**
 * custom formatter for convert string to regex
 * @see https://serde.rs/custom-date-format.html
 */
mod optimize_test_format {
    use mako_core::regex::Regex;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(v: &Option<Regex>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(v) = v {
            serializer.serialize_str(&v.to_string())
        } else {
            serializer.serialize_none()
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Regex>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = String::deserialize(deserializer)?;

        if v.is_empty() {
            Ok(None)
        } else {
            Ok(Regex::new(v.as_str()).ok())
        }
    }
}

const CONFIG_FILE: &str = "mako.config.json";
const DEFAULT_CONFIG: &str = r#"
{
    "entry": {},
    "output": {
      "path": "dist",
      "mode": "bundle",
      "esVersion": "es2022",
      "meta": false,
      "chunkLoadingGlobal": "",
      "preserveModules": false,
      "preserveModulesRoot": "",
      "skipWrite": false
    },
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
    "mdx": false,
    "platform": "browser",
    "hmr": { "host": "127.0.0.1", "port": 3000 },
    "moduleIdStrategy": "named",
    "hash": false,
    "_treeShaking": "basic",
    "autoCSSModules": false,
    "ignoreCSSParserErrors": false,
    "dynamicImportToRequire": false,
    "writeToDisk": true,
    "transformImport": [],
    "chunkParallel": true,
    "clean": true,
    "nodePolyfill": true,
    "ignores": [],
    "optimizePackageImports": false,
    "emotion": false,
    "flexBugs": false,
    "cjs": false,
    "optimization": { "skipModules": true, "concatenateModules": true },
    "react": {
      "pragma": "React.createElement",
      "importSource": "react",
      "runtime": "automatic",
      "pragmaFrag": "React.Fragment"
    },
    "emitAssets": true,
    "cssModulesExportOnlyLocales": false,
    "inlineCSS": false,
    "rscServer": false,
    "rscClient": false,
    "experimental": { "webpackSyntaxValidate": [] },
    "watch": { "ignorePaths": [] }
}
"#;

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

pub(crate) fn get_pkg_name(root: &Path) -> Option<String> {
    let pkg_json_path = root.join("package.json");

    if pkg_json_path.exists() {
        let pkg_json = std::fs::read_to_string(pkg_json_path).unwrap();
        let pkg_json: serde_json::Value = serde_json::from_str(&pkg_json).unwrap();

        pkg_json
            .get("name")
            .map(|name| name.as_str().unwrap().to_string())
    } else {
        None
    }
}

fn get_default_chunk_loading_global(umd: Option<String>, root: &Path) -> String {
    let unique_name = umd.unwrap_or_else(|| get_pkg_name(root).unwrap_or("global".to_string()));

    format!("makoChunk_{}", unique_name)
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
