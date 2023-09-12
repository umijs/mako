use std::cell::RefCell;
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use colored::*;
use pathdiff::diff_paths;
use serde::Serialize;

use crate::chunk::ChunkType;
use crate::compiler::Compiler;
use crate::load::file_size;

#[derive(Debug, PartialEq, Eq)]
// name 记录实际 filename , 用在 stats.json 中, hashname 用在产物描述和 manifest 中
pub struct AssetsInfo {
    pub assets_type: String,
    pub size: u64,
    pub name: String,
    pub hashname: String,
    pub chunk_id: String,
    pub path: PathBuf,
}

impl Ord for AssetsInfo {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for AssetsInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug)]
pub struct StatsInfo {
    // 产物信息
    pub assets: Vec<AssetsInfo>,
}
#[derive(Clone, Serialize, Debug)]
pub enum StatsJsonType {
    #[serde(rename = "type")]
    Asset(String),
    #[serde(rename = "type")]
    Module(String),
    #[serde(rename = "type")]
    Chunk(String),
}
#[derive(Serialize, Debug)]
pub struct StatsJsonAssetsItem {
    #[serde(flatten)]
    pub assets_type: StatsJsonType,
    pub size: u64,
    pub name: String,
    pub path: PathBuf,
}

#[derive(Serialize, Clone, Debug)]
pub struct StatsJsonModuleItem {
    #[serde(flatten)]
    pub module_type: StatsJsonType,
    pub size: u64,
    pub module_id: String,
    pub chunk_id: String,
}
#[derive(Serialize, Debug)]
pub struct StatsJsonChunkItem {
    #[serde(flatten)]
    pub chunk_type: StatsJsonType,
    pub chunk_id: String,
    pub files: Vec<String>,
    pub entry: bool,
    pub modules: Vec<StatsJsonModuleItem>,
}
#[derive(Serialize, Debug)]
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

    pub fn add_assets(
        &mut self,
        size: u64,
        name: String,
        chunk_id: String,
        path: PathBuf,
        hashname: String,
    ) {
        self.assets.push(AssetsInfo {
            assets_type: "asset".to_string(),
            size,
            name,
            chunk_id,
            path,
            hashname,
        });
    }
}

impl Default for StatsInfo {
    fn default() -> Self {
        Self::new()
    }
}

pub fn create_stats_info(compile_time: u128, compiler: &Compiler) -> StatsJsonMap {
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
                asset.1.clone(),
            );
        });

    // 获取 assets
    println!("{:?}", stats_info.assets);
    stats_map.assets = stats_info
        .assets
        .iter()
        .map(|asset| StatsJsonAssetsItem {
            assets_type: StatsJsonType::Asset(asset.assets_type.clone()),
            size: asset.size,
            name: asset.hashname.clone(),
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
            let entry = matches!(chunk.chunk_type, ChunkType::Entry(_));
            let id = chunk.id.id.clone();
            let chunk_modules: Vec<StatsJsonModuleItem> = modules
                .iter()
                .filter(|module| {
                    // ?modules 是虚拟模块，暂不记录
                    // TODO: 支持虚拟模块属性，同时增加 content 以用于 size 计算等用途
                    !module.id.contains("?modules")
                })
                .map(|module| {
                    let id = module.id.clone();
                    // 去拿 module 的文件 size 时，有可能 module 不存在，size 则设为 0
                    // 场景: xlsx 中引入了 fs 模块
                    let size = match file_size(&id) {
                        Ok(size) => size,
                        Err(..) => 0,
                    };
                    let module = StatsJsonModuleItem {
                        module_type: StatsJsonType::Module("module".to_string()),
                        size,
                        module_id: id,
                        // TODO: 现在是从每个 chunk 中找到包含的 module, 所以 chunk_id 是单个, 但是一个 module 有可能存在于多个 chunk 中, 后续需要把 chunk_id 改成 Vec
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

    stats_map
}

pub fn write_stats(stats: &StatsJsonMap, compiler: &Compiler) {
    let path = &compiler.context.config.output.path.join("stats.json");
    let stats_json = serde_json::to_string_pretty(stats).unwrap();
    fs::write(path, stats_json).unwrap();
}

// 文件大小转换
pub fn human_readable_size(size: u64) -> String {
    let units = ["kB", "mB", "gB"];
    // 把 B 转为 KB
    let mut size = (size as f64) / 1000.0;
    let mut i = 0;

    while size >= 1000.0 && i < units.len() - 1 {
        size /= 1000.0;
        i += 1;
    }

    format!("{:.2} {}", size, units[i])
}

fn pad_string(text: &str, max_length: usize, front: bool) -> String {
    let mut padded_text = String::from(text);
    let pad_length = max_length - text.chars().count();
    if front {
        let mut s = String::new();
        s.push_str(&" ".repeat(pad_length));
        s.push_str(text);
        s
    } else {
        padded_text.push_str(&" ".repeat(pad_length));
        padded_text
    }
}

pub fn print_stats(compiler: &Compiler) {
    let assets = &mut compiler.context.stats_info.lock().unwrap().assets;
    // 按照产物名称排序
    assets.sort();
    // 产物路径需要按照 output.path 来
    let abs_path = &compiler.context.root;
    let output_path = &compiler.context.config.output.path;
    let dist_path = diff_paths(output_path, abs_path).unwrap_or_else(|| output_path.clone());
    let mut path_str = dist_path.to_str().unwrap().to_string();
    if !path_str.ends_with('/') {
        path_str.push('/');
    }
    let dist = path_str.truecolor(128, 128, 128);

    // 最长的文件名字, size长度, map_size长度, 后续保持输出整齐
    let mut max_length_name = String::new();
    let mut max_size = 0;
    let mut max_map_size = 0;
    // 记录 name size map_size 的数组
    let mut assets_vec: Vec<(String, u64, u64)> = vec![];

    // 生成 (name, size, map_size) 的 vec
    for asset in assets {
        let name = asset.hashname.clone();
        let size_length = human_readable_size(asset.size).chars().count();
        // 记录较长的名字
        if name.chars().count() > max_length_name.chars().count() {
            max_length_name = name.clone();
        }

        // 如果是 .map 文件判断是否是上一个的文件的 sourceMap
        // 前面排序过了, sourceMap 一定 js/css 在后面
        if name.ends_with(".map") {
            let len = assets_vec.len();
            if let Some(last) = assets_vec.get_mut(len - 1) {
                if name == format!("{}.map", last.0) {
                    // 记录较长的 map_size
                    if size_length > max_map_size {
                        max_map_size = size_length;
                    }
                    *last = (last.0.clone(), last.1, asset.size);
                    continue;
                }
            }
        }
        // 记录较长的 size
        if size_length > max_size {
            max_size = size_length;
        }
        assets_vec.push((asset.hashname.clone(), asset.size, 0));
    }

    // 输出 stats
    let mut s = String::new();
    for asset in assets_vec {
        let file_name = format!("{}{}", dist, asset.0);
        let length = format!("{}{}", dist, max_length_name).chars().count() + 2;
        let file_name_str: String = pad_string(&file_name, length, false);
        let color_file_name_str = match file_name {
            s if s.ends_with(".js") => file_name_str.cyan(),
            s if s.ends_with(".css") => file_name_str.magenta(),
            _ => file_name_str.green(),
        };
        // 没有 map 的输出
        if asset.2 == 0 {
            let size = human_readable_size(asset.1);
            s.push_str(
                format!(
                    "{} {}\n",
                    color_file_name_str,
                    pad_string(&size, max_size, true),
                )
                .as_str(),
            );
        } else {
            // 有 map 的输出, | map: map_size
            let size = human_readable_size(asset.1);
            let map_size = human_readable_size(asset.2);
            s.push_str(
                format!(
                    "{} {} {} {}\n",
                    color_file_name_str,
                    pad_string(&size, max_size, true)
                        .truecolor(128, 128, 128)
                        .bold(),
                    "│ map:".truecolor(128, 128, 128),
                    pad_string(&map_size, max_map_size, true).truecolor(128, 128, 128)
                )
                .as_str(),
            );
        }
    }

    println!("{}", s.trim_end_matches('\n'));
}
