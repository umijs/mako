use std::{fs, path::PathBuf, str::FromStr};

use kuchiki::traits::*;

use crate::compiler::Compiler;
use crate::config::get_first_entry_value;

fn wrap_module(id: &str, code: &str) -> String {
    println!("> wrap_module: {}", id);
    format!(
        "define(\"{}\", function(module, exports, require) {{\n{}}});",
        id, code
    )
}

impl Compiler {
    pub fn generate(&self) {
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
        for val in values {
            results.push(wrap_module(&val.id.id, &val.info.code));
            if val.info.is_entry {
                entry_module_id = val.id.id.clone();
            }
        }
        output.extend(results);
        output.push(format!("\nrequireModule(\"{}\");", entry_module_id));
        let contents = output.join("\n");

        let root_dir = PathBuf::from_str(&self.context.config.root).unwrap();
        let output_dir = PathBuf::from_str(&self.context.config.output.path).unwrap();
        if !output_dir.exists() {
            fs::create_dir_all(&output_dir).unwrap();
        }

        // write to file
        fs::write(&output_dir.join("bundle.js"), contents).unwrap();

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

        println!("✅ DONE");
    }
}
