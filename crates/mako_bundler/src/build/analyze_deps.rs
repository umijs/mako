use swc_ecma_ast::{Module, ModuleDecl, ModuleItem};

use crate::context::Context;

pub struct AnalyzeDepsParam<'a> {
    pub path: &'a str,
    pub ast: &'a Module,
}

pub struct AnalyzeDepsResult {
    pub dependencies: Vec<String>,
}

pub fn analyze_deps(
    analyze_deps_param: &AnalyzeDepsParam,
    _context: &Context,
) -> AnalyzeDepsResult {
    let mut dependencies = Vec::<String>::new();

    // get dependencies from ast
    for node in &analyze_deps_param.ast.body {
        match node {
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                let src = import.src.value.to_string();
                dependencies.push(src);
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) => {
                if let Some(src) = &export.src {
                    let src = src.value.to_string();
                    dependencies.push(src);
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(export)) => {
                let src = export.src.value.to_string();
                dependencies.push(src);
            }
            _ => {}
        }
    }

    AnalyzeDepsResult { dependencies }
}
