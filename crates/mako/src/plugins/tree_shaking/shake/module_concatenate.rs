mod concatenate_context;
mod concatenated_transformer;
mod external_transformer;
mod module_ref_rewriter;
mod ref_link;
mod utils;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use concatenated_transformer::ConcatenatedTransform;
use external_transformer::ExternalTransformer;
use swc_core::common::util::take::Take;
use swc_core::common::GLOBALS;
use swc_core::ecma::transforms::base::hygiene::hygiene;
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::visit::VisitMutWith;

use self::concatenate_context::EsmDependantFlags;
use self::utils::uniq_module_prefix;
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::module::{Dependency, ImportType, ModuleId, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::tree_shaking::module::{AllExports, ModuleSystem, TreeShakeModule};
use crate::plugins::tree_shaking::shake::module_concatenate::concatenate_context::{
    ConcatenateContext, RuntimeFlags,
};
use crate::visitors::clean_ctxt::clean_syntax_context;
use crate::{mako_profile_function, mako_profile_scope};

pub fn optimize_module_graph(
    module_graph: &mut ModuleGraph,
    tree_shake_modules_map: &HashMap<ModuleId, RefCell<TreeShakeModule>>,
    context: &Arc<Context>,
) -> anyhow::Result<()> {
    mako_profile_function!();

    let (sorted_module_ids, circles) = module_graph.toposort();

    let all_in_circles: HashSet<_> = circles.into_iter().flatten().collect();

    let mut root_candidates = vec![];
    let mut inner_candidates = HashSet::new();

    for (order, module_id) in sorted_module_ids.iter().enumerate() {
        if all_in_circles.contains(module_id) {
            continue;
        }

        let mut can_be_root = true;
        let mut can_be_inner = true;

        if let Some(tsm) = tree_shake_modules_map.get(module_id) {
            let mut tsm = tsm.borrow_mut();

            tsm.topo_order = order;

            if tsm.module_system != ModuleSystem::ESModule {
                continue;
            }

            let incoming_deps = module_graph.dependant_dependencies(module_id);
            let dynamic_imported = incoming_deps.iter().any(|&deps| {
                deps.iter()
                    .any(|d| matches!(d.resolve_type, ResolveType::DynamicImport(_)))
            });

            if dynamic_imported {
                can_be_inner = false;
            }

            let deps = module_graph.get_dependencies_info(module_id);

            let has_not_supported_syntax = deps.iter().any(|(_, dep, is_async)| {
                dep.resolve_type.is_dynamic_esm()
                    || matches!(dep.resolve_type, ResolveType::Worker(_))
                    || (*is_async && dep.resolve_type.is_sync_esm())
            });
            if has_not_supported_syntax {
                can_be_inner = false;
                can_be_root = false;
            }

            let has_export_star = deps
                .iter()
                .any(|(_, dep, _)| matches!(dep.resolve_type, ResolveType::ExportAll));
            // 必须要有清晰的导出
            // ? 是不是不能有 export * from 'foo' 的语法
            // ： 可以有，但是不能有模糊的 export *
            if matches!(tsm.all_exports, AllExports::Ambiguous(_)) || has_export_star {
                can_be_inner = false;
            }

            module_graph
                .get_module(module_id)
                .and_then(|module| module.info.as_ref())
                .inspect(|info| {
                    if info.is_async || info.is_ignored {
                        can_be_inner = false;
                        can_be_root = false;
                    }
                });

            if can_be_root {
                root_candidates.push(module_id.clone());
            }

            if can_be_inner {
                inner_candidates.insert(module_id.clone());
            }
        }
    }

    let mut concat_configurations = vec![];
    let mut used_as_inner = HashSet::new();

    fn collect_inner_modules(
        current_module_id: &ModuleId,
        config: &mut ConcatenateConfig,
        candidate: &HashSet<ModuleId>,
        module_graph: &ModuleGraph,
    ) {
        if current_module_id.ne(&config.root) {
            let parents = module_graph.dependant_module_ids(current_module_id);

            let is_all_parents_in_config = parents.iter().all(|p| config.contains(p));

            if !is_all_parents_in_config {
                config.add_external(current_module_id.clone());
                return;
            }
        }

        let deps = module_graph.get_dependencies(current_module_id);

        let mut children = vec![];

        for (module_id, _dep) in deps {
            let esm_import = match _dep.resolve_type {
                ResolveType::Import(_) => true,
                ResolveType::ExportNamed(_) => true,
                ResolveType::ExportAll => true,
                ResolveType::Require => false,
                ResolveType::DynamicImport(_) => false,
                ResolveType::Css => false,
                ResolveType::Worker(_) => false,
            };

            if candidate.contains(module_id) && esm_import {
                if !(config.contains(module_id)) {
                    config.add_inner(module_id.clone());
                    children.push(module_id.clone());
                }
            } else {
                config.add_external(module_id.clone());
            }
        }

        for m in children.iter() {
            collect_inner_modules(m, config, candidate, module_graph);
        }
    }

    fn extends_external_modules(config: &mut ConcatenateConfig, module_graph: &ModuleGraph) {
        let mut visited = HashSet::new();

        for (ext, _) in config.externals.iter() {
            if visited.contains(ext) {
                continue;
            }

            let mut dfs = module_graph.dfs(ext);

            while let Some(node) = dfs.next(&module_graph.graph) {
                let m = &module_graph.graph[node];

                if visited.contains(&m.id) {
                    continue;
                }

                visited.insert(m.id.clone());
            }
        }
        for visited in visited {
            if config.inners.contains(&visited) {
                config.inners.remove(&visited);
                config.externals.insert(visited, Default::default());
            }
        }
    }

    for root in root_candidates.iter() {
        if used_as_inner.contains(root) {
            continue;
        }
        let mut config = ConcatenateConfig::new(root.clone());

        collect_inner_modules(root, &mut config, &inner_candidates, module_graph);

        extends_external_modules(&mut config, module_graph);

        if config.is_empty() {
        } else {
            used_as_inner.insert(config.root.clone());
            used_as_inner.extend(config.inners.iter().cloned());

            if config.externals.is_empty() {
                // wildcard interop can handle all interop case
            } else {
                for module_id in &config.inners {
                    for (dep_module_id, dep) in module_graph.get_dependencies(module_id) {
                        if let Some(curr_interops) = config.externals.get_mut(dep_module_id) {
                            let it: EsmDependantFlags = (&dep.resolve_type).into();
                            curr_interops.insert(it);
                        }
                    }
                }

                for (dep_module_id, dep) in module_graph.get_dependencies(&config.root) {
                    if let Some(curr_interops) = config.externals.get_mut(dep_module_id) {
                        let it: EsmDependantFlags = (&dep.resolve_type).into();
                        curr_interops.insert(it);
                    }
                }
            }

            concat_configurations.push(config);
        }
    }

    fn source_to_module_id(module_id: &ModuleId, mg: &ModuleGraph) -> HashMap<String, ModuleId> {
        let dep = mg.get_dependencies(module_id);

        let mut src_2_module_id = HashMap::new();

        for (module, dep) in dep {
            src_2_module_id.insert(dep.source.clone(), module.clone());
        }

        src_2_module_id
    }

    GLOBALS.set(&context.meta.script.globals, || {
        for config in &concat_configurations {
            mako_profile_scope!("concatenate", &config.root.id);

            if let Some(info) = module_graph
                .get_module_mut(&config.root)
                .and_then(|module| module.info.as_mut())
            {
                let js_ast = info.ast.as_script_mut();

                js_ast.ast.visit_mut_with(&mut hygiene());
                js_ast.ast.visit_mut_with(&mut resolver(
                    js_ast.unresolved_mark,
                    js_ast.top_level_mark,
                    false,
                ));
            }

            if let Ok(mut concatenate_context) = ConcatenateContext::init(config, module_graph) {
                let mut module_items = concatenate_context.interop_module_items.clone();

                for id in &config.sorted_modules(module_graph) {
                    if id.eq(&config.root) {
                        continue;
                    }

                    let import_source_to_module_id = source_to_module_id(id, module_graph);

                    if let Some(interop) = config.external_interops(id) {
                        let base_name = uniq_module_prefix(id);
                        let runtime_flags: RuntimeFlags = interop.into();

                        let cjs_name = concatenate_context.request_safe_var_name(&base_name);
                        let exposed_names = if runtime_flags.need_op() {
                            let esm_name = concatenate_context
                                .request_safe_var_name(&format!("{}_esm", base_name));
                            (cjs_name, esm_name)
                        } else {
                            (cjs_name.clone(), cjs_name)
                        };

                        let require_src = id.id.clone();
                        module_items.extend(interop.inject_external_export_decl(
                            &require_src,
                            &exposed_names,
                            &concatenate_context.interop_idents,
                        ));

                        concatenate_context.add_external_names(id, exposed_names);

                        module_graph.add_dependency(
                            &config.root,
                            id,
                            Dependency {
                                source: require_src,
                                resolve_as: None,
                                resolve_type: ResolveType::Require,
                                order: 0,
                                span: None,
                            },
                        );
                        continue;
                    }

                    let mut all_import_type = ImportType::empty();

                    module_graph.get_dependents(id).iter().for_each(|(_, dep)| {
                        match &dep.resolve_type {
                            ResolveType::Import(import_type) => all_import_type |= *import_type,
                            ResolveType::ExportNamed(named_export_type) => {
                                all_import_type |= named_export_type.into();
                            }
                            _ => {}
                        }
                    });

                    let module = module_graph.get_module_mut(id).unwrap();

                    let module_info = module.info.as_mut().unwrap();
                    let script_ast = module_info.ast.script_mut().unwrap();
                    script_ast.ast.visit_mut_with(&mut hygiene());
                    script_ast.ast.visit_mut_with(&mut resolver(
                        script_ast.unresolved_mark,
                        script_ast.top_level_mark,
                        false,
                    ));

                    let inner_print = false;
                    if cfg!(debug_assertions) && inner_print {
                        let code = script_ast.generate(context.clone()).unwrap().code;
                        println!("code:\n\n{}\n", code);
                    }

                    let mut ext_trans = ExternalTransformer {
                        src_to_module: &import_source_to_module_id,
                        concatenate_context: &mut concatenate_context,
                        unresolved_mark: script_ast.unresolved_mark,
                    };
                    script_ast.ast.visit_mut_with(&mut ext_trans);

                    if cfg!(debug_assertions) && inner_print {
                        let code = script_ast.generate(context.clone()).unwrap().code;
                        println!("after external:\n{}\n", code);
                    }
                    let mut ccn_trans = ConcatenatedTransform::new(
                        &mut concatenate_context,
                        id,
                        &import_source_to_module_id,
                        context,
                        script_ast.top_level_mark,
                    );
                    ccn_trans.imported(all_import_type);

                    script_ast.ast.visit_mut_with(&mut ccn_trans);
                    script_ast.ast.visit_mut_with(&mut clean_syntax_context());

                    if cfg!(debug_assertions) && inner_print {
                        let code = script_ast.generate(context.clone()).unwrap().code;
                        println!("after inner:\n{}\n", code);
                    }
                    module_items.append(&mut script_ast.ast.body.clone());
                }

                let root_module = module_graph.get_module_mut(&config.root).unwrap();

                let ast = &mut root_module.info.as_mut().unwrap().ast;

                let ast_script = ast.script_mut().unwrap();
                let root_print = false;
                if cfg!(debug_assertions) && root_print {
                    let code = ast_script.generate(context.clone()).unwrap().code;
                    println!("root:\n{}\n", code);
                }

                let mut root_module_ast = ast_script.ast.take();
                let unresolved_mark = ast_script.unresolved_mark;
                let top_level_mark = ast_script.top_level_mark;
                let src_2_module_id = source_to_module_id(&config.root, module_graph);

                let mut ext_trans = ExternalTransformer {
                    src_to_module: &src_2_module_id,
                    concatenate_context: &mut concatenate_context,
                    unresolved_mark,
                };
                root_module_ast.visit_mut_with(&mut ext_trans);

                let mut ccn_trans_for_root = ConcatenatedTransform::new(
                    &mut concatenate_context,
                    &config.root,
                    &src_2_module_id,
                    context,
                    top_level_mark,
                )
                .for_root();

                root_module_ast.visit_mut_with(&mut ccn_trans_for_root);

                if cfg!(debug_assertions) && root_print {
                    let a = JsAst {
                        ast: root_module_ast.clone(),
                        unresolved_mark,
                        top_level_mark,
                        path: config.root.id.clone(),
                        contains_top_level_await: false,
                    };

                    let code = a.generate(context.clone()).unwrap().code;
                    println!("root after all:\n{}\n", code);
                }

                root_module_ast.visit_mut_with(&mut clean_syntax_context());

                let prefix_items = concatenate_context.root_exports_stmts(&config.root);
                module_items.splice(0..0, prefix_items);

                root_module_ast.body.splice(0..0, module_items);
                root_module_ast.visit_mut_with(&mut resolver(
                    unresolved_mark,
                    top_level_mark,
                    false,
                ));

                let root_module = module_graph.get_module_mut(&config.root).unwrap();
                let ast = &mut root_module.info.as_mut().unwrap().ast;
                let ast_script = ast.script_mut().unwrap();
                ast_script.ast = root_module_ast;

                for inner in config.inners.iter() {
                    module_graph.remove_module(inner);
                }
            } else {
                continue;
            }
        }
        Ok(())
    })
}

#[derive(Debug)]
struct ConcatenateConfig {
    root: ModuleId,
    inners: HashSet<ModuleId>,
    externals: HashMap<ModuleId, EsmDependantFlags>,
}

impl ConcatenateConfig {
    fn new(root: ModuleId) -> Self {
        Self {
            root,
            inners: Default::default(),
            externals: Default::default(),
        }
    }

    fn add_inner(&mut self, inner: ModuleId) {
        self.inners.insert(inner);
    }

    pub fn contains(&self, module_id: &ModuleId) -> bool {
        self.root.eq(module_id) || self.inners.contains(module_id)
    }

    pub fn is_empty(&self) -> bool {
        self.inners.is_empty()
    }

    pub fn add_external(&mut self, external_id: ModuleId) {
        self.externals.insert(external_id, Default::default());
    }

    pub fn sorted_modules(&self, module_graph: &ModuleGraph) -> Vec<ModuleId> {
        let mut modules = vec![];

        fn walk<'a>(
            root: &'a ModuleId,
            orders: &mut Vec<ModuleId>,
            module_graph: &'a ModuleGraph,
            config: &ConcatenateConfig,
        ) {
            let deps = module_graph.get_dependencies(root);

            if deps.is_empty() {
                orders.push(root.clone());
                return;
            }

            for (module_id, _dep) in deps {
                if orders.contains(module_id) {
                    continue;
                }

                if config.is_external(module_id) {
                    orders.push(module_id.clone());
                    continue;
                }

                if config.contains(module_id) {
                    walk(module_id, orders, module_graph, config);
                }
            }

            orders.push(root.clone());
        }

        walk(&self.root, &mut modules, module_graph, self);

        modules
    }
    fn is_external(&self, module_id: &ModuleId) -> bool {
        self.externals.contains_key(module_id)
    }

    fn external_interops(&self, module_id: &ModuleId) -> Option<EsmDependantFlags> {
        self.externals.get(module_id).copied()
    }

    fn merged_runtime_flags(&self) -> RuntimeFlags {
        let mut rt_flags = RuntimeFlags::empty();

        self.externals.iter().for_each(|(_, v)| {
            let f: RuntimeFlags = v.into();
            rt_flags |= f;
        });

        rt_flags
    }
}
