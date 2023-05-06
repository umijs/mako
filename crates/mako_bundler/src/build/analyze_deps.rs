use swc_css_ast::*;
use swc_css_visit::{Visit as CssVisit, VisitWith as CssVisitWith};
use swc_ecma_ast::*;
use swc_ecma_visit::noop_visit_type;
use swc_ecma_visit::{Visit, VisitWith};

use crate::{
    context::Context,
    module::ModuleAst,
    module_graph::{Dependency, ResolveType},
};

pub struct AnalyzeDepsParam<'a> {
    pub path: &'a str,
    pub ast: &'a ModuleAst,
    pub transform_ast: &'a ModuleAst,
}

pub struct AnalyzeDepsResult {
    pub dependencies: Vec<Dependency>,
}

pub fn analyze_deps(
    analyze_deps_param: &AnalyzeDepsParam,
    _context: &Context,
) -> AnalyzeDepsResult {
    // get dependencies from ast
    let mut collector = DepsCollector::new();

    if let ModuleAst::Script(ast) = analyze_deps_param.ast {
        ast.visit_with(&mut collector);
        if let ModuleAst::Script(transform_ast) = analyze_deps_param.transform_ast {
            // transform ast 分析到的都是 require
            // TODO: only analyze top level require to improve performance
            transform_ast.visit_with(&mut collector);

            println!("> analyze deps: {}", analyze_deps_param.path);
            for d in &collector.dependencies {
                println!("  - {} ({:?})", d.source, d.resolve_type);
            }
        }
    } else if let ModuleAst::Css(stylesheet) = analyze_deps_param.ast {
        stylesheet.visit_with(&mut collector);
    }

    AnalyzeDepsResult {
        dependencies: collector.dependencies,
    }
}
pub struct DepsCollector {
    order: usize,
    pub dependencies: Vec<Dependency>,
    pub dep_strs: Vec<String>,
}

impl DepsCollector {
    pub fn new() -> Self {
        DepsCollector {
            dependencies: Vec::new(),
            dep_strs: Vec::new(),
            order: 0,
        }
    }

    fn bind_dependencies(&mut self, dependency: Dependency) {
        if !self.dep_strs.contains(&dependency.source) {
            self.dep_strs.push(dependency.source.clone());
            self.dependencies.push(dependency);
            self.order += 1;
        }
    }
}

impl Visit for DepsCollector {
    noop_visit_type!();

    fn visit_module_decl(&mut self, n: &ModuleDecl) {
        match n {
            ModuleDecl::Import(import) => {
                let src = import.src.value.to_string();
                self.bind_dependencies(Dependency {
                    source: src,
                    resolve_type: ResolveType::Import,
                    order: self.order,
                });
            }
            ModuleDecl::ExportNamed(export) => {
                if let Some(src) = &export.src {
                    let src = src.value.to_string();
                    self.bind_dependencies(Dependency {
                        source: src,
                        resolve_type: ResolveType::ExportNamed,
                        order: self.order,
                    });
                }
            }
            ModuleDecl::ExportAll(export) => {
                let src = export.src.value.to_string();
                self.bind_dependencies(Dependency {
                    source: src,
                    resolve_type: ResolveType::ExportAll,
                    order: self.order,
                });
            }
            _ => {}
        }
    }

    fn visit_call_expr(&mut self, expr: &CallExpr) {
        if is_commonjs_require(expr) {
            if let Expr::Lit(Lit::Str(dep)) = expr.args[0].expr.as_ref() {
                self.bind_dependencies(Dependency {
                    source: dep.value.to_string(),
                    resolve_type: ResolveType::Require,
                    order: self.order,
                });
            }
        } else if is_dynamic_import(expr) {
            if let Expr::Lit(Lit::Str(dep)) = expr.args[0].expr.as_ref() {
                self.bind_dependencies(Dependency {
                    source: dep.value.to_string(),
                    resolve_type: ResolveType::DynamicImport,
                    order: self.order,
                });
            }
        }
        expr.visit_children_with(self);
    }
}

impl CssVisit for DepsCollector {
    fn visit_import_href(&mut self, n: &ImportHref) {
        // 检查 @import
        if let ImportHref::Url(url) = n {
            let href_string = url
                .value
                .as_ref()
                .map(|box value| match value {
                    UrlValue::Str(str) => str.value.to_string(),
                    UrlValue::Raw(raw) => raw.value.to_string(),
                })
                .unwrap();
            self.bind_dependencies(Dependency {
                source: href_string,
                resolve_type: ResolveType::CssImportUrl,
                order: self.order,
            });
        } else if let ImportHref::Str(str) = n {
            self.bind_dependencies(Dependency {
                source: str.value.to_string(),
                resolve_type: ResolveType::CssImportStr,
                order: self.order,
            });
        }
        n.visit_children_with(self);
    }

    fn visit_url(&mut self, n: &Url) {
        // 检查 url 属性
        let href_string = n
            .value
            .as_ref()
            .map(|box value| match value {
                UrlValue::Str(str) => str.value.to_string(),
                UrlValue::Raw(raw) => raw.value.to_string(),
            })
            .unwrap();
        self.bind_dependencies(Dependency {
            source: href_string,
            resolve_type: ResolveType::CssImportStr,
            order: self.order,
        });
        n.visit_children_with(self);
    }
}

pub fn is_dynamic_import(call_expr: &CallExpr) -> bool {
    matches!(&call_expr.callee, Callee::Import(Import { .. }))
}
pub fn is_commonjs_require(call_expr: &CallExpr) -> bool {
    if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident { sym, .. })) = &call_expr.callee {
        sym == "require"
    } else {
        false
    }
}
