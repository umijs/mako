use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use mako_core::anyhow::{anyhow, Error, Result};
use mako_core::colored::Colorize;
use mako_core::regex::Regex;
use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::{Globals, SourceMap, DUMMY_SP};
use mako_core::swc_ecma_ast::Ident;

use crate::chunk_graph::ChunkGraph;
use crate::comments::Comments;
use crate::config::{hash_config, Config, OutputMode};
use crate::module_graph::ModuleGraph;
use crate::optimize_chunk::OptimizeChunksInfo;
use crate::plugin::{Plugin, PluginDriver};
use crate::plugins;
use crate::plugins::minifish::Inject;
use crate::resolve::{get_resolvers, Resolvers};
use crate::stats::StatsInfo;
use crate::swc_helpers::SwcHelpers;

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub modules_with_missing_deps: RwLock<Vec<String>>,
    pub config: Config,
    pub config_hash: u64,
    pub args: Args,
    pub root: PathBuf,
    pub meta: Meta,
    pub plugin_driver: PluginDriver,
    pub stats_info: Mutex<StatsInfo>,
    pub resolvers: Resolvers,
    pub static_cache: RwLock<MemoryChunkFileCache>,
    pub optimize_infos: Mutex<Option<Vec<OptimizeChunksInfo>>>,
    pub swc_helpers: Mutex<SwcHelpers>,
}
#[derive(Default)]
pub struct MemoryChunkFileCache {
    content_map: HashMap<String, (Vec<u8>, u64)>,
    root: Option<PathBuf>,
}

impl MemoryChunkFileCache {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            content_map: HashMap::new(),
            root,
        }
    }

    pub fn write<T: AsRef<str>>(&mut self, path: T, content: Vec<u8>, hash: u64) -> Result<()> {
        let str = path.as_ref();

        if let Some((_, in_mem_hash)) = self.content_map.get(str) {
            if *in_mem_hash != hash {
                self.write_to_disk(str, &content)?;
            }
        } else {
            self.write_to_disk(str, &content)?;
        }
        self.content_map
            .insert(path.as_ref().to_string(), (content, hash));
        Ok(())
    }

    pub fn read<T: AsRef<str>>(&self, path: T) -> Option<Vec<u8>> {
        self.content_map
            .get(path.as_ref())
            .map(|(content, _)| content.clone())
    }

    fn write_to_disk<T: AsRef<str>>(&self, path: T, content: &[u8]) -> Result<()> {
        if let Some(root) = &self.root {
            let path = root.join(path.as_ref());
            fs::write(path, content)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct Args {
    pub watch: bool,
}

impl Context {
    pub fn write_static_content<T: AsRef<str>>(
        &self,
        path: T,
        content: Vec<u8>,
        hash: u64,
    ) -> Result<()> {
        let mut map = self.static_cache.write().unwrap();
        map.write(path, content, hash)
    }

    pub fn get_static_content<T: AsRef<str>>(&self, path: T) -> Option<Vec<u8>> {
        let map = self.static_cache.read().unwrap();

        map.read(path)
    }
}

impl Default for Context {
    fn default() -> Self {
        let config: Config = Default::default();
        let resolvers = get_resolvers(&config);
        let config_hash = hash_config(&config);
        Self {
            config,
            config_hash,
            args: Args { watch: false },
            root: PathBuf::from(""),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            modules_with_missing_deps: RwLock::new(Vec::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
            // 产物信息放在上下文里是否合适
            stats_info: Mutex::new(StatsInfo::new()),
            resolvers,
            optimize_infos: Mutex::new(None),
            static_cache: Default::default(),
            swc_helpers: Mutex::new(Default::default()),
        }
    }
}

pub struct Meta {
    pub script: ScriptMeta,
    pub css: CssMeta,
}

impl Meta {
    pub fn new() -> Self {
        Self {
            script: ScriptMeta::new(),
            css: CssMeta::new(),
        }
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ScriptMeta {
    pub cm: Lrc<SourceMap>,
    pub origin_comments: RwLock<Comments>,
    pub output_comments: RwLock<Comments>,
    pub globals: Globals,
    // These idents may be used in other places, such as transform_async_module
    pub module_ident: Ident,
    pub exports_ident: Ident,
    pub require_ident: Ident,
}

impl ScriptMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            origin_comments: Default::default(),
            output_comments: Default::default(),
            globals: Globals::default(),
            module_ident: build_ident("module"),
            exports_ident: build_ident("exports"),
            require_ident: build_ident("__mako_require__"),
        }
    }
}

fn build_ident(ident: &str) -> Ident {
    Ident {
        span: DUMMY_SP,
        sym: ident.into(),
        optional: false,
    }
}

pub struct CssMeta {
    pub cm: Lrc<SourceMap>,
    pub globals: Globals,
}

impl CssMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            globals: Globals::default(),
        }
    }
}

impl Context {
    pub fn emit_assets(&self, origin_path: String, output_path: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(origin_path, output_path);
        drop(assets_info);
    }
}

pub struct Compiler {
    pub context: Arc<Context>,
}

impl Compiler {
    pub fn new(
        config: Config,
        root: PathBuf,
        args: Args,
        extra_plugins: Option<Vec<Arc<dyn Plugin>>>,
    ) -> Result<Self> {
        assert!(root.is_absolute(), "root path must be absolute");

        // why add plugins before builtin plugins?
        // because plugins like less-loader need to be added before assets plugin
        // TODO: support plugin orders
        let mut plugins: Vec<Arc<dyn Plugin>> = vec![];
        if let Some(extra_plugins) = extra_plugins {
            plugins.extend(extra_plugins);
        }
        let builtin_plugins: Vec<Arc<dyn Plugin>> = vec![
            // features
            Arc::new(plugins::manifest::ManifestPlugin {}),
            Arc::new(plugins::copy::CopyPlugin {}),
            Arc::new(plugins::import::ImportPlugin {}),
            // file types
            Arc::new(plugins::css::CSSPlugin {}),
            Arc::new(plugins::context_module::ContextModulePlugin {}),
            Arc::new(plugins::javascript::JavaScriptPlugin {}),
            Arc::new(plugins::json::JSONPlugin {}),
            Arc::new(plugins::md::MdPlugin {}),
            Arc::new(plugins::svg::SVGPlugin {}),
            Arc::new(plugins::toml::TOMLPlugin {}),
            Arc::new(plugins::wasm::WASMPlugin {}),
            Arc::new(plugins::xml::XMLPlugin {}),
            Arc::new(plugins::yaml::YAMLPlugin {}),
            Arc::new(plugins::assets::AssetsPlugin {}),
            Arc::new(plugins::runtime::MakoRuntime {}),
            Arc::new(plugins::farm_tree_shake::FarmTreeShake {}),
            Arc::new(plugins::invalid_syntax::InvalidSyntaxPlugin {}),
            Arc::new(plugins::hmr_runtime::HMRRuntimePlugin {}),
            Arc::new(plugins::wasm_runtime::WasmRuntimePlugin {}),
            Arc::new(plugins::async_runtime::AsyncRuntimePlugin {}),
        ];
        plugins.extend(builtin_plugins);

        let mut config = config;

        if config.node_polyfill {
            plugins.push(Arc::new(plugins::node_polyfill::NodePolyfillPlugin {}));
        }

        if config.output.mode == OutputMode::Bundless {
            plugins.insert(0, Arc::new(plugins::bundless_compiler::BundlessCompiler {}));
        }

        if let Some(minifish_config) = &config._minifish {
            let inject = if let Some(inject) = &minifish_config.inject {
                let mut map = HashMap::new();

                for (k, ii) in inject.iter() {
                    let exclude = if let Some(exclude) = &ii.exclude {
                        if let Ok(regex) = Regex::new(exclude) {
                            Some(regex)
                        } else {
                            return Err(anyhow!("Config Error invalid regex: {}", exclude));
                        }
                    } else {
                        None
                    };

                    map.insert(
                        k.clone(),
                        Inject {
                            from: ii.from.clone(),
                            name: k.clone(),
                            named: ii.named.clone(),
                            namespace: ii.namespace,
                            exclude,
                            prefer_require: ii.prefer_require.map_or(false, |v| v),
                        },
                    );
                }
                Some(map)
            } else {
                None
            };

            plugins.insert(
                0,
                Arc::new(plugins::minifish::MinifishPlugin {
                    mapping: minifish_config.mapping.clone(),
                    meta_path: minifish_config.meta_path.clone(),
                    inject,
                }),
            );
        }

        if !config.ignores.is_empty() {
            let ignore_regex = config
                .ignores
                .iter()
                .map(|ignore| Regex::new(ignore).map_err(Error::new))
                .collect::<Result<Vec<Regex>>>()?;

            plugins.push(Arc::new(plugins::ignore::IgnorePlugin {
                ignores: ignore_regex,
            }))
        }

        let plugin_driver = PluginDriver::new(plugins);

        plugin_driver.modify_config(&mut config, &root, &args)?;

        let resolvers = get_resolvers(&config);
        let is_watch = args.watch;
        Ok(Self {
            context: Arc::new(Context {
                static_cache: if config.write_to_disk {
                    RwLock::new(MemoryChunkFileCache::new(Some(config.output.path.clone())))
                } else {
                    Default::default()
                },
                config_hash: hash_config(&config),
                config,
                args,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
                assets_info: Mutex::new(HashMap::new()),
                modules_with_missing_deps: RwLock::new(Vec::new()),
                meta: Meta::new(),
                plugin_driver,
                stats_info: Mutex::new(StatsInfo::new()),
                resolvers,
                optimize_infos: Mutex::new(None),
                swc_helpers: Mutex::new(SwcHelpers::new(if is_watch {
                    Some(SwcHelpers::full_helpers())
                } else {
                    None
                })),
            }),
        })
    }

    pub fn compile(&self) -> Result<()> {
        // 先清空 dist 目录
        if self.context.config.clean {
            self.clean_dist();
        }

        let t_compiler = Instant::now();
        let is_prod = self.context.config.mode == crate::config::Mode::Production;
        let building_with_message = format!(
            "Building with {} for {}...",
            "mako".to_string().cyan(),
            if is_prod { "production" } else { "development" }
        )
        .green();
        println!("{}", building_with_message);
        {
            mako_core::mako_profile_scope!("Build Stage");
            self.build()?;
        }
        let result = {
            mako_core::mako_profile_scope!("Generate Stage");
            self.generate()
        };
        let t_compiler = t_compiler.elapsed();
        if result.is_ok() {
            println!(
                "{}",
                format!(
                    "✓ Built in {}",
                    format!("{}ms", t_compiler.as_millis()).bold()
                )
                .green()
            );
            if !self.context.args.watch {
                println!("{}", "Complete!".bold());
            }
            Ok(())
        } else {
            result
        }
    }

    pub fn full_hash(&self) -> u64 {
        mako_core::mako_profile_function!();
        let cg = self.context.chunk_graph.read().unwrap();
        let mg = self.context.module_graph.read().unwrap();

        cg.full_hash(&mg)
    }

    pub fn clean_dist(&self) {
        // compiler 前清除 dist，如果后续 dev 环境不在 output_path 里，需要再补上 dev 的逻辑
        let output_path = &self.context.config.output.path;
        if fs::metadata(output_path).is_ok() {
            fs::remove_dir_all(output_path).unwrap();
        }
    }
}
