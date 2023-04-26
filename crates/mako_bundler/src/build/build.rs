use std::collections::HashMap;

use crate::{
    compiler::Compiler,
    config::get_first_entry_value,
    module::{Module, ModuleAst, ModuleId, ModuleInfo},
    module_graph::Dependency,
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
        let cwd = &self.context.config.root;
        let entry_point = cwd
            .join(get_first_entry_value(&self.context.config.entry).unwrap())
            .to_string_lossy()
            .to_string();

        // build
        self.build_module_graph(entry_point, build_param);
    }

    fn build_module_graph(&mut self, entry_point: String, build_param: &BuildParam) {
        let mut queue: Vec<Task> = vec![Task {
            path: entry_point.clone(),
            parent_module_id: None,
            parent_dependecy: None,
        }];

        while !queue.is_empty() {
            let task = queue.pop().unwrap();
            let path_str = task.path.as_str();

            let module_id = ModuleId::new(path_str);
            let is_entry = path_str == entry_point;

            // check if module is already in the graph
            if self.context.module_graph.has_module(&module_id) {
                self.bind_dependency(&task, &module_id);
                continue;
            }

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

            // transform
            let transform_param = TransformParam {
                path: path_str,
                ast: &ModuleAst::Script(parse_result.ast.clone()),
                cm: &parse_result.cm,
            };
            let transform_result = transform(&transform_param, &self.context);

            // add current module to module graph
            let info = ModuleInfo {
                path: task.path.clone(),
                is_external: false,
                external_name: None,
                is_entry,
                original_cm: Some(parse_result.cm),
                original_ast: ModuleAst::Script(transform_result.ast),
            };
            let module = Module::new(module_id.clone(), info);
            self.context.module_graph.add_module(module);

            // handle dependency bind
            self.bind_dependency(&task, &module_id);

            // analyze deps
            let analyze_deps_param = AnalyzeDepsParam {
                path: path_str,
                ast: &parse_result.ast,
            };
            let analyze_deps_result = analyze_deps(&analyze_deps_param, &self.context);

            // resolve
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
                        external_name: Some(external_name),
                        is_entry: false,
                        original_cm: None,
                        original_ast: crate::module::ModuleAst::None,
                    };
                    let extrnal_module_id = ModuleId::new(&resolve_result.path);
                    let extranl_module = Module::new(extrnal_module_id.clone(), info);
                    self.context.module_graph.add_module(extranl_module);
                    self.context.module_graph.add_dependency(
                        &module_id,
                        &extrnal_module_id,
                        d.clone(),
                    )
                } else {
                    queue.push(Task {
                        parent_module_id: Some(module_id.clone()),
                        path: resolve_result.path,
                        parent_dependecy: Some(d.clone()),
                    });
                }
            }
        }
    }

    fn bind_dependency(&mut self, task: &Task, module_id: &ModuleId) {
        if let Some(parent_module_id) = &task.parent_module_id {
            let parent_dependency = task
                .parent_dependecy
                .as_ref()
                .expect("parent dependency is required for parent_module_id");
            self.context.module_graph.add_dependency(
                parent_module_id,
                module_id,
                parent_dependency.clone(),
            )
        }
    }
}
