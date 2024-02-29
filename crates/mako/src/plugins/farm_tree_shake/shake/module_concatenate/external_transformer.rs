use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    EmptyStmt, ExportNamedSpecifier, ExportSpecifier, Expr, ExprOrSpread, ImportDecl,
    ImportSpecifier, Lit, MemberExpr, Module, ModuleDecl, ModuleExportName, ModuleItem,
    NamedExport, Stmt, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_core::ecma::utils::{member_expr, quote_ident, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::ModuleId;
use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::ConcatenateContext;
use crate::plugins::farm_tree_shake::shake::module_concatenate::utils::{
    uniq_module_default_export_name, uniq_module_export_name,
};
use crate::plugins::javascript::is_commonjs_require;

macro_rules! var {
    ($left:ident = $init:expr) => {
        VarDeclarator {
            span: $left.span,
            name: $left.local.clone().into(),
            init: Some($init.into()),
            definite: false,
        }
    };
}

pub(super) struct ExternalTransformer<'a> {
    pub concatenate_context: &'a mut ConcatenateContext,
    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,
    pub unresolved_mark: Mark,
    pub my_top_level_vars: &'a mut HashSet<String>,
}

impl<'a> ExternalTransformer<'_> {
    fn src_to_export_name(&'a self, src: &str) -> Option<((String, String), ModuleId)> {
        self.src_to_module.get(src).and_then(|module_id| {
            self.concatenate_context
                .external_expose_names(module_id)
                .map(|export_names| (export_names.clone(), module_id.clone()))
        })
    }

    fn request_safe_var_name(&self, name: &str) -> String {
        self.concatenate_context
            .negotiate_safe_var_name(self.my_top_level_vars, name)
    }

    fn named_export_specifier_to_replace_items(
        &mut self,
        src_module_id: &ModuleId,
        specifiers: &Vec<ExportSpecifier>,
        external_module_namespace: &str,
    ) -> Vec<ModuleItem> {
        let mut var_decorators: Vec<ExportSpecifier> = vec![];

        let mut stmts: Vec<ModuleItem> = vec![];

        for spec in specifiers {
            match spec {
                ExportSpecifier::Namespace(namespace) => {
                    match &namespace.name {
                        ModuleExportName::Ident(local) => {
                            let spec = ExportNamedSpecifier {
                                span: DUMMY_SP,
                                orig: ModuleExportName::Ident(quote_ident!(
                                    external_module_namespace.to_string()
                                )),
                                exported: Some(ModuleExportName::Ident(local.clone())),
                                is_type_only: false,
                            };
                            var_decorators.push(spec.into());
                        }
                        ModuleExportName::Str(_) => {}
                    };
                }
                ExportSpecifier::Default(default_spec) => {
                    // when support export default from "m"
                    let local_default_name = if default_spec.exported.sym.eq("default") {
                        quote_ident!(self.request_safe_var_name(&uniq_module_default_export_name(
                            self.module_id,
                            self.context
                        )))
                    } else {
                        quote_ident!(self.request_safe_var_name(&uniq_module_export_name(
                            src_module_id,
                            &default_spec.exported.sym,
                            self.context
                        )))
                    };

                    let default_export_var_decl = MemberExpr {
                        span: DUMMY_SP,
                        obj: quote_ident!(external_module_namespace.to_string()).into(),
                        prop: quote_ident!("default").into(),
                    }
                    .into_var_decl(VarDeclKind::Var, local_default_name.clone().into());

                    stmts.push(default_export_var_decl.into());

                    let spec = ExportNamedSpecifier {
                        span: DUMMY_SP,
                        orig: ModuleExportName::Ident(local_default_name),
                        exported: Some(ModuleExportName::Ident(default_spec.exported.clone())),
                        is_type_only: false,
                    };
                    var_decorators.push(spec.into());
                }
                ExportSpecifier::Named(named) => match (&named.orig, &named.exported) {
                    (ModuleExportName::Ident(orig), Some(ModuleExportName::Ident(exported))) => {
                        let local_proxy_name = if exported.sym.eq("default") {
                            quote_ident!(self.request_safe_var_name(
                                &uniq_module_default_export_name(self.module_id, self.context)
                            ))
                        } else {
                            quote_ident!(self.request_safe_var_name(&uniq_module_export_name(
                                src_module_id,
                                &orig.sym,
                                self.context
                            )))
                        };

                        let export_name_decl = MemberExpr {
                            span: DUMMY_SP,
                            obj: quote_ident!(external_module_namespace.to_string()).into(),
                            prop: orig.clone().into(),
                        }
                        .into_var_decl(VarDeclKind::Var, local_proxy_name.clone().into());
                        stmts.push(export_name_decl.into());

                        var_decorators.push(
                            ExportNamedSpecifier {
                                span: Default::default(),
                                orig: local_proxy_name.into(),
                                exported: named.exported.clone(),
                                is_type_only: false,
                            }
                            .into(),
                        );
                    }
                    (ModuleExportName::Ident(orig), None) => {
                        let local_proxy_name = if orig.sym.eq("default") {
                            quote_ident!(self.request_safe_var_name(
                                &uniq_module_default_export_name(self.module_id, self.context)
                            ))
                        } else {
                            quote_ident!(self.request_safe_var_name(&uniq_module_export_name(
                                src_module_id,
                                &orig.sym,
                                self.context
                            )))
                        };

                        let var_decl = MemberExpr {
                            span: DUMMY_SP,
                            obj: quote_ident!(external_module_namespace.to_string()).into(),
                            prop: orig.clone().into(),
                        }
                        .into_var_decl(VarDeclKind::Var, local_proxy_name.clone().into());
                        stmts.push(var_decl.into());

                        var_decorators.push(
                            ExportNamedSpecifier {
                                span: DUMMY_SP,
                                orig: local_proxy_name.clone().into(),
                                exported: if local_proxy_name.sym.eq(&orig.sym) {
                                    None
                                } else {
                                    Some(ModuleExportName::Ident(orig.clone()))
                                },
                                is_type_only: false,
                            }
                            .into(),
                        );
                    }
                    (_, _) => {}
                },
            }
        }

        let md: ModuleDecl = NamedExport {
            span: Default::default(),
            specifiers: var_decorators,
            src: None,
            type_only: false,
            with: None,
        }
        .into();
        stmts.push(md.into());

        stmts
    }

    fn require_arg_to_module_namespace(
        &self,
        args: &Vec<ExprOrSpread>,
    ) -> Option<((String, String), ModuleId)> {
        if args.len() == 1
            && let Some(arg) = args.first()
            && arg.spread.is_none()
            && let Some(lit) = arg.expr.as_lit()
            && let Lit::Str(str) = lit
        {
            self.src_to_export_name(str.value.as_ref())
        } else {
            None
        }
    }

    fn import_decl_to_var_decl(
        &self,
        import_decl: &ImportDecl,
        namespace: &str,
    ) -> Option<VarDecl> {
        let var_decorators: Vec<_> = import_decl
            .specifiers
            .iter()
            .map(|spec| Self::import_specifier_to_var_declarator(spec, namespace))
            .collect();
        let mut var_decl = VarDecl {
            span: import_decl.span,
            decls: var_decorators,
            declare: false,
            kind: VarDeclKind::Var,
        };
        var_decl.span = import_decl.span;

        Some(var_decl)
    }

    fn import_specifier_to_var_declarator(
        specifier: &ImportSpecifier,
        exposed_esm: &str,
    ) -> VarDeclarator {
        match specifier {
            ImportSpecifier::Named(named) => {
                let var_init_val = match &named.imported {
                    None => MemberExpr {
                        span: DUMMY_SP,
                        obj: quote_ident!(exposed_esm).into(),
                        prop: named.local.clone().into(),
                    },
                    Some(ModuleExportName::Ident(imported_ident)) => MemberExpr {
                        span: imported_ident.span,
                        obj: quote_ident!(exposed_esm).into(),
                        prop: imported_ident.clone().into(),
                    },
                    Some(ModuleExportName::Str(_)) => {
                        unimplemented!(r#"export "str" not supported"#);
                    }
                };

                var!(named = var_init_val)
            }
            ImportSpecifier::Default(default) => {
                var!(
                    default = member_expr!(@EXT, DUMMY_SP,
                            quote_ident!(exposed_esm).into(),
                            default)
                )
            }
            ImportSpecifier::Namespace(namespace) => {
                var!(namespace = quote_ident!(exposed_esm))
            }
        }
    }
}

impl VisitMut for ExternalTransformer<'_> {
    fn visit_mut_expr(&mut self, n: &mut Expr) {
        if let Expr::Call(call_expr) = n
            && is_commonjs_require(call_expr, &self.unresolved_mark)
        {
            if let Some(((namespace, _), _)) = self.require_arg_to_module_namespace(&call_expr.args)
            {
                *n = quote_ident!(namespace.clone()).into();
            }
        }
    }

    fn visit_mut_module(&mut self, n: &mut Module) {
        let contains_external = self.src_to_module.values().any(|module_id| {
            self.concatenate_context
                .external_module_namespace
                .contains_key(module_id)
        });

        if contains_external {
            n.visit_mut_children_with(self);
        }
    }

    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        let mut replaces = vec![];

        for (index, item) in module_items.iter_mut().enumerate() {
            if let ModuleItem::ModuleDecl(module_decl) = item {
                match module_decl {
                    ModuleDecl::Import(import_decl) => {
                        if let Some(((_, esm_namespace), _)) =
                            self.src_to_export_name(import_decl.src.value.as_ref())
                        {
                            if import_decl.specifiers.is_empty() {
                                let empty: Stmt = EmptyStmt {
                                    span: import_decl.span,
                                }
                                .into();
                                *item = empty.into();
                            } else {
                                self.import_decl_to_var_decl(import_decl, &esm_namespace)
                                    .and_then(|var_dcl| {
                                        *item = var_dcl.into();
                                        None::<()>
                                    });
                            }
                        }
                    }
                    ModuleDecl::ExportNamed(named_export) => {
                        if let Some(src) = &named_export.src
                            && let Some(((_, external_module_namespace), src_module_id)) =
                                self.src_to_export_name(src.value.as_ref())
                        {
                            let items = self.named_export_specifier_to_replace_items(
                                &src_module_id,
                                &named_export.specifiers,
                                &external_module_namespace,
                            );

                            replaces.push((index, items));
                        }
                    }
                    ModuleDecl::ExportDecl(_) => {}
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::ExportDefaultDecl(_) => {}
                    ModuleDecl::ExportDefaultExpr(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }
            } else {
                item.visit_mut_children_with(self);
            }
        }

        for (index, items) in replaces {
            module_items.splice(index..index + 1, items);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use maplit::hashmap;
    use swc_core::common::GLOBALS;
    use swc_core::ecma::ast::Id;
    use swc_core::ecma::transforms::base::resolver;
    use swc_core::ecma::utils::collect_decls;

    use super::*;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;

    fn transform_with_external_replace(code: &str) -> String {
        let mut context: Context = Default::default();
        context.config.devtool = None;
        let context: Arc<Context> = Arc::new(context);

        let mut ast = build_js_ast("mut.js", code, &context).unwrap();

        let src_2_module: HashMap<String, ModuleId> = hashmap! {
            "external".to_string() => ModuleId::from("external")
        };
        let current_external_map = hashmap! {
            ModuleId::from("external") => (
                "external_namespace_cjs".to_string(), "external_namespace".to_string()
            )
        };

        let mut concatenate_context = ConcatenateContext {
            external_module_namespace: current_external_map,
            ..Default::default()
        };

        GLOBALS.set(&context.meta.script.globals, || {
            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));

            let mut my_top_vars: HashSet<String> = collect_decls(&ast.ast)
                .iter()
                .map(|id: &Id| id.0.to_string())
                .collect();

            let mut t = ExternalTransformer {
                src_to_module: &src_2_module,
                concatenate_context: &mut concatenate_context,
                context: &context,
                module_id: &ModuleId::from("mut.js"),
                unresolved_mark: ast.unresolved_mark,
                my_top_level_vars: &mut my_top_vars,
            };

            ast.ast.visit_mut_with(&mut t);
            js_ast_to_code(&ast.ast, &context, "sut.js")
                .unwrap()
                .0
                .trim()
                .to_string()
        })
    }

    #[test]
    fn test_external_import_transfer() {
        let code = transform_with_external_replace(r#"import "external";"#);

        assert_eq!(code, ";");
    }

    #[test]
    fn test_external_default_import_transfer() {
        let code = transform_with_external_replace(
            r#"
            import default_import from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
            var default_import = external_namespace.default;
            "#
            .trim()
        );
    }

    #[test]
    fn test_import_namespace_from_external() {
        let code = transform_with_external_replace(
            r#"
            import * as n from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
            var n = external_namespace;
            "#
            .trim()
        );
    }

    #[test]
    fn test_name_import_without_imported() {
        let code = transform_with_external_replace(
            r#"
            import { named } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
            var named = external_namespace.named;
            "#
            .trim()
        );
    }

    #[test]
    fn test_name_import_with_imported() {
        let code = transform_with_external_replace(
            r#"
            import { imported as named } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
            var named = external_namespace.imported;
            "#
            .trim()
        );
    }

    #[test]
    fn test_all_in_one() {
        let code = transform_with_external_replace(
            r#"
            import x, { imported as named, named_2 } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
            var x = external_namespace.default, named = external_namespace.imported, named_2 = external_namespace.named_2;
            "#
                .trim()
        );
    }

    #[test]
    fn test_untouched() {
        let code = transform_with_external_replace(
            r#"
            import { imported as named } from "inner";
            "#,
        );

        assert_eq!(
            code,
            r#"
            import { imported as named } from "inner";
            "#
            .trim()
        );
    }

    #[test]
    fn test_export_named_from_external() {
        let code = transform_with_external_replace(
            r#"
            export { named } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_external_named = external_namespace.named;
export { __mako_external_named as named };
            "#
            .trim()
        );
    }
    #[test]
    fn test_export_named_as_from_external() {
        let code = transform_with_external_replace(
            r#"
            export { named as foo} from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_external_named = external_namespace.named;
export { __mako_external_named as foo };
            "#
            .trim()
        );
    }

    #[test]
    fn test_export_namespace_from_external() {
        let code = transform_with_external_replace(
            r#"
            export * as foo from "external";
            "#,
        );

        assert_eq!(code, r#" export { external_namespace as foo }; "#.trim());
    }

    #[test]
    fn test_export_named_default_from_external() {
        let code = transform_with_external_replace(
            r#"
            export { default } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_mut_js_0 = external_namespace.default;
export { __mako_mut_js_0 as default };
         "#
            .trim()
        );
    }

    #[test]
    fn test_export_named_as_default_from_external() {
        let code = transform_with_external_replace(
            r#"
            export { foo as default } from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_mut_js_0 = external_namespace.foo;
export { __mako_mut_js_0 as default };
         "#
            .trim()
        );
    }

    #[test]
    fn test_require_from_external() {
        let code = transform_with_external_replace(r#"let e = require("external");"#);

        assert_eq!(code, r#"let e = external_namespace_cjs;"#.trim());
    }

    #[ignore]
    #[test]
    fn test_export_default_with_name_from_external() {
        let code = transform_with_external_replace(
            r#"
            export x from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_external_x = external_namespace.default;
export { __mako_external_x as x };
         "#
            .trim(),
        );
    }

    #[ignore]
    #[test]
    fn test_export_default_as_default_from_external() {
        let code = transform_with_external_replace(
            r#"
            export default from "external";
            "#,
        );

        assert_eq!(
            code,
            r#"
var __mako_mut_js_0 = external_namespace.default;
export { __mako_mut_js_0 as default };
         "#
            .trim(),
        );
    }
}
