use std::collections::HashMap;

use swc_ecma_ast::{Expr, ExprOrSpread, Lit, Str};
use swc_ecma_visit::{VisitMut, VisitMutWith};

use crate::analyze_deps::{is_commonjs_require, is_dynamic_import};

pub struct DepReplacer {
    pub dep_map: HashMap<String, String>,
}

impl VisitMut for DepReplacer {
    // fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
    //     self.replace_source(&mut *import_decl.src);
    // }

    // fn visit_mut_export_all(&mut self, export_all: &mut ExportAll) {
    //     self.replace_source(&mut *export_all.src);
    // }

    // fn visit_mut_named_export(&mut self, named_export: &mut NamedExport) {
    //     if let Some(src) = &mut named_export.src {
    //         self.replace_source(src);
    //     }
    // }

    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(call_expr) = expr {
            if is_commonjs_require(call_expr) || is_dynamic_import(call_expr) {
                if let ExprOrSpread {
                    expr: box Expr::Lit(Lit::Str(ref mut source)),
                    ..
                } = &mut call_expr.args[0]
                {
                    self.replace_source(source);
                }
            }
        }
        expr.visit_mut_children_with(self);
    }
}

impl DepReplacer {
    fn replace_source(&mut self, source: &mut Str) {
        if let Some(replacement) = self.dep_map.get(&source.value.to_string()) {
            let span = source.span;

            // NOTE: JsWord 有缓存，直接设置 value 的方式在这种情况下不会生效
            // if (process.env.NODE_ENV === 'development') { require("./foo") }
            *source = Str::from(replacement.clone());
            // 保持原来的 span，不确定不加的话会不会导致 sourcemap 错误
            source.span = span;
        }
    }
}
