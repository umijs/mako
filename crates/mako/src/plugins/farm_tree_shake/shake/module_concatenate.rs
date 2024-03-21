mod concatenate_context;
mod esm_hoister;
mod exports_transform;
mod external_transformer;
mod inner_transformer;
mod root_transformer;
mod utils;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use external_transformer::ExternalTransformer;
use inner_transformer::InnerTransform;
use mako_core::swc_common::util::take::Take;
use root_transformer::RootTransformer;
use swc_core::common::{Span, SyntaxContext, GLOBALS};
use swc_core::ecma::ast::{Id, ModuleItem};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::utils::collect_decls_with_ctxt;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use self::concatenate_context::EsmDependantFlags;
use self::utils::uniq_module_prefix;
use crate::ast::js_ast_to_code;
use crate::compiler::Context;
use crate::module::{generate_module_id, Dependency, ImportType, ModuleId, ResolveType};
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{AllExports, TreeShakeModule};
use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::{
    ConcatenateContext, RuntimeFlags,
};
use crate::plugins::farm_tree_shake::shake::module_concatenate::esm_hoister::EsmHoister;
use crate::tree_shaking::tree_shaking_module::ModuleSystem;

pub fn optimize_module_graph(
    module_graph: &mut ModuleGraph,
    tree_shake_modules_map: &HashMap<ModuleId, RefCell<TreeShakeModule>>,
    context: &Arc<Context>,
) -> mako_core::anyhow::Result<()> {
    let (sorted_module_ids, circles) = module_graph.toposort();

    let all_in_circles: HashSet<_> = circles.into_iter().flatten().collect();

    let mut root_candidates = vec![];
    let mut inner_candidates = HashSet::new();

    for (order, module_id) in sorted_module_ids.iter().enumerate() {
        // TODO async module should not be candidate

        // 环上的节点先不碰
        if all_in_circles.contains(module_id) {
            continue;
        }

        let can_be_root = true;
        let mut can_be_inner = true;
        /*
           - [ ] chunk 模式下的不能为 root
           - [ ] 被 import * all from 的不能为 inner
        */

        if let Some(tsm) = tree_shake_modules_map.get(module_id) {
            let mut tsm = tsm.borrow_mut();

            tsm.topo_order = order;

            if tsm.module_system != ModuleSystem::ESModule {
                continue;
            }

            let incoming_deps = module_graph.dependant_dependencies(module_id);
            let dynamic_imported = incoming_deps.iter().any(|&deps| {
                deps.iter()
                    .any(|d| matches!(d.resolve_type, ResolveType::DynamicImport))
            });

            if dynamic_imported {
                can_be_inner = false;
            }

            let export_all = module_graph
                .get_dependencies(module_id)
                .into_iter()
                .any(|(_, dep)| dep.resolve_type == ResolveType::ExportAll);
            if export_all {
                can_be_inner = false;
            }

            // 必须要有清晰的导出
            // ? 是不是不能有 export * from 'foo' 的语法
            // ： 可以有，但是不能有模糊的 export *
            if matches!(tsm.all_exports, AllExports::Ambiguous(_)) {
                can_be_inner = false;
            }

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
                ResolveType::DynamicImport => false,
                ResolveType::Css => false,
                ResolveType::Worker => false,
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
            if cfg!(debug_assertions) {
                dbg!(&config);
            }
            let mut module_items: Vec<ModuleItem> = Vec::new();

            let mut concatenate_context = ConcatenateContext::default();

            let runtime_flags = config.merged_runtime_flags();
            let (items, vars) = runtime_flags.interop_runtime_helpers();
            module_items.extend(items);
            concatenate_context.top_level_vars.extend(vars);

            for id in &config.sorted_modules(module_graph) {
                if id.eq(&config.root) {
                    continue;
                }

                let import_source_to_module_id = source_to_module_id(id, module_graph);

                if cfg!(debug_assertions) {
                    println!("\n*** start for {}", id.id);
                    println!(
                        "config.external_interops(id) {:?} ",
                        &config.external_interops(id)
                    );
                }

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

                    let require_src = generate_module_id(id.id.clone(), context);
                    module_items
                        .extend(interop.inject_external_export_decl(&require_src, &exposed_names));

                    concatenate_context.add_external_names(id, exposed_names);

                    module_graph.add_dependency(
                        &config.root,
                        id,
                        Dependency {
                            source: id.id.clone(),
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

                let p = false;
                if cfg!(debug_assertions) && p {
                    let code_map = js_ast_to_code(&script_ast.ast, context, &id.id).unwrap();
                    println!("code:\n\n{}\n", code_map.0);
                }

                let mut current_module_top_level_vars: HashSet<String> = collect_decls_with_ctxt(
                    &script_ast.ast,
                    SyntaxContext::empty().apply_mark(script_ast.top_level_mark),
                )
                .iter()
                .map(|id: &Id| id.0.to_string())
                .collect();

                script_ast.ast.visit_mut_with(&mut EsmHoister::new());

                let mut ext_trans = ExternalTransformer {
                    src_to_module: &import_source_to_module_id,
                    concatenate_context: &mut concatenate_context,
                    module_id: id,
                    unresolved_mark: script_ast.unresolved_mark,
                    my_top_level_vars: &mut current_module_top_level_vars,
                };
                script_ast.ast.visit_mut_with(&mut ext_trans);

                if cfg!(debug_assertions) && p {
                    let code_map = js_ast_to_code(&script_ast.ast, context, &id.id).unwrap();
                    println!("after external:\n{}\n", code_map.0);
                }
                let mut inner_transformer = InnerTransform::new(
                    &mut concatenate_context,
                    id,
                    &import_source_to_module_id,
                    context,
                    script_ast.top_level_mark,
                );
                inner_transformer.imported(all_import_type);

                script_ast.ast.visit_mut_with(&mut inner_transformer);
                script_ast.ast.visit_mut_with(&mut CleanSyntaxContext {});

                if cfg!(debug_assertions) && p {
                    let code_map = js_ast_to_code(&script_ast.ast, context, &id.id).unwrap();
                    println!("after inner:\n{}\n", code_map.0);
                }
                module_items.append(&mut script_ast.ast.body.clone());
            }

            let root_module = module_graph.get_module_mut(&config.root).unwrap();

            let ast = &mut root_module.info.as_mut().unwrap().ast;

            let ast_script = ast.script_mut().unwrap();

            let mut root_module_ast = ast_script.ast.take();
            let unresolved_mark = ast_script.unresolved_mark;
            let top_level_mark = ast_script.top_level_mark;
            let src_2_module_id = source_to_module_id(&config.root, module_graph);

            let p = false;
            if cfg!(debug_assertions) && p {
                let code_map = js_ast_to_code(&root_module_ast, context, &config.root.id).unwrap();
                println!("root:\n{}\n", code_map.0);
            }

            let mut current_module_top_level_vars: HashSet<String> = collect_decls_with_ctxt(
                &root_module_ast,
                SyntaxContext::empty().apply_mark(top_level_mark),
            )
            .iter()
            .map(|id: &Id| id.0.to_string())
            .collect();

            let mut ext_trans = ExternalTransformer {
                src_to_module: &src_2_module_id,
                concatenate_context: &mut concatenate_context,
                module_id: &config.root,
                unresolved_mark,
                my_top_level_vars: &mut current_module_top_level_vars,
            };
            root_module_ast.visit_mut_with(&mut ext_trans);

            root_module_ast.visit_mut_with(&mut RootTransformer::new(
                &mut concatenate_context,
                &config.root,
                context,
                top_level_mark,
                &src_2_module_id,
            ));

            root_module_ast.visit_mut_with(&mut CleanSyntaxContext {});

            root_module_ast.body.splice(0..0, module_items);
            root_module_ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));

            if cfg!(debug_assertions) && p {
                let code_map = js_ast_to_code(&root_module_ast, context, &config.root.id).unwrap();
                println!("root after all:\n{}\n", code_map.0);
            }

            let root_module = module_graph.get_module_mut(&config.root).unwrap();
            let ast = &mut root_module.info.as_mut().unwrap().ast;
            let ast_script = ast.script_mut().unwrap();
            ast_script.ast = root_module_ast;

            for inner in config.inners.iter() {
                module_graph.remove_module(inner);
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

impl ConcatenateConfig {}

pub struct CleanSyntaxContext;

impl VisitMut for CleanSyntaxContext {
    fn visit_mut_span(&mut self, n: &mut Span) {
        n.ctxt = SyntaxContext::empty();
    }
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
