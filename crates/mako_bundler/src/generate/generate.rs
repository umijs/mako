use std::{fs, path::PathBuf, str::FromStr};

use crate::{compiler::Compiler, module::ModuleId};

fn wrap_module(id: &str, code: &str) -> String {
    println!("> wrap_module: {}", id);
    format!(
        "define(\"{}\", function(module, exports, require) {{\n{}}});",
        id, code
    )
}

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
    pub fn generate(&self, generate_param: &GenerateParam) -> GenerateResult {
        // generate code
        let mut output: Vec<String> = vec![r#"
const modules = new Map();
const define = (name, moduleFactory) => {
  modules.set(name, moduleFactory);
};

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
        let values = self.context.module_graph.id_module_map.values();

        let mut results: Vec<String> = vec![];
        let mut entry_module_id = String::new();
        let mut module_ids = vec![];
        for val in values {
            module_ids.push(val.id.clone());
            if val.info.is_entry {
                entry_module_id = val.id.id.clone();
            }
        }

        // 先简单 sort，后面根据 module_graph 排序
        module_ids.sort_by_key(|v| v.id.clone());

        for module_id in module_ids.iter() {
            let id = module_id.id.clone();
            let code = self
                .context
                .module_graph
                .id_module_map
                .get(module_id)
                .unwrap()
                .info
                .code
                .clone();
            results.push(wrap_module(&id, &code));
        }

        output.extend(results);
        output.push(format!("\nrequireModule(\"{}\");", entry_module_id));
        let contents = output.join("\n");

        let root_dir = PathBuf::from_str(&self.context.config.root).unwrap();
        let output_dir = PathBuf::from_str(&self.context.config.output.path).unwrap();
        if generate_param.write && !output_dir.exists() {
            fs::create_dir_all(&output_dir).unwrap();
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
