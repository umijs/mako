use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use anyhow;
use wasmparser::{Import, Parser, Payload};

use crate::ast::file::{Content, JsContent};
use crate::compiler::Context;
use crate::plugin::{Plugin, PluginLoadParam};

pub struct WasmRuntimePlugin {}

impl Plugin for WasmRuntimePlugin {
    fn name(&self) -> &str {
        "wasm_runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        if context
            .assets_info
            .lock()
            .unwrap()
            .values()
            .any(|info| info.ends_with(".wasm"))
        {
            Ok(vec![
                include_str!("./wasm_runtime/wasm_runtime.js").to_string()
            ])
        } else {
            Ok(vec![])
        }
    }

    fn load(
        &self,
        param: &PluginLoadParam,
        _context: &Arc<Context>,
    ) -> anyhow::Result<Option<Content>> {
        let file = param.file;
        if file.path.to_string_lossy().ends_with(".wasm") {
            let final_file_name = format!(
                "{}.{}.{}",
                file.get_file_stem(),
                file.get_content_hash()?,
                file.extname
            );
            let origin_path = file.pathname.to_string_lossy().to_string();
            _context.emit_assets(origin_path, final_file_name.clone());

            let mut buffer = Vec::new();
            File::open(file.path.as_path())?.read_to_end(&mut buffer)?;
            // Parse wasm file to get imports
            let mut import_objs_map: HashMap<&str, Vec<String>> = HashMap::new();
            for payload in Parser::new(0).parse_all(&buffer) {
                if let Ok(Payload::ImportSection(imports)) = payload {
                    for import in imports {
                        match import {
                            Ok(Import { module, name, ty }) => {
                                if let Some(import_obj) = import_objs_map.get_mut(module) {
                                    import_obj.push(name.to_string());
                                } else {
                                    import_objs_map.insert(module, vec![name.to_string()]);
                                }
                            }
                            Err(_) => {
                                println!("import error");
                            }
                        }
                    }
                }
            }

            let mut js_imports_str = String::new();
            let mut import_objs_str = String::new();

            for (index, (key, value)) in import_objs_map.iter().enumerate() {
                js_imports_str.push_str(&format!(
                    "import * as module{module_idx} from \"{module}\";\n",
                    module_idx = index,
                    module = key
                ));

                import_objs_str.push_str(&format!(
                    "\"{module}\": {{ {names} }}",
                    module = key,
                    names = value
                        .iter()
                        .map(|name| format!("{}: module{}.{}", name, index, name))
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }

            let mut content = String::new();
            content.push_str(&js_imports_str);

            if import_objs_str.is_empty() {
                content.push_str(&format!(
                    "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                    final_file_name
                ));
            } else {
                content.push_str(&format!(
                    "module.exports = require._interopreRequireWasm(exports, \"{}\", {{{}}})",
                    final_file_name, import_objs_str
                ));
            }

            return Ok(Some(Content::Js(JsContent {
                content,
                ..Default::default()
            })));
        }

        Ok(None)
    }
}
