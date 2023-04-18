use std::{fs, path::PathBuf, str::FromStr};

use kuchiki::traits::*;

use crate::config::get_first_entry_value;
use crate::{compiler::Compiler, module::ModuleId};

fn wrap_module(id: &ModuleId, code: &str) -> String {
    let id = id.id.clone();
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
    pub fn generate(&mut self, generate_param: &GenerateParam) -> GenerateResult {
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
            let transform_info = module.transform_info.as_ref().unwrap();
            let code = transform_info.code.clone();
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

        let entry = get_first_entry_value(&self.context.config.entry).unwrap();
        if entry.ends_with(".html") || entry.ends_with(".htm") {
            let p = root_dir.join(entry);

            let html = fs::read_to_string(p.as_path()).unwrap();
            let document = kuchiki::parse_html().one(html);

            for node_data in document.select("script[src]").unwrap() {
                let node = node_data.as_node().as_element().unwrap();
                let mut attrs = node.attributes.borrow_mut();

                if let Some(src) = attrs.get("src") {
                    if !src.starts_with("http://") && !src.starts_with("https://") {
                        attrs.insert("src", "/bundle.js".to_owned());
                    }
                }
            }

            let mut updated_html = Vec::new();
            document.serialize(&mut updated_html).unwrap();
            fs::write(&output_dir.join("index.html"), updated_html).unwrap();
        } else {
            // copy html
            let index_html_file = &root_dir.join("index.html");
            if index_html_file.exists() {
                fs::copy(index_html_file, &output_dir.join("index.html")).unwrap();
            }
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
