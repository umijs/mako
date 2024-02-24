use std::collections::HashMap;
use std::sync::Arc;

use swc_core::common::{Spanned, DUMMY_SP};
use swc_core::ecma::ast::{
    EmptyStmt, ExportNamedSpecifier, ExportSpecifier, ImportSpecifier, MemberExpr, Module,
    ModuleDecl, ModuleExportName, ModuleItem, NamedExport, Stmt, VarDecl, VarDeclKind,
};
use swc_core::ecma::utils::{member_expr, quote_ident, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::compiler::Context;
use crate::module::ModuleId;
use crate::plugins::farm_tree_shake::shake::module_concatenate::utils::uniq_module_prefix;

pub(super) struct ExternalTransformer<'a> {
    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub current_external_map: &'a HashMap<ModuleId, String>,
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,
}

impl<'a> ExternalTransformer<'_> {
    fn src_to_export_name(&'a self, src: &str) -> Option<&'a String> {
        self.src_to_module
            .get(src)
            .and_then(|module_id| self.current_external_map.get(module_id))
    }

    fn named_export_specifier_to_repalce_items(
        &self,
        specifiers: &Vec<ExportSpecifier>,
        name: &String,
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
                                orig: ModuleExportName::Ident(quote_ident!(name.clone())),
                                exported: Some(ModuleExportName::Ident(local.clone())),
                                is_type_only: false,
                            };
                            var_decorators.push(spec.into());
                        }
                        ModuleExportName::Str(_) => {}
                    };
                }
                ExportSpecifier::Default(default_spec) => {
                    let default_export_var_decl = MemberExpr {
                        span: DUMMY_SP,
                        obj: quote_ident!(name.clone()).into(),
                        prop: quote_ident!("default").into(),
                    }
                    .into_var_decl(VarDeclKind::Var, default_spec.exported.clone().into());

                    stmts.push(default_export_var_decl.into());

                    let spec = ExportNamedSpecifier {
                        span: DUMMY_SP,
                        orig: ModuleExportName::Ident(quote_ident!(name.clone())),
                        exported: Some(ModuleExportName::Ident(quote_ident!("default"))),
                        is_type_only: false,
                    };
                    var_decorators.push(spec.into());
                }
                ExportSpecifier::Named(named) => match (&named.orig, &named.exported) {
                    (ModuleExportName::Ident(orig), Some(ModuleExportName::Ident(exported))) => {
                        let local_proxy_name = if exported.sym.eq("default") {
                            quote_ident!(uniq_module_default_export_name(
                                self.module_id,
                                self.context
                            ))
                        } else {
                            exported.clone()
                        };

                        let export_name_decl = MemberExpr {
                            span: DUMMY_SP,
                            obj: quote_ident!(name.clone()).into(),
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
                            quote_ident!(uniq_module_default_export_name(
                                self.module_id,
                                self.context
                            ))
                        } else {
                            orig.clone()
                        };

                        let var_decl = MemberExpr {
                            span: DUMMY_SP,
                            obj: quote_ident!(name.clone()).into(),
                            prop: orig.clone().into(),
                        }
                        .into_var_decl(VarDeclKind::Var, local_proxy_name.clone().into());
                        stmts.push(var_decl.into());

                        var_decorators.push(
                            ExportNamedSpecifier {
                                span: DUMMY_SP,
                                orig: local_proxy_name.clone().into(),
                                exported: Some(ModuleExportName::Ident(orig.clone())),
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
}

impl VisitMut for ExternalTransformer<'_> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let contains_external = self
            .src_to_module
            .values()
            .any(|module_id| self.current_external_map.contains_key(module_id));

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
                        if let Some(imported_module_id) =
                            self.src_to_module.get(&import_decl.src.value.to_string())
                            && let Some(external_module_namespace) =
                                self.current_external_map.get(imported_module_id)
                        {
                            if import_decl.specifiers.is_empty() {
                                let empty_stmt: Stmt = EmptyStmt { span: DUMMY_SP }.into();

                                *item = empty_stmt.into();
                            } else {
                                let mut var_decorators = vec![];

                                for specifier in &import_decl.specifiers {
                                    match specifier {
                                        ImportSpecifier::Named(named) => {
                                            if let Some(imported) = &named.imported {
                                                match imported {
                                                    ModuleExportName::Ident(imported_ident) => {
                                                        let x = MemberExpr {
                                                            span: DUMMY_SP,
                                                            obj: quote_ident!(
                                                                external_module_namespace.clone()
                                                            )
                                                            .into(),
                                                            prop: imported_ident.clone().into(),
                                                        }
                                                        .into_var_decl(
                                                            VarDeclKind::Var,
                                                            named.local.clone().into(),
                                                        );

                                                        var_decorators.extend(x.decls);
                                                    }
                                                    ModuleExportName::Str(_) => {
                                                        unimplemented!(
                                                            r#"export "str" not supported"#
                                                        )
                                                    }
                                                }
                                            } else {
                                                let x = MemberExpr {
                                                    span: DUMMY_SP,
                                                    obj: quote_ident!(
                                                        external_module_namespace.clone()
                                                    )
                                                    .into(),
                                                    prop: named.local.clone().into(),
                                                }
                                                .into_var_decl(
                                                    VarDeclKind::Var,
                                                    named.local.clone().into(),
                                                );

                                                var_decorators.extend(x.decls);
                                            }
                                        }
                                        ImportSpecifier::Default(default) => {
                                            let x = member_expr!(@EXT, DUMMY_SP,
                                            quote_ident!(default.span, external_module_namespace
                                                .clone()).into()
                                            , default)
                                            .into_var_decl(
                                                VarDeclKind::Var,
                                                default.local.clone().into(),
                                            );

                                            var_decorators.extend(x.decls);
                                        }
                                        ImportSpecifier::Namespace(namespace) => {
                                            let var_dec =
                                                quote_ident!(external_module_namespace.clone())
                                                    .into_var_decl(
                                                        VarDeclKind::Var,
                                                        namespace.local.clone().into(),
                                                    );

                                            var_decorators.extend(var_dec.decls);
                                        }
                                    }
                                }

                                *item = VarDecl {
                                    span: item.span(),
                                    decls: var_decorators,
                                    declare: false,
                                    kind: VarDeclKind::Var,
                                }
                                .into();
                            }
                        }
                    }

                    ModuleDecl::ExportDecl(_) => {}
                    ModuleDecl::ExportNamed(named_export) => {
                        if let Some(src) = &named_export.src
                            && let Some(external_module_namespace) =
                                self.src_to_export_name(src.value.as_ref())
                        {
                            let items = self.named_export_specifier_to_repalce_items(
                                &named_export.specifiers,
                                external_module_namespace,
                            );

                            replaces.push((index, items));
                        }
                    }
                    ModuleDecl::ExportDefaultDecl(_) => {}
                    ModuleDecl::ExportDefaultExpr(_) => {}
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }
            }
        }

        for (index, items) in replaces {
            module_items.splice(index..index + 1, items);
        }
    }
}

fn uniq_module_default_export_name(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!("{}_0", uniq_module_prefix(module_id, context))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use maplit::hashmap;

    use super::*;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;

    fn transform_with_external_replace(code: &str) -> String {
        let mut context: Context = Default::default();
        context.config.devtool = None;
        let context: Arc<Context> = Arc::new(context);

        let mut ast = build_js_ast("sut.js", code, &context).unwrap();

        let src_2_module: HashMap<String, ModuleId> = hashmap! {
            "external".to_string() => ModuleId::from("external")
        };
        let current_external_map = hashmap! {
            ModuleId::from("external") => "external_namespace".to_string()
        };

        let mut t = ExternalTransformer {
            src_to_module: &src_2_module,
            current_external_map: &current_external_map,
            context: &context,
            module_id: &ModuleId::from("sut.js"),
        };

        ast.ast.visit_mut_with(&mut t);
        js_ast_to_code(&ast.ast, &context, "sut.js")
            .unwrap()
            .0
            .trim()
            .to_string()
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
}
