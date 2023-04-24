use std::{collections::HashMap, fs};

use crate::{compiler::Compiler, module::ModuleId};

fn wrap_module(id: &ModuleId, code: &str) -> String {
    let id = id.id.clone();
    println!("> wrap_module: {}", id);
    format!(
        "define(\"{}\", function(module, exports, require) {{\n{}}});",
        id, code
    )
}

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
        // generate code
        let mut output: Vec<String> = vec![r#"
const modules = new Map();
const define = (name, moduleFactory) => {
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
            .to_string(),
            r#"
        define("@swc/helpers/_/_interop_require_default", function(module, exports, require) {
"use strict";
exports._ = exports._interop_require_default = _interop_require_default;
function _interop_require_default(obj) {
    return obj && obj.__esModule ? obj : { default: obj, ...obj };
}
        });
"#
            .to_string(),
            r#"
        define("@swc/helpers/_/_export_star", function(module, exports, require) {
"use strict";
exports._ = exports._export_star = _export_star;
function _export_star(from, to) {
    Object.keys(from).forEach(function(k) {
        if (k !== "default" && !Object.prototype.hasOwnProperty.call(to, k)) {
            Object.defineProperty(to, k, {
                enumerable: true,
                get: function() {
                    return from[k];
                }
            });
        }
    });
    return from;
}
        });
"#
            .to_string(),
            r#"
        define("@swc/helpers/_/_interop_require_wildcard", function(module, exports, require) {
"use strict";
function _getRequireWildcardCache(nodeInterop) {
    if (typeof WeakMap !== "function") return null;
    var cacheBabelInterop = new WeakMap();
    var cacheNodeInterop = new WeakMap();
    return (_getRequireWildcardCache = function(nodeInterop) {
        return nodeInterop ? cacheNodeInterop : cacheBabelInterop;
    })(nodeInterop);
}
exports._ = exports._interop_require_wildcard = _interop_require_wildcard;
function _interop_require_wildcard(obj, nodeInterop) {
    if (!nodeInterop && obj && obj.__esModule) return obj;
    if (obj === null || typeof obj !== "object" && typeof obj !== "function") return { default: obj };
    var cache = _getRequireWildcardCache(nodeInterop);
    if (cache && cache.has(obj)) return cache.get(obj);
    var newObj = {};
    var hasPropertyDescriptor = Object.defineProperty && Object.getOwnPropertyDescriptor;
    for (var key in obj) {
        if (key !== "default" && Object.prototype.hasOwnProperty.call(obj, key)) {
            var desc = hasPropertyDescriptor ? Object.getOwnPropertyDescriptor(obj, key) : null;
            if (desc && (desc.get || desc.set)) Object.defineProperty(newObj, key, desc);
            else newObj[key] = obj[key];
        }
    }
    newObj.default = obj;
    if (cache) cache.set(obj, newObj);
    return newObj;
}
        });
"#
                .to_string(),


        ];
        let module_ids = self
            .context
            .module_graph
            .topo_sort()
            .expect("module graph has cycle");

        let mut entry_module_id = String::new();
        let mut results: Vec<String> = vec![];
        for module_id in module_ids {
            let id = module_id.clone();
            let module = self
                .context
                .module_graph
                .get_module(&id)
                .expect("module not found");

            if module.info.is_entry {
                entry_module_id = module.id.id.clone();
            }

            let info = &module.info;
            let code = if info.is_external {
                format!(
                    "/* external {} */ exports.default = {};",
                    info.path,
                    info.external_name.as_ref().unwrap(),
                )
            } else {
                // get deps
                let deps = self.context.module_graph.get_dependencies(&module_id);
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
                    cm,
                    ast: &info.original_ast,
                    dep_map,
                    env_map,
                };
                let transform_result = transform(&transform_param, &self.context);
                transform_result.code
            };

            results.push(wrap_module(&id, &code));
        }

        output.extend(results);
        output.push(format!("\nrequireModule(\"{}\");", entry_module_id));
        let contents = output.join("\n");

        let root_dir = &self.context.config.root;
        let output_dir = &self.context.config.output.path;
        if generate_param.write && !output_dir.exists() {
            fs::create_dir_all(output_dir).unwrap();
        }

        // write to file
        if generate_param.write {
            fs::write(&output_dir.join("bundle.js"), &contents).unwrap();
        }

        // write assets
        let assets_info = &self.context.assets_info;
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

        GenerateResult {
            output_files: vec![OutputFile {
                path: "bundle.js".to_string(),
                // for test
                __output: output,
                contents,
            }],
        }
    }
}
