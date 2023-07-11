use std::collections::HashMap;

use swc_ecma_visit::VisitMutWith;

use crate::compiler::Compiler;
use crate::module::ModuleId;
use crate::tree_shaking_module::TreeShakingModule;
use crate::unused_statement_marker::UnusedStatementMarker;

impl Compiler {
    pub fn tree_shaking(&self) {
        // 拓扑排序
        let (entry_modules, sorted_modules, cycle_modules) = self.make_toposort();

        // 入口模块设置为副作用模块
        self.markup_entry_modules_as_side_effects(entry_modules);

        // 循环依赖模块设置为副作用
        self.markup_cycle_modules_as_side_effects(cycle_modules);

        let (tree_shaking_module_ids, mut tree_shaking_module_map) =
            self.create_tree_shaking_module_map(sorted_modules);

        let tree_shaking_module_map = &mut tree_shaking_module_map;

        let mut modules_to_remove = vec![];
        for tree_shaking_module_id in tree_shaking_module_ids {
            let tree_shaking_module = tree_shaking_module_map
                .get_mut(&tree_shaking_module_id)
                .unwrap();

            // FIXME: 先默认是 esm 模块

            // 包含副作用的模块
            if tree_shaking_module.side_effects {
                {
                    {
                        let imports = tree_shaking_module.imports();
                        let exports = tree_shaking_module.exports();

                        // 分析使用情况
                        for import in imports {
                            self.analyze_import_statement(
                                tree_shaking_module_map,
                                &tree_shaking_module_id,
                                import,
                            );
                        }

                        for export in exports {
                            self.analyze_export_statement(
                                tree_shaking_module_map,
                                &tree_shaking_module_id,
                                export,
                            );
                        }
                    };
                }
            } else {
                // 不包含副作用的模块，执行 tree shaking
                let tree_shaking_module = tree_shaking_module_map
                    .get_mut(&tree_shaking_module_id)
                    .unwrap();

                // 当前模块没有被使用到的导出，删除当前模块
                if tree_shaking_module.used_exports.is_empty() {
                    modules_to_remove.push(tree_shaking_module_id.clone());
                    continue;
                }

                let mut module_graph = self.context.module_graph.write().unwrap();
                let module = module_graph
                    .get_module_mut(&tree_shaking_module.id)
                    .unwrap();
                let ast = module.info.as_mut().unwrap().ast.as_script_mut();

                // 通过 tree_shaking_module 进行无用的标记
                let mut comments = self.context.meta.script.output_comments.write().unwrap();
                let mut marker = UnusedStatementMarker::new(tree_shaking_module, &mut comments);
                ast.visit_mut_with(&mut marker);
                drop(module_graph);
                drop(comments);

                {
                    let this = self;
                    let imports = tree_shaking_module.imports();
                    let exports = tree_shaking_module.exports();

                    // 分析使用情况
                    for import in imports {
                        this.analyze_import_statement(
                            tree_shaking_module_map,
                            &tree_shaking_module_id,
                            import,
                        );
                    }

                    for export in exports {
                        this.analyze_export_statement(
                            tree_shaking_module_map,
                            &tree_shaking_module_id,
                            export,
                        );
                    }
                };
            }
        }

        // TODO: 增加minify阶段的删除功能
    }

    fn make_toposort(&self) -> (Vec<ModuleId>, Vec<ModuleId>, Vec<Vec<ModuleId>>) {
        let module_graph = self.context.module_graph.read().unwrap();
        let entry_modules = module_graph.get_entry_modules();
        let (sorted_modules, cycle_modules) = module_graph.toposort();
        (entry_modules, sorted_modules, cycle_modules)
    }

    fn markup_entry_modules_as_side_effects(&self, entry_modules: Vec<ModuleId>) {
        let mut module_graph = self.context.module_graph.write().unwrap();
        for entry_module_id in entry_modules {
            let module = module_graph.get_module_mut(&entry_module_id).unwrap();
            module.side_effects = true;
        }
    }

    fn markup_cycle_modules_as_side_effects(&self, cycle_modules: Vec<Vec<ModuleId>>) {
        let mut module_graph = self.context.module_graph.write().unwrap();
        for cycle_module in cycle_modules {
            for module_id in cycle_module {
                let module = module_graph.get_module_mut(&module_id).unwrap();
                module.side_effects = true;
            }
        }
    }

    fn create_tree_shaking_module_map(
        &self,
        sorted_modules: Vec<crate::module::ModuleId>,
    ) -> (Vec<ModuleId>, HashMap<ModuleId, TreeShakingModule>) {
        let mut tree_shaking_module_ids = vec![];
        let mut tree_shaking_module_map = HashMap::new();
        for module_id in sorted_modules {
            let mut dependencies_of_module: Vec<ModuleId> = vec![];
            {
                let module_graph_r = self.context.module_graph.read().unwrap();
                dependencies_of_module.extend(
                    module_graph_r
                        .get_dependencies(&module_id)
                        .iter()
                        .map(|f| f.0.clone())
                        .collect::<Vec<_>>(),
                );
            }

            let mut module_graph = self.context.module_graph.write().unwrap();
            let module = module_graph.get_module_mut(&module_id).unwrap();

            // external 模块，设置为副作用模块
            if module.is_external() {
                module.side_effects = true;
            }

            if !module.get_module_type().is_script() {
                // 非 js 模块，以及他们的依赖，都设置为副作用模块
                for dep_module_id in dependencies_of_module {
                    let dep_module = module_graph.get_module_mut(&dep_module_id).unwrap();
                    dep_module.side_effects = true;
                }
            }

            let module = module_graph.get_module(&module_id).unwrap();
            let tree_shaking_module = TreeShakingModule::new(module);
            tree_shaking_module_ids.push(tree_shaking_module.id.clone());
            tree_shaking_module_map.insert(tree_shaking_module.id.clone(), tree_shaking_module);
        }
        (tree_shaking_module_ids, tree_shaking_module_map)
    }
}

#[cfg(test)]
mod tests {
    use crate::assert_display_snapshot;
    use crate::test_helper::{read_dist_file, setup_compiler};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking() {
        let compiler = setup_compiler("test/build/tree-shaking", false);
        compiler.compile();
        let content = read_dist_file(&compiler);
        assert_display_snapshot!(content);
    }
}
