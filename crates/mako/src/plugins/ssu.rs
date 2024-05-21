use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File as SysFile;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dashmap::DashSet;
use mako_core::anyhow::Result;
use mako_core::rayon::prelude::*;
use mako_core::regex::Regex;
use mako_core::tracing::debug;
use serde::{Deserialize, Serialize};

use crate::ast::file::{Content, File, JsContent};
use crate::compiler::{Args, Compiler, Context};
use crate::config::{
    CodeSplittingStrategy, Config, OptimizeAllowChunks, OptimizeChunkGroup, OptimizeChunkOptions,
};
use crate::generate::chunk::ChunkType;
use crate::generate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::plugin::{NextBuildParam, Plugin, PluginLoadParam};
use crate::resolve::ResolverResource;

#[derive(Serialize, Deserialize, Debug, Default)]
struct CacheState {
    reversed_required_files: HashSet<String>,
    cached_boundaries: HashMap<String, String>,
    js_patch_map: HashMap<String, String>,
    css_patch_map: HashMap<String, String>,
}

pub struct SUPlus {
    scanning: Arc<Mutex<bool>>,
    enabled: Arc<Mutex<bool>>,
    dependence_node_module_files: DashSet<File>,
    reversed_required_files: DashSet<File>,
    cached_state: Arc<Mutex<CacheState>>,
    current_state: Arc<Mutex<CacheState>>,
}

#[derive(Debug)]
enum CodeType {
    SourceCode,
    Dependency,
}

impl From<bool> for CodeType {
    fn from(value: bool) -> Self {
        if value {
            CodeType::Dependency
        } else {
            CodeType::SourceCode
        }
    }
}

impl SUPlus {
    pub fn new() -> Self {
        SUPlus {
            scanning: Arc::new(Mutex::new(true)),
            enabled: Arc::new(Mutex::new(false)),
            dependence_node_module_files: Default::default(),
            reversed_required_files: Default::default(),
            cached_state: Default::default(),
            current_state: Default::default(),
        }
    }
}

impl SUPlus {
    fn write_current_cache_state(&self, context: &Arc<Context>) {
        let cache_file = context.root.join(".mako_cache");
        let cache = self.current_state.lock().unwrap();
        std::fs::write(cache_file, serde_json::to_string(&*cache).unwrap()).unwrap();
    }

    fn load_cached_state(&self, context: &Arc<Context>) -> Option<CacheState> {
        let cache_file = context.root.join(".mako_cache");
        if let Ok(content) = std::fs::read_to_string(cache_file)
            && let Ok(disk_cache) = serde_json::from_str(&content)
        {
            return Some(disk_cache);
        }

        None
    }
}

impl Plugin for SUPlus {
    fn name(&self) -> &str {
        "speedup_plus"
    }

    fn modify_config(&self, config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        for p in config.entry.values_mut() {
            *p = PathBuf::from(format!("virtual:E:node_modules:{}", p.to_string_lossy()));
        }

        config.code_splitting = Some(CodeSplittingStrategy::Advanced(OptimizeChunkOptions {
            min_size: 0,
            groups: vec![
                OptimizeChunkGroup {
                    name: "node_modules".to_string(),
                    allow_chunks: OptimizeAllowChunks::All,
                    min_chunks: 0,
                    min_size: 0,
                    max_size: usize::MAX,
                    priority: 10,
                    test: Regex::new(r"[/\\]node_modules[/\\]").ok(),
                },
                OptimizeChunkGroup {
                    name: "common".to_string(),
                    min_chunks: 0,
                    // always split, to avoid multi-instance risk
                    min_size: 1,
                    max_size: usize::MAX,
                    priority: 0,
                    ..Default::default()
                },
            ],
        }));

        Ok(())
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if param.file.path.starts_with("virtual:E:node_modules:") {
            let path_string = param.file.path.to_string_lossy().to_string();

            let path = PathBuf::from(path_string.as_str()[23..].to_string());

            let mut require_externals = _context
                .config
                .externals
                .iter()
                .map(|ext| format!("require('{}');", ext.0))
                .collect::<Vec<_>>();

            require_externals.sort();

            let mut reverse_require = self
                .cached_state
                .lock()
                .unwrap()
                .reversed_required_files
                .iter()
                .map(|f| format!("require('{}')", f))
                .collect::<Vec<_>>();
            reverse_require.sort();

            let content = format!(
                r#"
require("virtual:C:/node_modules/css/css.css");                    
let patch = require._su_patch();
console.log(patch);
{}
Promise.all(
    patch.map((d)=>__mako_require__.ensure(d))
).then(()=>{{
    {}
    require("{}");
}}, console.log);
"#,
                require_externals.join("\n"),
                reverse_require.join("\n"),
                path.to_string_lossy()
            );

            debug!("entry contrent:\n{}", content);

            return Ok(Some(Content::Js(JsContent {
                content,
                is_jsx: false,
            })));
        }

        if param
            .file
            .path
            .starts_with("virtual:C:/node_modules/css/css.css")
        {
            return Ok(Some(Content::Css("._mako_mock_css { }".to_string())));
        }
        Ok(None)
    }

    fn next_build(&self, next_build_param: &NextBuildParam) -> bool {
        let from: CodeType = next_build_param
            .current_module
            .id
            .contains("/node_modules/")
            .into();
        let to = next_build_param.next_file.is_under_node_modules.into();

        debug!(
            "{} -> {}",
            next_build_param.current_module.id,
            next_build_param
                .next_file
                .pathname
                .to_string_lossy()
                .to_string()
        );

        match (from, to) {
            (CodeType::SourceCode, CodeType::Dependency) => {
                if let ResolverResource::Resolved(resolved) = &next_build_param.resource {
                    self.dependence_node_module_files
                        .insert(next_build_param.next_file.clone());

                    let path_name = next_build_param
                        .next_file
                        .path
                        .to_string_lossy()
                        .to_string();

                    let version = resolved
                        .0
                        .package_json()
                        .and_then(|p| p.raw_json().get("version"))
                        .map_or("0.0.0".to_string(), |v| {
                            v.as_str().unwrap_or("0.0.0").to_string()
                        });

                    self.current_state
                        .lock()
                        .unwrap()
                        .cached_boundaries
                        .insert(path_name, version);

                    let scanning = *self.scanning.lock().unwrap();
                    !scanning
                } else {
                    true
                }
            }
            (CodeType::Dependency, CodeType::SourceCode) => {
                debug!(
                    "{} -> {}",
                    next_build_param.current_module.id,
                    next_build_param.next_file.pathname.to_string_lossy()
                );

                self.current_state
                    .lock()
                    .unwrap()
                    .reversed_required_files
                    .insert(
                        next_build_param
                            .next_file
                            .pathname
                            .to_string_lossy()
                            .to_string(),
                    );
                true
            }
            _ => true,
        }
    }

    fn after_build(&self, _context: &Arc<Context>, compiler: &Compiler) -> Result<()> {
        debug!("start after build");

        let cached = self.cached_state.lock().unwrap();
        let current_state = self.current_state.lock().unwrap();

        #[cfg(debug_assertions)]
        {
            debug!("collected {:?}", current_state.cached_boundaries);
            debug!("cached {:?}", cached.cached_boundaries);
        }

        let cache_valid = current_state.cached_boundaries.len() == cached.cached_boundaries.len()
            && cached
                .cached_boundaries
                .iter()
                .any(|(k, _)| cached.cached_boundaries.contains_key(k));

        drop(current_state);

        debug!("cache valid? {}", cache_valid);

        if cache_valid {
            *self.enabled.lock().unwrap() = true;
            return Ok(());
        }

        *self.enabled.lock().unwrap() = false;

        let files = self
            .dependence_node_module_files
            .iter()
            .map(|f| f.clone())
            .collect::<Vec<File>>();

        let mut s = self.scanning.lock().unwrap();
        *s = false;
        drop(s);

        debug!("start to build dep");
        compiler.build(files)?;

        let mut s = self.scanning.lock().unwrap();
        *s = true;

        self.reversed_required_files
            .iter()
            .for_each(|f| println!("r: {:?}", f.path));

        Ok(())
    }

    fn after_generate_chunk_files(
        &self,
        chunk_files: &Vec<ChunkFile>,
        context: &Arc<Context>,
    ) -> Result<()> {
        if *self.enabled.lock().unwrap() {
            return Ok(());
        }

        let cache_root = context.root.join(".cache");
        if !cache_root.exists() {
            fs::create_dir_all(&cache_root)?;
        }

        let mut js_patch_map = HashMap::new();
        let mut css_patch_map = HashMap::new();

        // 改改根据 chunk graph 来修正
        chunk_files
            .iter()
            .filter(|&cf| cf.file_name.starts_with("node_modules"))
            .for_each(|cf| match cf.file_type {
                ChunkFileType::JS => {
                    js_patch_map.insert(cf.chunk_id.clone(), cf.disk_name());
                }
                ChunkFileType::Css => {
                    css_patch_map.insert(cf.chunk_id.clone(), cf.disk_name());
                }
            });

        {
            let mut state = self.current_state.lock().unwrap();
            state.js_patch_map = js_patch_map;
            state.css_patch_map = css_patch_map;
        }

        self.write_current_cache_state(context);

        chunk_files
            .par_iter()
            .filter(|&cf| cf.file_name.starts_with("node_modules"))
            .for_each(|cf| {
                let p = cache_root.join(cf.disk_name());
                if let Some(source_map) = &cf.source_map {
                    fs::write(cache_root.join(cf.source_map_disk_name()), source_map).unwrap();

                    let mut f = SysFile::create(&p).unwrap();

                    let last_line = match &cf.file_type {
                        ChunkFileType::JS => {
                            format!("\n//# sourceMappingURL={}", cf.source_map_disk_name())
                        }
                        ChunkFileType::Css => {
                            format!("\n/*# sourceMappingURL={}*/", cf.source_map_disk_name())
                        }
                    };
                    // where should store a integrity file to verify the cache validate or not
                    f.write_all(&cf.content).unwrap();
                    f.write_all(last_line.as_bytes()).unwrap();
                } else {
                    std::fs::write(p, &cf.content).unwrap();
                }
            });

        Ok(())
    }

    fn build_start(&self, context: &Arc<Context>) -> Result<Option<()>> {
        if let Some(content) = self.load_cached_state(context) {
            let mut state = self.cached_state.lock().unwrap();
            *state = content;
        }
        // verify cached files

        Ok(None)
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> Result<Vec<String>> {
        if *self.enabled.lock().unwrap() {
            let cache = self.cached_state.lock().unwrap();

            let code = format!(
                r#"
requireModule._su_patch = function(){{
    var js_patch = {};
    var css_patch = {};
    for(var key in js_patch) {{
        chunksIdToUrlMap[key] = js_patch[key];
    }}
    for(var key in js_patch) {{
        cssChunksIdToUrlMap[key] = css_patch[key];
    }}
    return Object.keys(js_patch).sort();
}}
"#,
                serde_json::to_string(&cache.js_patch_map).unwrap(),
                serde_json::to_string(&cache.css_patch_map).unwrap(),
            );

            Ok(vec![code])
        } else {
            let cg = _context.chunk_graph.read().unwrap();

            cg.get_chunks()
                .into_iter()
                .filter(|c| c.chunk_type == ChunkType::Sync)
                .for_each(|c| {
                    println!("chunk: {}", c.filename());
                });

            Ok(vec![r#"
requireModule._su_patch = function(){
     var js_patch = {
        "node_modules": "node_modules.js"
     };
    var css_patch = {
        "node_modules": "node_modules.css"
    };
    for(var key in js_patch) {
        chunksIdToUrlMap[key] = js_patch[key];
    }
    for(var key in js_patch) {
        cssChunksIdToUrlMap[key] = css_patch[key];
    } 
  return ["node_modules"];
}"#
            .to_string()])
        }
    }
}
