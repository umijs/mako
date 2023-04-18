use std::{collections::{HashMap, HashSet}, fs, path::PathBuf, str::FromStr};
use std::path::Path;
use select::document::Document;
use select::predicate::{Attr, Name, Predicate};
use std::io::{Error, ErrorKind};

use crate::{
    compiler::Compiler,
    config::get_first_entry_value,
    module::{Module, ModuleAst, ModuleId, ModuleInfo},
};

use super::{
    analyze_deps::{analyze_deps, AnalyzeDepsParam},
    load::{load, LoadParam},
    parse::{parse, ParseParam},
    resolve::{resolve, ResolveParam},
    transform::transform::{transform, TransformParam},
};

impl Compiler {
    fn get_entry_point(&self) -> Result<String, Error> {
        let cwd = PathBuf::from_str(self.context.config.root.as_str()).unwrap();
        let entry_point = cwd
            .join(get_first_entry_value(&self.context.config.entry).unwrap());

        if let Some(ext) = entry_point.extension() {
            match ext.to_str().or(Some("")).unwrap() {
                "ts" | "tsx" | "js" | "jsx" => {
                    return Ok(entry_point.to_string_lossy().to_string());
                }
                "html" | "htm" => {
                    let relative_entry = self.extract_entry_from_html(entry_point.as_path()).unwrap();
                    let entry_point = cwd.join(relative_entry.as_str())
                        .canonicalize().expect(format!("Bad entry point {}", relative_entry).as_str());

                    return Ok(entry_point.to_string_lossy().to_string());
                }
                _ => {
                    panic!("Unsupported file {}", entry_point.to_string_lossy().to_string());
                }
            }
        } else {
            panic!("Unsupported file {}", entry_point.to_string_lossy().to_string());
        }
    }

    fn extract_entry_from_html(&self, html_path: &Path) -> Result<String, Error> {
        let path_name = html_path.to_string_lossy().to_string();

        let html = fs::read_to_string(html_path)?;
        let document = Document::from(html.as_str());
        let mut entries: Vec<String> = document
            .find(Name("script").and(Attr("src", ())))
            .filter_map(|node| node.attr("src"))
            .filter(|src| !src.starts_with("http://") &&
                !src.starts_with("https://") &&
                !src.starts_with("//")
            )
            .map(|src| src.to_string())
            .collect();

        match entries.len() {
            0 => {
                Err(Error::new(ErrorKind::NotFound, format!("No entry found in {}", path_name)))
            }
            1 => {
                let entry = entries.remove(0);
                if entry.starts_with("/") {
                    Ok(entry.strip_prefix("/").unwrap().to_string())
                } else {
                    Ok(entry)
                }
            }
            _ => {
                dbg!(entries);
                Err(Error::new(ErrorKind::InvalidData, format!("Multiple entries found in {}", path_name)))
            }
        }
    }

    pub fn build(&mut self) {
        let entry_point = self.get_entry_point().unwrap();
        let mut seen = HashSet::<String>::new();
        let mut queue = vec![entry_point.clone()];

        while !queue.is_empty() {
            let path = queue.pop().unwrap();
            let path_str = path.as_str();
            if seen.contains(&path) {
                continue;
            }
            seen.insert(path.clone());

            let module_id = ModuleId::new(path_str);
            let is_entry = path_str == entry_point;

            // load
            let load_param = LoadParam { path: path_str };
            let load_result = load(&load_param, &self.context);

            // parse
            let parse_param = ParseParam {
                path: path_str,
                content: load_result.content,
            };
            let parse_result = parse(&parse_param, &self.context);

            // analyze deps
            let analyze_deps_param = AnalyzeDepsParam {
                path: path_str,
                ast: &parse_result.ast,
            };
            let analyze_deps_result = analyze_deps(&analyze_deps_param, &self.context);

            // resolve
            let mut dep_map = HashMap::<String, String>::new();
            for d in &analyze_deps_result.dependencies {
                let resolve_param = ResolveParam {
                    path: path_str,
                    dependency: d,
                };
                let resolve_result = resolve(&resolve_param, &self.context);
                println!(
                    "> resolve {} from {} -> {}",
                    d, path_str, resolve_result.path
                );
                if resolve_result.is_external {
                    let external_name = resolve_result.external_name.unwrap();
                    let info = ModuleInfo {
                        path: resolve_result.path.clone(),
                        is_external: resolve_result.is_external,
                        is_entry: false,
                        code: format!(
                            "/* external {} */ exports.default = {};",
                            resolve_result.path, external_name,
                        ),
                        ast: crate::module::ModuleAst::None,
                    };
                    let module_id = ModuleId::new(&resolve_result.path);
                    dep_map.insert(d.clone(), module_id.id.clone());
                    let module = Module::new(module_id.clone(), info);
                    let _ = &self
                        .context
                        .module_graph
                        .id_module_map
                        .insert(module_id, module);
                } else {
                    dep_map.insert(d.clone(), ModuleId::new(resolve_result.path.as_str()).id);
                    queue.push(resolve_result.path);
                }
            }

            // transform
            // TODO: move transform before analyze deps
            let transform_param = TransformParam {
                path: path_str,
                ast: parse_result.ast,
                cm: parse_result.cm,
                dep_map,
            };
            let transform_result = transform(&transform_param, &self.context);

            // add current module to module graph
            let info = ModuleInfo {
                path,
                is_external: false,
                is_entry,
                ast: ModuleAst::Script(transform_result.ast),
                code: transform_result.code,
            };

            let module = Module::new(module_id.clone(), info);
            let _ = &self
                .context
                .module_graph
                .id_module_map
                .insert(module_id, module);
        }
    }
}
