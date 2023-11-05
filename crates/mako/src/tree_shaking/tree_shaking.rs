use std::collections::HashMap;

use mako_core::swc_ecma_visit::VisitMutWith;
use mako_core::tracing::debug;

use crate::compiler::Compiler;
use crate::module::ModuleId;
use crate::tree_shaking::tree_shaking_module::{ModuleSystem, TreeShakingModule, UsedExports};
use crate::tree_shaking::unused_statement_cleanup::UnusedStatementCleanup;
use crate::tree_shaking::unused_statement_marker::UnusedStatementMarker;
use crate::tree_shaking::unused_statement_sweep::UnusedStatementSweep;

impl Compiler {
    pub fn tree_shaking(&self) -> Vec<ModuleId> {
        // 拓扑排序
        let (entry_modules, sorted_modules, cycle_modules) = self.make_toposort();
        debug!("entry_modules: {:?}", &entry_modules);
        debug!("cycle_modules: {:?}", &cycle_modules);

        // 入口模块设置为副作用模块
        self.markup_entry_modules_as_side_effects(entry_modules);

        // 循环依赖模块设置为副作用
        // 场景比如：在 app.ts 里使用 umi 的 createGlobalStyle 时
        self.markup_cycle_modules_as_side_effects(cycle_modules);

        let (tree_shaking_module_ids, mut tree_shaking_module_map) =
            self.create_tree_shaking_module_map(sorted_modules);

        let tree_shaking_module_map = &mut tree_shaking_module_map;

        let mut modules_to_remove = vec![];
        for tree_shaking_module_id in &tree_shaking_module_ids {
            let tree_shaking_module = tree_shaking_module_map
                .get_mut(tree_shaking_module_id)
                .unwrap();
            let module_graph = self.context.module_graph.read().unwrap();
            let module = module_graph.get_module(&tree_shaking_module.id).unwrap();
            let module_is_side_effects = module.info.as_ref().unwrap().get_side_effects_flag();
            drop(module_graph);
            debug!(
                "tree_shaking module: [side_effects:{}/{}] {}",
                module_is_side_effects,
                tree_shaking_module.side_effects,
                &tree_shaking_module_id.id,
            );
            debug!(
                "    - used_exports: {:?}",
                &tree_shaking_module.used_exports
            );

            if matches!(tree_shaking_module.module_system, ModuleSystem::ESModule) {
                // 包含副作用的模块
                if tree_shaking_module.side_effects {
                    {
                        {
                            let imports = tree_shaking_module.imports();
                            let exports = tree_shaking_module.exports();

                            // 分析使用情况
                            for import in imports {
                                debug!("    - import: {:?}", &import);
                                self.analyze_import_statement(
                                    tree_shaking_module_map,
                                    tree_shaking_module_id,
                                    import,
                                );
                            }

                            for export in exports {
                                debug!("    - export: {:?}", &export);
                                self.analyze_export_statement(
                                    tree_shaking_module_map,
                                    tree_shaking_module_id,
                                    export,
                                );
                            }
                        };
                    }
                } else {
                    // 不包含副作用的模块，执行 tree shaking
                    let tree_shaking_module = tree_shaking_module_map
                        .get_mut(tree_shaking_module_id)
                        .unwrap();

                    let mut module_graph = self.context.module_graph.write().unwrap();
                    let module = module_graph
                        .get_module_mut(&tree_shaking_module.id)
                        .unwrap();
                    let ast = module.info.as_mut().unwrap().ast.as_script_mut();

                    let unused = tree_shaking_module.get_used_export_statement();
                    // 把没有用到的 stmt 删除
                    let mut reexport_cleanup =
                        UnusedStatementCleanup::new(&tree_shaking_module.id, &unused);
                    ast.visit_mut_with(&mut reexport_cleanup);

                    // 通过 tree_shaking_module 进行无用的标记
                    let mut comments = self.context.meta.script.output_comments.write().unwrap();
                    let mut marker =
                        UnusedStatementMarker::new(tree_shaking_module, unused, &mut comments);
                    ast.visit_mut_with(&mut marker);

                    let mut sweep = UnusedStatementSweep::new(&tree_shaking_module.id, &comments);
                    ast.visit_mut_with(&mut sweep);
                    tree_shaking_module.update_statement(module);

                    drop(module_graph);
                    drop(comments);

                    // 当前模块没有被使用到的导出，删除当前模块
                    if tree_shaking_module.used_exports.is_empty() {
                        modules_to_remove.push(tree_shaking_module_id.clone());
                        continue;
                    }

                    {
                        let imports = tree_shaking_module.imports();
                        let exports = tree_shaking_module.exports();

                        // 分析使用情况
                        for import in imports {
                            debug!("    - import: {:?}", &import);
                            self.analyze_import_statement(
                                tree_shaking_module_map,
                                tree_shaking_module_id,
                                import,
                            );
                        }

                        for export in exports {
                            debug!("    - export: {:?}", &export);
                            self.analyze_export_statement(
                                tree_shaking_module_map,
                                tree_shaking_module_id,
                                export,
                            );
                        }
                    };
                }
            } else {
                let module_graph = self.context.module_graph.read().unwrap();
                for (dep_id, _) in module_graph.get_dependencies(tree_shaking_module_id) {
                    let dep_tree_shake_module = tree_shaking_module_map.get_mut(dep_id);

                    if let Some(dep_tree_shake_module) = dep_tree_shake_module {
                        dep_tree_shake_module.used_exports = UsedExports::All;
                    }
                }
                drop(module_graph);
            }
            // 处理 dynamic import 的情况，把他们都设置成为具备副作用
            self.markup_module_by_resolve_type(tree_shaking_module_id, tree_shaking_module_map);
        }

        self.cleanup_no_used_export_module(&modules_to_remove);

        modules_to_remove
    }

    // Require & DynamicImport 的模块先标记为 UsedExports::All，后续可以优化
    fn markup_module_by_resolve_type(
        &self,
        tree_shaking_module_id: &ModuleId,
        tree_shaking_module_map: &mut HashMap<ModuleId, TreeShakingModule>,
    ) {
        let module_graph = self.context.module_graph.read().unwrap();
        for (dep, edge) in module_graph.get_dependencies(tree_shaking_module_id) {
            if matches!(edge.resolve_type, crate::module::ResolveType::DynamicImport) {
                let tree_shake_module = tree_shaking_module_map.get_mut(dep).unwrap();
                tree_shake_module.side_effects = true;
                tree_shake_module.used_exports = UsedExports::All;
            } else if matches!(edge.resolve_type, crate::module::ResolveType::Require) {
                let tree_shake_module = tree_shaking_module_map.get_mut(dep).unwrap();
                tree_shake_module.used_exports = UsedExports::All;
            }
        }
        drop(module_graph);
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
    #[allow(dead_code)]
    fn markup_cycle_modules_as_side_effects(&self, cycle_modules: Vec<Vec<ModuleId>>) {
        let mut module_graph = self.context.module_graph.write().unwrap();
        for cycle_module in cycle_modules {
            for module_id in cycle_module {
                let module = module_graph.get_module_mut(&module_id).unwrap();
                module.side_effects = true;
            }
        }
    }

    #[allow(dead_code)]
    fn cleanup_no_used_export_module(&self, modules_to_remove: &Vec<ModuleId>) {
        let mut module_graph = self.context.module_graph.write().unwrap();
        debug!("modules_to_remove: {:?}", &modules_to_remove);

        for module_id in modules_to_remove {
            module_graph.remove_module_and_deps(module_id);
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
    use mako_core::tokio;

    use crate::assert_display_snapshot;
    use crate::test_helper::{read_dist_file, setup_compiler};

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_esm_cjs_mix() {
        let compiler = setup_compiler("test/build/tree-shaking-esm-mix-cjs", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");

        assert!(content.contains(r#"cjs.js"#), "cjs.js missing in dist");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_export_all() {
        let compiler = setup_compiler("test/build/tree-shaking-export-all", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");

        assert!(!content.contains(r#"2.ts"#), "2.ts should be shaked");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_esm_cjs_mix_2() {
        let compiler = setup_compiler("test/build/tree-shaking-esm-mix-cjs-2", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");

        println!("{}", content);
        assert!(!content.contains(r#"cjs.js"#), "cjs.js should be shaked");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking() {
        let compiler = setup_compiler("test/build/tree-shaking", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_reexport() {
        let compiler = setup_compiler("test/build/tree-shaking_reexport", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_named_reexport() {
        let compiler = setup_compiler("test/build/tree-shaking_named_reexport", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_export_namespace() {
        let compiler = setup_compiler("test/build/tree-shaking_export_namespace", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_named_export() {
        let compiler = setup_compiler("test/build/tree-shaking_named_export", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_fn() {
        let compiler = setup_compiler("test/build/tree-shaking_fn", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_side_effect() {
        let compiler = setup_compiler("test/build/tree-shaking_side_effect", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_class() {
        let compiler = setup_compiler("test/build/tree-shaking_class", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_exported() {
        let compiler = setup_compiler("test/build/tree-shaking_exported", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_export_default() {
        let compiler = setup_compiler("test/build/tree-shaking_export_default", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_issues_271() {
        let compiler = setup_compiler("test/build/tree-shaking_issues_271", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_jsx() {
        let compiler = setup_compiler("test/build/tree-shaking_jsx", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_dynamic_import() {
        let compiler = setup_compiler("test/build/tree-shaking_dynamic-import", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
        let content = read_dist_file(&compiler, "dist/a_ts-async.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_style() {
        let compiler = setup_compiler("test/build/tree-shaking_style", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_require_esm() {
        let compiler = setup_compiler("test/build/tree-shaking_require_esm", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_import_multi() {
        let compiler = setup_compiler("test/build/tree-shaking_import_multi", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn test_tree_shaking_import_self() {
        let compiler = setup_compiler("test/build/tree-shaking_import_self", false);
        compiler.compile().unwrap();
        let content = read_dist_file(&compiler, "dist/index.js");
        assert_display_snapshot!(content);
    }
}
