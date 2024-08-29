use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use colored::*;
use indexmap::IndexMap;
use pathdiff::diff_paths;
use serde::Serialize;
use swc_core::common::source_map::SmallPos;

use crate::compiler::{Compiler, Context};
use crate::features::rsc::{RscClientInfo, RscCssModules};
use crate::generate::chunk::ChunkType;

impl Compiler {
    pub fn create_stats_info(&self) -> StatsJsonMap {
        let mut stats_map = StatsJsonMap::new();
        let context = self.context.clone();

        // 获取 hash
        let hash = self.full_hash();
        // 获取 root_path
        let root_path = context.root.to_string_lossy().to_string();
        // 获取 output_path
        let output_path = context.config.output.path.to_string_lossy().to_string();

        stats_map.built_at = chrono::Local::now().timestamp_millis();
        stats_map.hash = hash;
        stats_map.root_path = root_path;
        stats_map.output_path = output_path;

        let stats_info = &context.stats_info;

        // 把 context 中的静态资源信息加入到 stats_info 中
        self.context
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
                    self.context
                        .config
                        .output
                        .path
                        .join(asset.1.clone())
                        .to_string_lossy()
                        .to_string(),
                    asset.1.clone(),
                );
            });

        // 获取 assets
        stats_map.assets = stats_info
            .get_assets()
            .iter()
            .map(|asset| StatsJsonAssetsItem {
                assets_type: StatsJsonType::Asset(asset.assets_type.clone()),
                size: asset.size,
                name: asset.hashname.clone(),
                path: asset.path.clone(),
            })
            .collect();

        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let module_graph = self.context.module_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();

        // 在 chunks 中获取 modules
        let mut chunk_modules: Vec<StatsJsonChunkModuleItem> = Vec::new();

        // 获取 chunks
        stats_map.chunks = chunks
            .iter()
            .map(|chunk| {
                let modules = chunk.get_modules();
                let entry = matches!(chunk.chunk_type, ChunkType::Entry(_, _, _));
                let id = chunk.id.id.clone();
                let chunk_modules: Vec<StatsJsonChunkModuleItem> = modules
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
                        let size = file_size(&id).unwrap_or_default();
                        let module = StatsJsonChunkModuleItem {
                            module_type: StatsJsonType::Module("module".to_string()),
                            size,
                            id,
                            // TODO: 现在是从每个 chunk 中找到包含的 module, 所以 chunk_id 是单个, 但是一个 module 有可能存在于多个 chunk 中
                            chunks: vec![chunk.id.id.clone()],
                        };
                        chunk_modules.push(module.clone());
                        module
                    })
                    .collect();
                let files: Vec<String> = stats_info
                    .get_assets()
                    .iter()
                    .filter(|asset| asset.chunk_id == id)
                    .map(|asset| asset.hashname.clone())
                    .collect();
                let siblings = chunk_graph
                    .sync_dependencies_chunk(&chunk.id)
                    .iter()
                    .map(|id| id.id.clone())
                    .collect::<Vec<_>>();
                let origin_chunk_modules = match chunk.chunk_type {
                    // sync chunk is the common dependency of async chunk
                    // so the origin chunk module within its dependent async chunk rather than itself
                    ChunkType::Sync => chunk_graph
                        .dependents_chunk(&chunk.id)
                        .iter()
                        .filter_map(|chunk_id| {
                            chunk_graph.chunk(chunk_id).unwrap().modules.iter().last()
                        })
                        .collect::<Vec<_>>(),
                    _ => vec![chunk.modules.iter().last().unwrap()],
                };
                let mut origins_set = IndexMap::new();
                for origin_chunk_module in origin_chunk_modules {
                    let origin_deps = module_graph.get_dependents(origin_chunk_module);

                    for (id, dep) in origin_deps {
                        let unique_key = format!("{}:{}", id.id, dep.source);

                        if !origins_set.contains_key(&unique_key) {
                            origins_set.insert(
                                unique_key,
                                StatsJsonChunkOriginItem {
                                    module: id.id.clone(),
                                    module_identifier: id.id.clone(),
                                    module_name: module_graph
                                        .get_module(id)
                                        .and_then(|module| {
                                            module.info.as_ref().map(|info| {
                                                info.file.path.to_string_lossy().to_string()
                                            })
                                        })
                                        .unwrap_or("".to_string()),
                                    // -> "lo-hi"
                                    loc: dep
                                        .span
                                        .map(|span| {
                                            format!("{}-{}", span.lo.to_u32(), span.hi.to_u32())
                                        })
                                        .unwrap_or("".to_string()),
                                    request: dep.source.clone(),
                                },
                            );
                        }
                    }
                }
                let origins = origins_set.into_values().collect::<Vec<_>>();

                StatsJsonChunkItem {
                    chunk_type: StatsJsonType::Chunk("chunk".to_string()),
                    id,
                    files,
                    entry,
                    modules: chunk_modules,
                    siblings,
                    origins,
                }
            })
            .collect();
        stats_map.entrypoints = chunks
            .iter()
            .filter_map(|chunk| match &chunk.chunk_type {
                ChunkType::Entry(_, name, _) => {
                    let mut chunks = chunk_graph
                        .entry_dependencies_chunk(&chunk.id)
                        .into_iter()
                        .map(|id| id.id)
                        .collect::<Vec<_>>();

                    chunks.push(chunk.id.id.clone());

                    Some((
                        name.clone(),
                        StatsJsonEntryItem {
                            name: name.clone(),
                            chunks,
                        },
                    ))
                }
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        stats_map.chunk_modules = chunk_modules;

        stats_map.modules = stats_info.get_modules();
        stats_map.rsc_client_components = stats_info.get_rsc_client_components();
        stats_map.rsc_css_modules = stats_info.get_rsc_css_modules();

        stats_map
    }

    pub fn print_stats(&self) {
        let mut assets = self.context.stats_info.get_assets();
        // 按照产物名称排序
        assets.sort();
        // 产物路径需要按照 output.path 来
        let abs_path = &self.context.root;
        let output_path = &self.context.config.output.path;
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

        // Sort the output stats by their size in desc order
        assets_vec.sort_by_key(|(_, size, _)| std::cmp::Reverse(*size));
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
}

#[derive(Debug, PartialEq, Eq, Clone)]
// name 记录实际 filename , 用在 stats.json 中, hashname 用在产物描述和 manifest 中
pub struct AssetsInfo {
    pub assets_type: String,
    pub size: u64,
    pub name: String,
    pub hashname: String,
    pub chunk_id: String,
    pub path: String,
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

#[derive(Serialize, Debug, Clone)]
pub struct ModuleInfo {
    pub id: String,
    pub dependencies: Vec<String>,
    pub dependents: Vec<String>,
}

#[derive(Debug)]
pub struct StatsInfo {
    pub assets: Mutex<Vec<AssetsInfo>>,
    pub rsc_client_components: Mutex<Vec<RscClientInfo>>,
    pub rsc_css_modules: Mutex<Vec<RscCssModules>>,
    pub modules: Mutex<HashMap<String, ModuleInfo>>,
}

impl StatsInfo {
    pub fn new() -> Self {
        Self {
            assets: Mutex::new(vec![]),
            rsc_client_components: Mutex::new(vec![]),
            rsc_css_modules: Mutex::new(vec![]),
            modules: Mutex::new(HashMap::new()),
        }
    }

    pub fn add_assets(
        &self,
        size: u64,
        name: String,
        chunk_id: String,
        path: String,
        hashname: String,
    ) {
        let mut assets = self.assets.lock().unwrap();
        assets.push(AssetsInfo {
            assets_type: "asset".to_string(),
            size,
            name,
            chunk_id,
            path,
            hashname,
        });
    }

    pub fn clear_assets(&self) {
        self.assets.lock().unwrap().clear()
    }

    pub fn get_assets(&self) -> Vec<AssetsInfo> {
        self.assets.lock().unwrap().iter().cloned().collect()
    }

    pub fn parse_modules(&self, context: Arc<Context>) {
        let module_graph = context.module_graph.read().unwrap();
        let mut modules = self.modules.lock().unwrap();
        module_graph.modules().iter().for_each(|module| {
            let dependencies = module_graph
                .get_dependencies(&module.id)
                .iter()
                .map(|(id, _dep)| id.generate(&context))
                .collect::<Vec<_>>();
            let dependents = module_graph
                .get_dependents(&module.id)
                .iter()
                .map(|(id, _dep)| id.generate(&context))
                .collect::<Vec<_>>();
            let id = module.id.generate(&context);
            modules.insert(
                id.clone(),
                ModuleInfo {
                    id,
                    dependencies,
                    dependents,
                },
            );
        });
    }

    pub fn get_modules(&self) -> HashMap<String, ModuleInfo> {
        self.modules.lock().unwrap().clone()
    }

    pub fn get_rsc_client_components(&self) -> Vec<RscClientInfo> {
        self.rsc_client_components.lock().unwrap().clone()
    }

    pub fn add_rsc_client_component(&self, rsc_client_component: RscClientInfo) {
        self.rsc_client_components
            .lock()
            .unwrap()
            .push(rsc_client_component)
    }

    pub fn get_rsc_css_modules(&self) -> Vec<RscCssModules> {
        self.rsc_css_modules.lock().unwrap().clone()
    }

    pub fn add_rsc_css_module(&self, rsc_css_module: RscCssModules) {
        self.rsc_css_modules.lock().unwrap().push(rsc_css_module)
    }
}

impl Default for StatsInfo {
    fn default() -> Self {
        Self::new()
    }
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

#[derive(Serialize, Debug, Clone)]
pub struct StatsJsonAssetsItem {
    #[serde(flatten)]
    pub assets_type: StatsJsonType,
    pub size: u64,
    pub name: String,
    pub path: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct StatsJsonModuleItem {
    pub id: String,
    pub deps: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct StatsJsonChunkModuleItem {
    #[serde(flatten)]
    pub module_type: StatsJsonType,
    pub size: u64,
    pub id: String,
    pub chunks: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatsJsonChunkOriginItem {
    pub module: String,
    pub module_identifier: String,
    pub module_name: String,
    pub loc: String,
    pub request: String,
}
#[derive(Serialize, Debug, Clone)]
pub struct StatsJsonChunkItem {
    #[serde(flatten)]
    pub chunk_type: StatsJsonType,
    pub id: String,
    pub files: Vec<String>,
    pub entry: bool,
    pub modules: Vec<StatsJsonChunkModuleItem>,
    pub siblings: Vec<String>,
    pub origins: Vec<StatsJsonChunkOriginItem>,
}
#[derive(Serialize, Debug, Clone)]
pub struct StatsJsonEntryItem {
    pub name: String,
    pub chunks: Vec<String>,
}
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatsJsonMap {
    hash: u64,
    built_at: i64,
    root_path: String,
    output_path: String,
    assets: Vec<StatsJsonAssetsItem>,
    chunk_modules: Vec<StatsJsonChunkModuleItem>,
    modules: HashMap<String, ModuleInfo>,
    chunks: Vec<StatsJsonChunkItem>,
    entrypoints: HashMap<String, StatsJsonEntryItem>,
    rsc_client_components: Vec<RscClientInfo>,
    #[serde(rename = "rscCSSModules")]
    rsc_css_modules: Vec<RscCssModules>,
    pub start_time: i64,
    pub end_time: i64,
}

impl StatsJsonMap {
    fn new() -> Self {
        Self {
            hash: 0,
            built_at: 0,
            root_path: String::new(),
            output_path: String::new(),
            assets: vec![],
            modules: HashMap::new(),
            chunk_modules: vec![],
            chunks: vec![],
            entrypoints: HashMap::new(),
            rsc_client_components: vec![],
            rsc_css_modules: vec![],
            start_time: 0,
            end_time: 0,
        }
    }
}

pub fn write_stats(path: &Path, stats: &StatsJsonMap) {
    let path = path.join("stats.json");
    let stats_json = serde_json::to_string_pretty(stats).unwrap();
    fs::write(path, stats_json).unwrap();
}

// 文件大小转换
pub fn human_readable_size(size: u64) -> String {
    let units = ["kB", "MB", "GB"];
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
fn file_size(path: &str) -> Result<u64> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.len())
}
