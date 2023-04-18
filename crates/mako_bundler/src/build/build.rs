use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    str::FromStr,
};

use crate::{
    compiler::Compiler,
    config::get_first_entry_value,
    module::{Module, ModuleAst, ModuleId, ModuleInfo}, module_graph::Dependency,
};

use super::{
    analyze_deps::{analyze_deps, AnalyzeDepsParam},
    load::{load, LoadParam},
    parse::{parse, ParseParam},
    resolve::{resolve, ResolveParam},
    transform::transform::{transform, TransformParam},
};

pub struct BuildParam {
    pub files: Option<HashMap<String, String>>,
}

struct Task {
	pub path: String,
	pub parent_module_id: Option<ModuleId>,
	pub parent_dependecy: Option<Dependency>,
}

impl Compiler {
    pub fn build(&mut self, build_param: &BuildParam) {
        let cwd = PathBuf::from_str(self.context.config.root.as_str()).unwrap();
        let entry_point = cwd
            .join(get_first_entry_value(&self.context.config.entry).unwrap())
            .to_string_lossy()
            .to_string();
        let mut seen = HashSet::<String>::new();
        let mut queue: Vec<Task> = vec![Task{
			path: entry_point.clone(),
			parent_module_id: None,
			parent_dependecy: None,
		}];

        while !queue.is_empty() {
            let task = queue.pop().unwrap();
            let path_str = task.path.as_str();
            if seen.contains(&task.path) {
                continue;
            }
            seen.insert(task.path.clone());

            let module_id = ModuleId::new(path_str);
            let is_entry = path_str == entry_point;

            // load
            let load_param = LoadParam {
                path: path_str,
                files: build_param.files.as_ref(),
            };
            let load_result = load(&load_param, &mut self.context);

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
                    dependency: &d.source,
                    files: build_param.files.as_ref(),
                };
                let resolve_result = resolve(&resolve_param, &self.context);
                println!(
                    "> resolve {} from {} -> {}",
                    &d.source, path_str, resolve_result.path
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
                    dep_map.insert(d.source.clone(), module_id.id.clone());
                    let module = Module::new(module_id.clone(), info);
                    let _ = &self
                        .context
                        .module_graph
						.add_module(module);
                } else {
					let dep_module_id = ModuleId::new(resolve_result.path.as_str());
                    dep_map.insert(d.source.clone(), dep_module_id.id.clone());
                    queue.push(Task {
						parent_module_id: Some(module_id.clone()),
						path: resolve_result.path,
						parent_dependecy: Some(d.clone()),
					});
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
                path: task.path,
                is_external: false,
                is_entry,
                ast: ModuleAst::Script(transform_result.ast),
                code: transform_result.code,
            };
            let module = Module::new(module_id.clone(), info);
            let _ = &self
                .context
                .module_graph.add_module(module);

			// handle dependency bind
			if let Some(parent_module_id) = task.parent_module_id {
				let parent_dependency = task.parent_dependecy.expect("parent dependency is required for parent_module_id");
				self.context.module_graph.add_dependency(&parent_module_id, &module_id, parent_dependency)
			}

        }
    }
}
