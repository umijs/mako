use std::{collections::HashMap, fs};

use crate::chunk::{Chunk, ChunkType};
use crate::compiler::Compiler;
use crate::module_graph::ModuleGraph;
use rayon::prelude::*;
use tracing::debug;

use super::transform::transform::{transform, TransformParam};

pub struct GenerateParam {
    pub write: bool,
}

#[derive(Debug)]
pub struct GenerateResult {
    pub output_files: Vec<OutputFile>,
}

#[derive(Debug)]
pub struct OutputFile {
    pub path: String,
    pub __output: Vec<String>,
    pub contents: String,
}

impl Compiler {
    pub fn generate(&mut self, generate_param: &GenerateParam) -> GenerateResult {
        let root_dir = &self.context.config.root;
        let output_dir = &self.context.config.output.path;

        // ensure dir
        if generate_param.write && !output_dir.exists() {
            fs::create_dir_all(output_dir).unwrap();
        }

        let mut output_files: Vec<OutputFile> = vec![];
        {
            let chunk_graph = self.context.chunk_graph.read().unwrap();
            let module_graph = self.context.module_graph.read().unwrap();
            debug!("chunks {}", &chunk_graph);
            // generate codes
            chunk_graph
                .get_chunks()
                .par_iter()
                .map(|chunk| {
                    let output = Self::chunk_codegen(chunk, &module_graph);
                    let contents = output.join("\n");
                    OutputFile {
                        path: chunk.filename(),
                        __output: output,
                        contents,
                    }
                })
                .collect_into_vec(&mut output_files);
        }

        // write to file
        output_files.par_iter().for_each(|file| {
            if generate_param.write {
                let output = &output_dir.join(&file.path);
                debug!(
                    "output {} {} {}",
                    output.to_string_lossy(),
                    output_dir.to_string_lossy(),
                    file.path
                );
                fs::write(output, &file.contents).unwrap();
            }
        });

        // write assets
        let assets_info = &(*self.context.assets_info.lock().unwrap());
        for (k, v) in assets_info {
            let asset_path = &root_dir.join(k);
            let asset_output_path = &output_dir.join(v);
            if generate_param.write && asset_path.exists() {
                // just copy files for now
                fs::copy(asset_path, asset_output_path).unwrap();
            }
        }

        // copy html
        let index_html_file = &root_dir.join("index.html");
        if generate_param.write && index_html_file.exists() {
            fs::copy(index_html_file, &output_dir.join("index.html")).unwrap();
        }

        // generate_end hook
        self.plugin_driver
            .run_hook_serial(|p, _| {
                p.generate_end(&self.context, generate_param)?;
                Ok(Some(()))
            })
            .unwrap();

        GenerateResult { output_files }
    }

    fn chunk_codegen(chunk: &Chunk, module_graph: &ModuleGraph) -> Vec<String> {
        // TODO: 根据不同的chunk类型使用不同的 wrapper，比如 async chunk 的 wrapper 就不太一样
        let mut results = vec![];
        let entry_preset = vec![r#"
const modules = new Map();
const g_define = (name, moduleFactory) => {
    modules.set(name, moduleFactory);
};

function _interop_require_default(obj) {
    return obj && obj.__esModule && obj['default'] ? obj : { default: obj, ...obj };
}

function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) {
        return obj;
    }
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") {
        return { default: obj };
    }
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) {
        return cache.get(obj);
    }
    var newObj = {};
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for (var key in obj) {
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) {
                Object.defineProperty(newObj, key, desc);
            } else {
                newObj[key] = obj[key];
            }
        }
    }
    newObj.default = obj;
    if (cache) {
        cache.set(obj, newObj);
    }
    return newObj;
}

const moduleCache = new Map();
const requireModule = (name) => {
    if (moduleCache.has(name)) {
    return moduleCache.get(name).exports;
    }

    if (!modules.has(name)) {
    throw new Error(`Module '${name}' does not exist.`);
    }

    const moduleFactory = modules.get(name);
    const module = {
    exports: {},
    };
    moduleCache.set(name, module);
    moduleFactory(module, module.exports, requireModule);
    return module.exports;
};
        "#
        .to_string()];
        let outputs = chunk
            .get_modules()
            .par_iter()
            .map(|module_id| {
                let module = module_graph
                    .get_module(module_id)
                    .expect("module not found");
                let info = module.info.as_ref().unwrap();
                let code = if info.is_external {
                    format!(
                        "/* external {} */ exports.default = {};",
                        info.path,
                        info.external_name.as_ref().unwrap(),
                    )
                } else {
                    // get deps
                    let deps = module_graph.get_dependencies(module_id);
                    let dep_map: HashMap<String, String> = deps
                        .into_iter()
                        .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
                        .collect();

                    // define env
                    let env_map: HashMap<String, String> =
                        HashMap::from([("NODE_ENV".into(), "production".into())]);

                    let cm = info.original_cm.as_ref().unwrap();

                    // transform
                    let transform_param = TransformParam {
                        id: module_id,
                        cm,
                        ast: &info.original_ast,
                        dep_map,
                        env_map,
                    };
                    let transform_result = transform(&transform_param);
                    transform_result.code
                };
                code
                // wrap_module(module_id, &code)
            })
            .collect::<Vec<_>>();
        // setup entry module
        match chunk.chunk_type {
            ChunkType::Runtime => {}
            ChunkType::Entry => {
                results.extend(entry_preset);
                results.extend(outputs);
                results.push(format!("\nrequireModule(\"{}\");", &chunk.id.id));
            }
            ChunkType::Async => {
                results.extend(outputs);
            }
        }
        results
    }
}
