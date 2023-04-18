use swc_ecma_ast::{Module, ModuleDecl, ModuleItem};

use crate::{
    context::Context,
    module_graph::{Dependency, ResolveType},
};

pub struct AnalyzeDepsParam<'a> {
    pub path: &'a str,
    pub ast: &'a Module,
}

pub struct AnalyzeDepsResult {
    pub dependencies: Vec<Dependency>,
}

pub fn analyze_deps(
    analyze_deps_param: &AnalyzeDepsParam,
    _context: &Context,
) -> AnalyzeDepsResult {
    let mut dependencies = Vec::<Dependency>::new();
    let mut order = 0;
    // get dependencies from ast
    for node in &analyze_deps_param.ast.body {
        match node {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                let src = import.src.value.to_string();
                dependencies.push(Dependency {
                    source: src,
                    resolve_type: ResolveType::Import,
                    order,
                });
                order += 1;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
                if let Some(src) = &export.src {
                    let src = src.value.to_string();
                    dependencies.push(Dependency {
                        source: src,
                        resolve_type: ResolveType::ExportNamed,
                        order,
                    });
                }
                order += 1;
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(export)) => {
                let src = export.src.value.to_string();
                dependencies.push(Dependency {
                    source: src,
                    resolve_type: ResolveType::ExportAll,
                    order,
                });
                order += 1;
            }
            _ => {}
        }
    }

    AnalyzeDepsResult { dependencies }
}
