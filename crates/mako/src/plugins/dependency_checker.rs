mod collect_exports;
mod collect_imports;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLockReadGuard};

use anyhow::Result;
use collect_exports::CollectExports;
use collect_imports::CollectImports;
use swc_core::ecma::visit::VisitWith;
use tracing::error;

use crate::compiler::{Compiler, Context};
use crate::module::{ModuleId, ModuleSystem};
use crate::module_graph::ModuleGraph;
use crate::plugin::Plugin;

pub struct DependencyChecker {}

fn pick_no_export_specifiers_with_imports_info(
    module_id: &ModuleId,
    module_graph: &RwLockReadGuard<ModuleGraph>,
    specifiers: &mut HashSet<String>,
) {
    if !specifiers.is_empty() {
        let dep_module = module_graph.get_module(module_id).unwrap();
        if let Some(info) = &dep_module.info {
            match info.module_system {
                ModuleSystem::ESModule => {
                    let mut exports_star_sources: Vec<String> = vec![];
                    let ast = &info.ast.as_script().unwrap().ast;
                    ast.visit_with(&mut CollectExports {
                        specifiers,
                        exports_star_sources: &mut exports_star_sources,
                    });
                    exports_star_sources.into_iter().for_each(|source| {
                        if let Some(id) =
                            module_graph.get_dependency_module_by_source(module_id, &source)
                        {
                            pick_no_export_specifiers_with_imports_info(
                                id,
                                module_graph,
                                specifiers,
                            );
                        }
                    })
                }
                ModuleSystem::CommonJS | ModuleSystem::Custom => {
                    specifiers.clear();
                }
            }
        }
    }
}
impl Plugin for DependencyChecker {
    fn name(&self) -> &str {
        "dependency_checker"
    }
    fn after_build(&self, context: &Arc<Context>, _compiler: &Compiler) -> Result<()> {
        let mut modules_imports_map: HashMap<&ModuleId, HashMap<String, HashSet<String>>> =
            HashMap::new();

        let module_graph = context.module_graph.read().unwrap();
        let modules = module_graph.modules();

        for m in modules {
            if let Some(info) = &m.info {
                if !info.file.is_under_node_modules
                    && matches!(info.module_system, ModuleSystem::ESModule)
                {
                    // 收集 imports
                    let ast = &info.ast.as_script().unwrap().ast;
                    let mut import_specifiers: HashMap<String, HashSet<String>> = HashMap::new();

                    ast.visit_with(&mut CollectImports {
                        imports_specifiers_with_source: &mut import_specifiers,
                    });
                    modules_imports_map.insert(&m.id, import_specifiers);
                }
            }
        }
        // 收集 exports
        modules_imports_map
            .iter_mut()
            .for_each(|(module_id, import_specifiers)| {
                import_specifiers
                    .iter_mut()
                    .for_each(|(source, specifiers)| {
                        if let Some(dep_module_id) =
                            module_graph.get_dependency_module_by_source(module_id, source)
                        {
                            pick_no_export_specifiers_with_imports_info(
                                dep_module_id,
                                &module_graph,
                                specifiers,
                            );
                        }
                    })
            });
        let mut should_panic = false;
        modules_imports_map
            .into_iter()
            .for_each(|(module_id, import_specifiers)| {
                import_specifiers
                    .into_iter()
                    .filter(|(_, specifiers)| !specifiers.is_empty())
                    .for_each(|(source, specifiers)| {
                        should_panic = true;
                        specifiers.iter().for_each(|specifier| {
                            error!(
                                "'{}' is undefined: import from '{}' in '{}'",
                                specifier, source, module_id.id
                            );
                        })
                    });
            });
        if should_panic {
            panic!("dependency check error!");
        };
        Ok(())
    }
}
