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
            _context.emit_assets(
                file.pathname.to_string_lossy().to_string(),
                final_file_name.clone(),
            );

            let mut buffer = Vec::new();
            File::open(&file.path)?.read_to_end(&mut buffer)?;
            // Parse wasm file to get imports
            let mut wasm_import_object_map: HashMap<&str, Vec<String>> = HashMap::new();
            Parser::new(0).parse_all(&buffer).for_each(|payload| {
                if let Ok(Payload::ImportSection(imports)) = payload {
                    imports.into_iter_with_offsets().for_each(|import| {
                        if let Ok((
                            _,
                            Import {
                                module,
                                name,
                                ty: _,
                            },
                        )) = import
                        {
                            if let Some(import_object) = wasm_import_object_map.get_mut(module) {
                                import_object.push(name.to_string());
                            } else {
                                wasm_import_object_map.insert(module, vec![name.to_string()]);
                            }
                        }
                    });
                }
            });

            let mut module_import_code = String::new();
            let mut wasm_import_object_code = String::new();

            for (index, (key, value)) in wasm_import_object_map.iter().enumerate() {
                module_import_code.push_str(&format!(
                    "import * as module{module_idx} from \"{module}\";\n",
                    module_idx = index,
                    module = key
                ));

                wasm_import_object_code.push_str(&format!(
                    "\"{module}\": {{ {names} }}",
                    module = key,
                    names = value
                        .iter()
                        .map(|name| format!("\"{}\": module{}[\"{}\"]", name, index, name))
                        .collect::<Vec<String>>()
                        .join(", ")
                ));
            }

            let mut content = String::new();
            content.push_str(&module_import_code);

            if wasm_import_object_code.is_empty() {
                content.push_str(&format!(
                    "module.exports = require._interopreRequireWasm(exports, \"{}\")",
                    final_file_name
                ));
            } else {
                content.push_str(&format!(
                    "module.exports = require._interopreRequireWasm(exports, \"{}\", {{{}}})",
                    final_file_name, wasm_import_object_code
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
