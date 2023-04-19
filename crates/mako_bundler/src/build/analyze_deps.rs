use swc_ecma_ast::*;
use swc_ecma_visit::noop_visit_type;
use swc_ecma_visit::{Visit, VisitWith};

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
    // get dependencies from require
    let mut collector = RequiresCollector::new(&mut dependencies);
    analyze_deps_param.ast.visit_with(&mut collector);

    AnalyzeDepsResult { dependencies }
}
pub struct RequiresCollector<'a> {
    pub requires: &'a mut Vec<Dependency>,
}

impl<'a> RequiresCollector<'a> {
    pub fn new(deps: &'a mut Vec<Dependency>) -> Self {
        RequiresCollector { requires: deps }
    }
}

impl Visit for RequiresCollector<'_> {
    noop_visit_type!();

    fn visit_call_expr(&mut self, expr: &CallExpr) {
        if let Callee::Expr(callee_expr) = &expr.callee {
            if let Expr::Ident(ident) = callee_expr.as_ref() {
                if ident.sym.to_string() == "require" && expr.args.len() == 1 {
                    println!("calling ---> {} ", ident.sym.to_string());
                    dbg!(expr.args[0].expr.as_ref());
                    if let Expr::Lit(Lit::Str(ref dep)) = expr.args[0].expr.as_ref() {
                        dbg!(dep.value.to_string());

                        self.requires.push(Dependency {
                            source: dep.value.to_string(),
                            resolve_type: ResolveType::Require,
                            order: 100,
                        });
                    }
                }
            }
        }
        expr.visit_children_with(self);
    }
}
