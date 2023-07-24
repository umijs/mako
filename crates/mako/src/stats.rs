extern crate prettytable;

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tracing::info;

use crate::chunk::ChunkType;
use crate::compiler::Compiler;
use crate::load::file_size;

#[derive(Debug)]
pub struct AssetsInfo {
    pub assets_type: String,
    pub size: u64,
    pub name: String,
    pub chunk_id: String,
    pub path: PathBuf,
}
#[derive(Debug)]
pub struct StatsInfo {
    // 产物信息
    pub assets: Vec<AssetsInfo>,
}
#[derive(Clone, Serialize)]
pub enum StatsJsonType {
    #[serde(rename = "type")]
    Asset(String),
    #[serde(rename = "type")]
    Module(String),
    #[serde(rename = "type")]
    Chunk(String),
}
#[derive(Serialize)]
pub struct StatsJsonAssetsItem {
    #[serde(flatten)]
    pub assets_type: StatsJsonType,
    pub size: u64,
    pub name: String,
    pub chunk_id: String,
    pub path: PathBuf,
}

#[derive(Serialize, Clone)]
pub struct StatsJsonModuleItem {
    #[serde(flatten)]
    pub module_type: StatsJsonType,
    pub size: u64,
    pub module_id: String,
    pub chunk_id: String,
}
#[derive(Serialize)]
pub struct StatsJsonChunkItem {
    #[serde(flatten)]
    pub chunk_type: StatsJsonType,
    pub chunk_id: String,
    pub files: Vec<String>,
    pub entry: bool,
    pub modules: Vec<StatsJsonModuleItem>,
}
#[derive(Serialize)]
pub struct StatsJsonMap {
    hash: u64,
    time: u128,
    built_at: u128,
    root_path: PathBuf,
    output_path: PathBuf,
    assets: Vec<StatsJsonAssetsItem>,
    modules: Vec<StatsJsonModuleItem>,
    chunks: Vec<StatsJsonChunkItem>,
}

impl StatsJsonMap {
    fn new() -> Self {
        Self {
            hash: 0,
            time: 0,
            built_at: 0,
            root_path: PathBuf::new(),
            output_path: PathBuf::new(),
            assets: vec![],
            modules: vec![],
            chunks: vec![],
        }
    }
}

impl StatsInfo {
    pub fn new() -> Self {
        Self { assets: vec![] }
    }

    pub fn add_assets(&mut self, size: u64, name: String, chunk_id: String, path: PathBuf) {
        self.assets.push(AssetsInfo {
            assets_type: "asset".to_string(),
            size,
            name,
            chunk_id,
            path,
        });
    }
}

impl Default for StatsInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
pub fn create_stats_info(compile_time: u128, compiler: &Compiler) {
    let mut stats_map = StatsJsonMap::new();
    let context = compiler.context.clone();
    // 获取当前时间
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    // 获取 hash
    let hash = compiler.full_hash();
    // 获取 root_path
    let root_path = context.root.clone();
    // 获取 output_path
    let output_path = context.config.output.path.clone();

    stats_map.built_at = now;
    stats_map.time = compile_time;
    stats_map.hash = hash;
    stats_map.root_path = root_path;
    stats_map.output_path = output_path;

    let mut stats_info = context.stats_info.lock().unwrap();

    // 把 context 中的静态资源信息加入到 stats_info 中
    compiler
        .context
        .assets_info
        .lock()
        .unwrap()
        .iter()
        .for_each(|asset| {
            let size = file_size(asset.0).unwrap();
            stats_info.add_assets(
                size,
                asset.1.clone(),
                "".to_string(),
                compiler.context.config.output.path.join(asset.1.clone()),
            );
        });

    // 获取 assets
    stats_map.assets = stats_info
        .assets
        .iter()
        .map(|asset| StatsJsonAssetsItem {
            assets_type: StatsJsonType::Asset(asset.assets_type.clone()),
            size: asset.size,
            name: asset.name.clone(),
            chunk_id: asset.chunk_id.clone(),
            path: asset.path.clone(),
        })
        .collect();

    let chunk_graph = compiler.context.chunk_graph.read().unwrap();
    let chunks = chunk_graph.get_chunks();

    // 在 chunks 中获取 modules
    let modules_vec: Rc<RefCell<Vec<StatsJsonModuleItem>>> = Rc::new(RefCell::new(Vec::new()));

    // 获取 chunks
    stats_map.chunks = chunks
        .iter()
        .map(|chunk| {
            let modules = chunk.get_modules();
            let entry = matches!(chunk.chunk_type, ChunkType::Entry);
            let id = chunk.id.id.clone();
            let chunk_modules: Vec<StatsJsonModuleItem> = modules
                .iter()
                .map(|module| {
                    let id = module.id.clone();
                    let size = file_size(&id).unwrap();
                    let module = StatsJsonModuleItem {
                        module_type: StatsJsonType::Module("module".to_string()),
                        size,
                        module_id: id,
                        chunk_id: chunk.id.id.clone(),
                    };

                    modules_vec.borrow_mut().push(module.clone());

                    module
                })
                .collect();
            let files: Vec<String> = stats_info
                .assets
                .iter()
                .filter(|asset| asset.chunk_id == id)
                .map(|asset| asset.name.clone())
                .collect();

            StatsJsonChunkItem {
                chunk_type: StatsJsonType::Chunk("chunk".to_string()),
                chunk_id: id,
                files,
                entry,
                modules: chunk_modules,
            }
        })
        .collect();

    // 获取 modules
    let modules: Vec<StatsJsonModuleItem> = modules_vec.borrow().iter().cloned().collect();

    stats_map.modules = modules;

    print_stats(stats_map, compiler);
}

pub fn print_stats(stats: StatsJsonMap, compiler: &Compiler) {
    let path = &compiler.context.root.join("stats.json");
    let stats_json = serde_json::to_string_pretty(&stats).unwrap();
    fs::write(path, stats_json).unwrap();
    info!("stats.json has been created in {:?}", path);
}

#[allow(dead_code)]
// 文件大小转换
pub fn human_readable_size(size: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
    let mut size = size as f64;
    let mut i = 0;

    while size >= 1024.0 && i < units.len() - 1 {
        size /= 1024.0;
        i += 1;
    }

    format!("{:.2} {}", size, units[i])
}
