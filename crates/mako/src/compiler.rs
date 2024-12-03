use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use anyhow::{anyhow, Error, Result};
use colored::Colorize;
use regex::Regex;
use swc_core::common::sync::Lrc;
use swc_core::common::{Globals, SourceMap, DUMMY_SP};
use swc_core::ecma::ast::Ident;
use swc_node_comments::SwcComments;
use tracing::debug;

use crate::ast::comments::Comments;
use crate::ast::file::win_path;
use crate::config::{Config, ModuleIdStrategy, OutputMode};
use crate::generate::chunk_graph::ChunkGraph;
use crate::generate::optimize_chunk::OptimizeChunksInfo;
use crate::module_graph::ModuleGraph;
use crate::plugin::{Plugin, PluginDriver, PluginGenerateEndParams};
use crate::plugins;
use crate::resolve::{get_resolvers, Resolvers};
use crate::share::helpers::SWC_HELPERS;
use crate::stats::StatsInfo;
use crate::utils::id_helper::{assign_numeric_ids, compare_modules_by_incoming_edges};
use crate::utils::{thread_pool, ParseRegex};

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub modules_with_missing_deps: RwLock<Vec<String>>,
    pub config: Config,
    pub numeric_ids_map: RwLock<HashMap<String, usize>>,
    pub args: Args,
    pub root: PathBuf,
    pub meta: Meta,
    pub plugin_driver: PluginDriver,
    pub stats_info: StatsInfo,
    pub resolvers: Resolvers,
    pub static_cache: RwLock<MemoryChunkFileCache>,
    pub optimize_infos: Mutex<Option<Vec<OptimizeChunksInfo>>>,
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
        let mut numeric_ids_map = HashMap::new();
        SWC_HELPERS.iter().enumerate().for_each(|(i, item)| {
            numeric_ids_map.insert(item.to_string(), i);
        });
        let config: Config = Default::default();
        let resolvers = get_resolvers(&config);
        Self {
            config,
            args: Args { watch: false },
            root: PathBuf::from(""),
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            modules_with_missing_deps: RwLock::new(Vec::new()),
            meta: Meta::new(),
            plugin_driver: Default::default(),
            stats_info: StatsInfo::new(),
            resolvers,
            optimize_infos: Mutex::new(None),
            static_cache: Default::default(),
            numeric_ids_map: RwLock::new(numeric_ids_map),
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
        ctxt: Default::default(),
        span: DUMMY_SP,
        sym: ident.into(),
        optional: false,
    }
}

pub struct CssMeta {
    pub cm: Lrc<SourceMap>,
    pub globals: Globals,
    pub comments: SwcComments,
}

impl CssMeta {
    fn new() -> Self {
        Self {
            cm: Default::default(),
            globals: Globals::default(),
            comments: Default::default(),
        }
    }
}

impl Context {
    pub fn emit_assets(&self, origin_path: String, output_path: String) {
        let mut assets_info = self.assets_info.lock().unwrap();
        assets_info.insert(origin_path, output_path);
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
        if !root.is_absolute() {
            return Err(anyhow!("root path must be absolute"));
        }

        let root = PathBuf::from(win_path(root.to_str().unwrap()));

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
            Arc::new(plugins::case_sensitive::CaseSensitivePlugin::new()),
            Arc::new(plugins::context_module::ContextModulePlugin {}),
            Arc::new(plugins::runtime::MakoRuntime {}),
            Arc::new(plugins::invalid_webpack_syntax::InvalidWebpackSyntaxPlugin {}),
            Arc::new(plugins::hmr_runtime::HMRRuntimePlugin {}),
            Arc::new(plugins::wasm_runtime::WasmRuntimePlugin {}),
            Arc::new(plugins::async_runtime::AsyncRuntimePlugin {}),
            Arc::new(plugins::emotion::EmotionPlugin {}),
            Arc::new(plugins::tree_shaking::FarmTreeShake {}),
            Arc::new(plugins::detect_circular_dependence::LoopDetector {}),
        ];
        plugins.extend(builtin_plugins);

        let mut config = config;

        if let Some(progress) = &config.progress {
            plugins.push(Arc::new(plugins::progress::ProgressPlugin::new(
                plugins::progress::ProgressPluginOptions {
                    prefix: "Mako".to_string(),
                    template:
                        "● {prefix:.bold} {bar:25.green/white.dim} ({percent}%) {wide_msg:.dim}"
                            .to_string(),
                    progress_chars: progress.progress_chars.clone(),
                },
            )));
        }

        if let Some(duplicate_package_checker) = &config.check_duplicate_package {
            plugins.push(Arc::new(
                plugins::duplicate_package_checker::DuplicatePackageCheckerPlugin::new()
                    .show_help(duplicate_package_checker.show_help)
                    .emit_error(duplicate_package_checker.emit_error)
                    .verbose(duplicate_package_checker.verbose),
            ));
        }

        if config.experimental.require_context {
            plugins.push(Arc::new(plugins::require_context::RequireContextPlugin {}))
        }

        if config.output.mode == OutputMode::Bundless {
            plugins.insert(
                0,
                Arc::new(plugins::bundless_compiler::BundlessCompilerPlugin {}),
            );
        }

        if std::env::var("DEBUG_GRAPH").is_ok_and(|v| v == "true") {
            plugins.push(Arc::new(plugins::graphviz::Graphviz {}));
        }

        if args.watch && std::env::var("SSU").is_ok_and(|v| v == "true") {
            plugins.push(Arc::new(plugins::ssu::SUPlus::new()));
        }

        if args.watch && config.experimental.central_ensure {
            plugins.push(Arc::new(plugins::central_ensure::CentralChunkEnsure {}));
        }

        if let Some(minifish_config) = &config._minifish {
            let inject = if let Some(inject) = &minifish_config.inject {
                let mut map = HashMap::new();

                for (k, ii) in inject.iter() {
                    map.insert(
                        k.clone(),
                        plugins::minifish::Inject {
                            from: ii.from.clone(),
                            name: k.clone(),
                            named: ii.named.clone(),
                            namespace: ii.namespace,
                            exclude: ii.exclude.parse_into_regex()?,
                            include: ii.include.parse_into_regex()?,
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

        let ignores = config
            .ignores
            .iter()
            .map(|ignore| Regex::new(ignore).map_err(Error::new))
            .collect::<Result<Vec<Regex>>>()?;
        plugins.push(Arc::new(plugins::ignore::IgnorePlugin { ignores }));

        let plugin_driver = PluginDriver::new(plugins);

        plugin_driver.modify_config(&mut config, &root, &args)?;

        let resolvers = get_resolvers(&config);
        let mut numeric_ids_map = HashMap::new();
        SWC_HELPERS.iter().enumerate().for_each(|(i, item)| {
            numeric_ids_map.insert(item.to_string(), i);
        });
        Ok(Self {
            context: Arc::new(Context {
                static_cache: if config.write_to_disk {
                    RwLock::new(MemoryChunkFileCache::new(Some(config.output.path.clone())))
                } else {
                    Default::default()
                },
                config,
                args,
                root,
                module_graph: RwLock::new(ModuleGraph::new()),
                chunk_graph: RwLock::new(ChunkGraph::new()),
                assets_info: Mutex::new(HashMap::new()),
                modules_with_missing_deps: RwLock::new(Vec::new()),
                meta: Meta::new(),
                plugin_driver,
                numeric_ids_map: RwLock::new(numeric_ids_map),
                stats_info: StatsInfo::new(),
                resolvers,
                optimize_infos: Mutex::new(None),
            }),
        })
    }

    pub fn compile(&self) -> Result<()> {
        // 先清空 dist 目录
        if self.context.config.clean {
            self.clean_dist()?;
        }

        let t_compiler = Instant::now();
        let start_time = chrono::Local::now().timestamp_millis();
        let building_with_message = format!(
            "Building with {} for {}...",
            "mako".to_string().cyan(),
            self.context.config.mode
        )
        .green();
        println!("{}", building_with_message);
        {
            crate::mako_profile_scope!("Build Stage");
            let files = self
                .context
                .config
                .entry
                .values()
                .map(|entry| {
                    let mut entry = entry.to_string_lossy().to_string();
                    let is_browser = matches!(
                        self.context.config.platform,
                        crate::config::Platform::Browser
                    );
                    let watch = self.context.args.watch;
                    let hmr = self.context.config.hmr.is_some();
                    if is_browser && watch && hmr {
                        entry = format!("{}?hmr", entry);
                    }
                    crate::ast::file::File::new_entry(entry, self.context.clone())
                })
                .collect();
            self.context.plugin_driver.build_start(&self.context)?;

            self.build(files)?;

            debug!("start after build");

            self.context
                .plugin_driver
                .after_build(&self.context, self)?;
        }

        self.context.plugin_driver.before_generate(&self.context)?;

        if let ModuleIdStrategy::Numeric = self.context.config.module_id_strategy {
            let module_graph = self.context.module_graph.read().unwrap();
            assign_numeric_ids(
                module_graph.modules(),
                |a, b| compare_modules_by_incoming_edges(&module_graph, &a.id, &b.id),
                |module, id| {
                    let mut numeric_ids_map = self.context.numeric_ids_map.write().unwrap();
                    // reserved ten indexes for swc helper and others runtime module
                    numeric_ids_map.insert(module.id.id.clone(), id + 10);
                },
            )
        }

        let result = {
            crate::mako_profile_scope!("Generate Stage");
            // need to put all rayon parallel iterators run in the existed scope, or else rayon
            // will create a new thread pool for those parallel iterators
            thread_pool::scope(|_| self.generate())
        };
        let t_compiler_duration = t_compiler.elapsed();
        match result {
            Ok(mut stats) => {
                stats.start_time = start_time;
                stats.end_time = chrono::Local::now().timestamp_millis();
                println!(
                    "{}",
                    format!(
                        "✓ Built in {}",
                        format!("{}ms", t_compiler_duration.as_millis()).bold()
                    )
                    .green()
                );
                if !self.context.args.watch {
                    println!("{}", "Complete!".bold());
                }
                let params = PluginGenerateEndParams {
                    is_first_compile: true,
                    time: t_compiler.elapsed().as_millis() as i64,
                    stats,
                };
                self.context
                    .plugin_driver
                    .generate_end(&params, &self.context)?;
                self.context.plugin_driver.write_bundle(&self.context)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn full_hash(&self) -> u64 {
        crate::mako_profile_function!();
        let cg = self.context.chunk_graph.read().unwrap();
        let mg = self.context.module_graph.read().unwrap();
        cg.full_hash(&mg)
    }

    fn clean_dist(&self) -> Result<()> {
        // compiler 前清除 dist，如果后续 dev 环境不在 output_path 里，需要再补上 dev 的逻辑
        let output_path = &self.context.config.output.path;
        if fs::metadata(output_path).is_ok() {
            fs::remove_dir_all(output_path)?;
        }
        Ok(())
    }
}
