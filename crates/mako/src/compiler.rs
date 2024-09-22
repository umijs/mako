use std::collections::HashMap;
use std::fs;
use std::hash::Hasher;
use std::ops::Deref;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Error, Result};
use colored::Colorize;
use r2d2::Pool;
use r2d2_sqlite::rusqlite::Connection;
use r2d2_sqlite::SqliteConnectionManager;
use regex::Regex;
use swc_core::common::sync::Lrc;
use swc_core::common::util::take::Take;
use swc_core::common::{Globals, SourceMap, DUMMY_SP};
use swc_core::ecma::ast::Ident;
use swc_node_comments::SwcComments;
use tracing::debug;

use crate::ast::comments::Comments;
use crate::config::{Config, OutputMode};
use crate::generate::chunk_graph::ChunkGraph;
use crate::generate::optimize_chunk::OptimizeChunksInfo;
use crate::module_graph::ModuleGraph;
use crate::plugin::{Plugin, PluginDriver, PluginGenerateEndParams};
use crate::resolve::{get_resolvers, Resolvers};
use crate::stats::StatsInfo;
use crate::utils::{thread_pool, ParseRegex};
use crate::{mako_profile_function, plugins};

pub struct Context {
    pub module_graph: RwLock<ModuleGraph>,
    pub chunk_graph: RwLock<ChunkGraph>,
    pub assets_info: Mutex<HashMap<String, String>>,
    pub modules_with_missing_deps: RwLock<Vec<String>>,
    pub config: Config,
    pub args: Args,
    pub root: PathBuf,
    pub meta: Meta,
    pub plugin_driver: PluginDriver,
    pub stats_info: StatsInfo,
    pub resolvers: Resolvers,
    pub static_cache: RwLock<MemoryChunkFileCache>,
    pub optimize_infos: Mutex<Option<Vec<OptimizeChunksInfo>>>,
    pub cache: SimpleCache,
}

type CacheData = (String, u64, Vec<u8>, Vec<u8>);

pub struct SimpleCache {
    pub sender: Sender<CacheData>,
    pub pool: Pool<SqliteConnectionManager>,
}

const SIZE_8M: usize = 8 * 1024 * 1024;

fn db_writer_thread(rx: Receiver<CacheData>) {
    let mut conn = r2d2_sqlite::rusqlite::Connection::open("mako_cache.db").unwrap();
    let mut batch = Vec::new();
    let mut buffered_bytes: usize = 0;
    let mut last_insert = Instant::now();

    loop {
        while let Ok(data) = rx.recv_timeout(Duration::from_secs(30)) {
            buffered_bytes += data.2.len() + data.3.len();
            batch.push(data);

            if buffered_bytes >= SIZE_8M {
                break; // 批量插入的最大数目
            }
        }

        if buffered_bytes < SIZE_8M && last_insert.elapsed() < Duration::from_secs(60) {
            continue;
        }

        // 如果有数据，插入到数据库
        if !batch.is_empty() {
            match insert_into_db(&mut conn, batch.take()) {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to insert batch: {}", e),
            }

            buffered_bytes = 0;
            last_insert = Instant::now();
        }
    }
}

fn insert_into_db(conn: &mut Connection, data: Vec<CacheData>) -> Result<()> {
    mako_profile_function!(data.len().to_string());

    let tx = conn.transaction()?; // 开启事务
    {
        let last_modified = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let mut stmt = tx.prepare(
            "INSERT INTO cache (file_path, file_hash, module_data, deps_data, last_modified) 
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(file_path, file_hash) 
             DO UPDATE SET module_data=excluded.module_data, deps_data=excluded.deps_data, last_modified=excluded.last_modified;"
        )?;

        // 批量插入数据
        for cache in data {
            stmt.execute((
                cache.0,
                cache.1.to_string(),
                cache.2,
                cache.3,
                last_modified,
            ))?;
        }
    }
    tx.commit()?; // 提交事务
    Ok(())
}

fn init_db(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "
    CREATE TABLE IF NOT EXISTS cache (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        file_path        TEXT        NOT NULL,
        file_hash        TEXT        NOT NULL,
        module_data      BLOB        NOT NULL,
        deps_data        BLOB        NOT NULL,
        last_modified    INTEGER     NOT NULL,
        UNIQUE(file_path, file_hash)
    );
    
        PRAGMA journal_mode = WAL;
        PRAGMA wal_autocheckpoint = 10000;
        PRAGMA synchronous = OFF;
        PRAGMA cache_size = -20000;
        PRAGMA temp_store = MEMORY;
        PRAGMA read_uncommitted = 1;
    ",
    )?;
    Ok(())
}

impl SimpleCache {
    fn new() -> Self {
        let manager = SqliteConnectionManager::file("mako_cache.db");
        let pool = r2d2::Pool::new(manager).unwrap();

        let conn = pool.get().unwrap();
        init_db(conn.deref()).unwrap();

        let (sender, receiver) = channel::<CacheData>();

        std::thread::spawn(move || {
            db_writer_thread(receiver);
        });

        SimpleCache { pool, sender }
    }

    pub fn get_cached(&self, file_path: &str, file_hash: u64) -> Option<(Vec<u8>, Vec<u8>)> {
        mako_profile_function!(file_path);
        let start = Instant::now();
        let connection = self.pool.get().unwrap();

        let mut statement = connection
            .prepare(
                "
        SELECT module_data, deps_data FROM cache WHERE file_path = ? AND file_hash = ?;
        ",
            )
            .ok()?;

        let mut rows = statement
            .query((file_path, file_hash.to_string().as_str()))
            .ok()?;

        if let Some(row) = rows.next().ok()? {
            let module_data: Vec<u8> = row.get(0).ok().unwrap();
            let deps_data: Vec<u8> = row.get(1).ok().unwrap();
            let elapsed = start.elapsed();

            debug!(
                "{file_path} takes  {} ({} {})",
                elapsed.as_millis(),
                module_data.len(),
                deps_data.len()
            );

            return Some((module_data, deps_data));
        }
        None
    }

    pub fn insert(
        &self,
        file_path: String,
        file_hash: u64,
        serialized_data: Vec<u8>,
        deps_data: Vec<u8>,
    ) -> Result<()> {
        self.sender
            .send((file_path, file_hash, serialized_data, deps_data))?;
        Ok(())
    }

    pub fn insert_sql(
        &self,
        file_path: &str,
        file_hash: u64,
        serialized_data: &[u8],
    ) -> Result<()> {
        let last_modified = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let con = self.pool.get().unwrap();

        let mut statement  = con
            .prepare(
                "
        INSERT INTO cache (file_path, file_hash, serialized_data, last_modified) 
        VALUES (?, ?, ?, ?)
        ON CONFLICT(file_path, file_hash) 
        DO UPDATE SET serialized_data=excluded.serialized_data, last_modified=excluded.last_modified;
        ",
            )?;

        statement.execute((
            file_path,
            file_hash.to_string().as_str(),
            serialized_data,
            last_modified,
        ))?;

        Ok(())
    }
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
            cache: SimpleCache::new(),
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
            Arc::new(plugins::context_module::ContextModulePlugin {}),
            Arc::new(plugins::runtime::MakoRuntime {}),
            Arc::new(plugins::invalid_webpack_syntax::InvalidWebpackSyntaxPlugin {}),
            Arc::new(plugins::hmr_runtime::HMRRuntimePlugin {}),
            Arc::new(plugins::ensure_2::Ensure2 {}),
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

        if !config.ignores.is_empty() {
            let ignores = config
                .ignores
                .iter()
                .map(|ignore| Regex::new(ignore).map_err(Error::new))
                .collect::<Result<Vec<Regex>>>()?;
            plugins.push(Arc::new(plugins::ignore::IgnorePlugin { ignores }))
        }

        let plugin_driver = PluginDriver::new(plugins);

        plugin_driver.modify_config(&mut config, &root, &args)?;

        let resolvers = get_resolvers(&config);
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
                stats_info: StatsInfo::new(),
                resolvers,
                optimize_infos: Mutex::new(None),
                cache: SimpleCache::new(),
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
