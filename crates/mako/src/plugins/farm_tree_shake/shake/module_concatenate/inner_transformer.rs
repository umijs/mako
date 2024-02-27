use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::swc_common::util::take::Take;
use swc_core::common::comments::{Comment, CommentKind};
use swc_core::common::{Mark, Spanned, SyntaxContext, DUMMY_SP};
use swc_core::ecma::ast::{
    DefaultDecl, ExportSpecifier, Id, ImportSpecifier, KeyValueProp, Module, ModuleDecl,
    ModuleExportName, ModuleItem, ObjectLit, Prop, PropOrSpread, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{
    collect_decls_with_ctxt, member_expr, quote_ident, ExprFactory, IdentRenamer,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use super::exports_transform::collect_exports_map;
use super::utils::{
    declare_var_with_init_stmt, uniq_module_prefix, MODULE_CONCATENATE_ERROR,
    MODULE_CONCATENATE_ERROR_STR_MODULE_NAME,
};
use crate::compiler::Context;
use crate::module::{relative_to_root, ImportType, ModuleId};

pub(super) struct InnerTransform<'a> {
    pub context: &'a Arc<Context>,
    pub module_id: &'a ModuleId,
    pub modules_in_scope: &'a mut HashMap<ModuleId, HashMap<String, String>>,
    pub src_to_module: &'a HashMap<String, ModuleId>,
    pub current_top_level_vars: &'a mut HashSet<String>,
    pub top_level_mark: Mark,

    my_top_decls: HashSet<String>,
    exports: HashMap<String, String>,
    rename_request: Vec<(Id, Id)>,
    imported_type: ImportType,
}

impl<'a> InnerTransform<'a> {
    pub fn new<'s>(
        modules_in_scope: &'a mut HashMap<ModuleId, HashMap<String, String>>,
        top_level_var: &'a mut HashSet<String>,
        module_id: &'a ModuleId,
        src_to_module: &'a HashMap<String, ModuleId>,
        context: &'a Arc<Context>,
        top_level_mark: Mark,
    ) -> InnerTransform<'a>
    where
        'a: 's,
    {
        Self {
            modules_in_scope,
            current_top_level_vars: top_level_var,
            module_id,
            src_to_module,
            context,
            top_level_mark,
            exports: Default::default(),
            my_top_decls: Default::default(),
            rename_request: vec![],
            imported_type: Default::default(),
        }
    }

    pub fn imported(&mut self, imported_type: ImportType) {
        self.imported_type = imported_type;
    }

    fn collect_exports(&mut self, n: &Module) {
        let mut export_map = collect_exports_map(n);
        if export_map.get(&"default".to_string()).is_some() {
            let default_name = uniq_module_default_export_name(self.module_id, self.context);

            export_map.insert("default".to_string(), default_name.clone());
            self.my_top_decls.insert(default_name);
        }

        self.exports.extend(export_map);
    }

    fn add_leading_comment(&self, n: &Module) {
        if let Some(item) = n.body.first() {
            let mut comments = self.context.meta.script.origin_comments.write().unwrap();

            comments.add_leading_comment_at(
                item.span_lo(),
                Comment {
                    kind: CommentKind::Line,
                    span: DUMMY_SP,
                    text: format!(
                        " CONCATENATED MODULE: {}",
                        relative_to_root(&self.module_id.id, &self.context.root)
                    )
                    .into(),
                },
            );
        }
    }

    fn apply_renames(&mut self, n: &mut Module) {
        let map = self.rename_request.iter().cloned().collect();
        let mut renamer = IdentRenamer::new(&map);
        n.visit_mut_with(&mut renamer);

        // todo performance?
        for (from, to) in &map {
            self.exports.iter_mut().for_each(|(_k, v)| {
                if from.0.eq(v) {
                    *v = to.0.to_string();
                }
            });
        }
    }

    fn get_non_conflict_name(&self, name: &String) -> String {
        let mut new_name = name.to_string();
        let mut i = 1;
        while self.current_top_level_vars.contains(&new_name)
            || self.my_top_decls.contains(&new_name)
        {
            new_name = format!("{}_{}", name, i);
            i += 1;
        }
        new_name
    }

    fn resolve_conflict(&mut self) {
        let conflicts: Vec<_> = self
            .current_top_level_vars
            .intersection(&self.my_top_decls)
            .cloned()
            .collect();

        let ctxt = SyntaxContext::empty().apply_mark(self.top_level_mark);
        let default_name = uniq_module_default_export_name(self.module_id, self.context);
        for conflicted_name in conflicts {
            self.my_top_decls.remove(&conflicted_name);

            let new_name = self.get_non_conflict_name(&conflicted_name);

            if conflicted_name == default_name {
                self.exports.insert("default".to_string(), new_name.clone());
            }

            self.exports
                .entry(conflicted_name.clone())
                .and_modify(|e| *e = new_name.clone());
            self.my_top_decls.insert(new_name.clone());
            self.rename_request
                .push(((conflicted_name.into(), ctxt), (new_name.into(), ctxt)));
        }
    }
}

impl<'a> VisitMut for InnerTransform<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        self.my_top_decls =
            collect_decls_with_ctxt(n, SyntaxContext::empty().apply_mark(self.top_level_mark))
                .iter()
                .map(|id: &Id| id.0.to_string())
                .collect();
        self.collect_exports(n);

        n.visit_mut_children_with(self);

        self.resolve_conflict();
        self.apply_renames(n);
        self.add_leading_comment(n);

        if (self.imported_type & ImportType::Namespace) == ImportType::Namespace {
            let ns_name = self
                .get_non_conflict_name(&uniq_module_namespace_name(self.module_id, self.context));
            let ns_ident = quote_ident!(ns_name.clone());

            let empty_obj = ObjectLit {
                span: DUMMY_SP,
                props: vec![],
            };

            let init_stmt: Stmt = empty_obj
                .into_var_decl(VarDeclKind::Var, ns_ident.clone().into())
                .into();

            let mut key_value_props: Vec<PropOrSpread> = vec![];

            for (exported_name, local_name) in self.exports.iter() {
                key_value_props.push(
                    Prop::KeyValue(KeyValueProp {
                        key: quote_ident!(exported_name.clone()).into(),
                        value: quote_ident!(local_name.clone()).into_lazy_fn(vec![]).into(),
                    })
                    .into(),
                )
            }

            let define_exports: Stmt = member_expr!(DUMMY_SP, __mako_require__.e)
                .as_call(
                    DUMMY_SP,
                    vec![
                        ns_ident.as_arg(),
                        ObjectLit {
                            span: DUMMY_SP,
                            props: key_value_props,
                        }
                        .as_arg(),
                    ],
                )
                .into_stmt();

            self.exports.insert("*".to_string(), ns_name.clone());
            self.my_top_decls.insert(ns_name);

            n.body.push(init_stmt.into());
            n.body.push(define_exports.into());
        }

        self.modules_in_scope
            .insert(self.module_id.clone(), self.exports.clone());
        self.current_top_level_vars
            .extend(self.my_top_decls.iter().cloned());
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        let mut replaces = vec![];

        for index in (0..items.len()).rev() {
            let mut stmts: Option<Vec<ModuleItem>> = None;

            let item = items.get_mut(index).unwrap();

            if let Some(module_decl) = item.as_mut_module_decl() {
                match module_decl {
                    ModuleDecl::Import(import) => {
                        let src = import.src.value.to_string();
                        stmts = Some(vec![]);

                        if let Some(src_module_id) = self.src_to_module.get(&src)
                            && let Some(exports_map) = self.modules_in_scope.get(src_module_id)
                        {
                            for import_specifier in import.specifiers.iter() {
                                match &import_specifier {
                                    ImportSpecifier::Named(named_import) => {
                                        let imported_name = match &named_import.imported {
                                            None => named_import.local.sym.to_string(),
                                            Some(ModuleExportName::Ident(id)) => id.sym.to_string(),
                                            Some(ModuleExportName::Str(_)) => {
                                                unimplemented!("")
                                            }
                                        };

                                        let local = named_import.local.sym.to_string();

                                        if let Some(mapped_export) = exports_map.get(&imported_name)
                                        {
                                            if local != *mapped_export {
                                                let stmt: Stmt = declare_var_with_init_stmt(
                                                    named_import.local.clone(),
                                                    mapped_export,
                                                );

                                                stmts.as_mut().unwrap().push(stmt.into());
                                            } else {
                                                self.my_top_decls.remove(&local);
                                            }
                                        }
                                    }
                                    ImportSpecifier::Default(default_import) => {
                                        if let Some(default_export_name) =
                                            exports_map.get("default")
                                        {
                                            let stmt: Stmt =
                                                quote_ident!(default_export_name.clone())
                                                    .into_var_decl(
                                                        VarDeclKind::Var,
                                                        default_import.local.clone().into(),
                                                    )
                                                    .into();

                                            stmts.as_mut().unwrap().push(stmt.into());
                                        }
                                    }
                                    ImportSpecifier::Namespace(namespace) => {
                                        let exported_namespace = exports_map.get("*").unwrap();
                                        let stmt: Stmt = quote_ident!(exported_namespace.clone())
                                            .into_var_decl(
                                                VarDeclKind::Var,
                                                namespace.local.clone().into(),
                                            )
                                            .into();
                                        stmts.as_mut().unwrap().push(stmt.into());
                                    }
                                }
                            }
                        }
                    }
                    ModuleDecl::ExportDecl(export_decl) => {
                        let decl = export_decl.decl.take();

                        let stmt: Stmt = decl.into();

                        *item = stmt.into();
                    }
                    ModuleDecl::ExportNamed(named_export) => {
                        if let Some(export_src) = &named_export.src {
                            if let Some(imported_module_id) =
                                self.src_to_module.get(&export_src.value.to_string())
                                && let Some(export_map) =
                                    self.modules_in_scope.get(imported_module_id)
                            {
                                stmts = Some(vec![]);

                                for spec in &named_export.specifiers {
                                    match spec {
                                        ExportSpecifier::Namespace(ns) => {
                                            let exported_namespace = export_map.get("*").unwrap();

                                            match &ns.name {
                                                ModuleExportName::Ident(name_ident) => {
                                                    let stmt: Stmt = declare_var_with_init_stmt(
                                                        name_ident.clone(),
                                                        exported_namespace,
                                                    );
                                                    stmts.as_mut().unwrap().push(stmt.into());
                                                }
                                                ModuleExportName::Str(_) => {
                                                    unimplemented!(
                                                        "{}",
                                                        MODULE_CONCATENATE_ERROR_STR_MODULE_NAME
                                                    );
                                                }
                                            }
                                        }
                                        ExportSpecifier::Default(default_reexport) => {
                                            let default_export_name =
                                                export_map.get("default").unwrap();

                                            let stmt: Stmt = declare_var_with_init_stmt(
                                                default_reexport.exported.clone(),
                                                default_export_name,
                                            );

                                            stmts.as_mut().unwrap().push(stmt.into());
                                        }
                                        ExportSpecifier::Named(named) => {
                                            let (exported_ident, orig_name) =
                                                match (&named.exported, &named.orig) {
                                                    (None, ModuleExportName::Ident(orig)) => {
                                                        (orig.clone(), orig.sym.to_string())
                                                    }
                                                    (
                                                        Some(ModuleExportName::Ident(
                                                            exported_ident,
                                                        )),
                                                        ModuleExportName::Ident(orig_ident),
                                                    ) => (
                                                        exported_ident.clone(),
                                                        orig_ident.sym.to_string(),
                                                    ),
                                                    (_, _) => {
                                                        unimplemented!(
                                                            "{}",
                                                            MODULE_CONCATENATE_ERROR_STR_MODULE_NAME
                                                        )
                                                    }
                                                };

                                            if let Some(mapped_export) = export_map.get(&orig_name)
                                            {
                                                stmts.as_mut().unwrap().push(
                                                    declare_var_with_init_stmt(
                                                        exported_ident,
                                                        mapped_export,
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                unreachable!("{}", MODULE_CONCATENATE_ERROR);
                            }
                        } else {
                            let dcl_stmts: Vec<ModuleItem> = vec![];

                            for export_spec in &named_export.specifiers {
                                match export_spec {
                                    ExportSpecifier::Namespace(_) => {
                                        unreachable!("namespace export unreachable when no src")
                                    }
                                    ExportSpecifier::Default(_) => {
                                        unreachable!("default export unreachable when no src")
                                    }
                                    ExportSpecifier::Named(named) => {
                                        match (&named.exported, &named.orig) {
                                            (
                                                Some(ModuleExportName::Ident(exported_ident)),
                                                ModuleExportName::Ident(orig_ident),
                                            ) => {
                                                if !exported_ident.sym.eq("default") {
                                                    self.exports.insert(
                                                        exported_ident.sym.to_string(),
                                                        orig_ident.sym.to_string(),
                                                    );
                                                }
                                            }
                                            (None, ModuleExportName::Ident(_)) => {
                                                // do nothing, it will be removed
                                            }
                                            (_, ModuleExportName::Str(_))
                                            | (Some(ModuleExportName::Str(_)), _) => {
                                                unimplemented!("export 'str' not supported now");
                                            }
                                        }
                                    }
                                }
                            }

                            stmts = Some(dcl_stmts);
                        }
                    }
                    ModuleDecl::ExportDefaultDecl(export_default_dcl) => {
                        match &mut export_default_dcl.decl {
                            DefaultDecl::Class(dcl) => {
                                let stmt: Stmt = dcl
                                    .take()
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        quote_ident!(uniq_module_default_export_name(
                                            self.module_id,
                                            self.context
                                        ))
                                        .into(),
                                    )
                                    .into();

                                items[index] = stmt.into();
                            }
                            DefaultDecl::Fn(dcl) => {
                                let stmt: Stmt = dcl
                                    .take()
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        quote_ident!(uniq_module_default_export_name(
                                            self.module_id,
                                            self.context
                                        ))
                                        .into(),
                                    )
                                    .into();

                                items[index] = stmt.into();
                            }
                            DefaultDecl::TsInterfaceDecl(_) => {
                                unreachable!("TS should already be stripped")
                            }
                        }
                    }
                    ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                        let span = export_default_expr.span.apply_mark(self.top_level_mark);

                        let stmt: Stmt = export_default_expr
                            .expr
                            .take()
                            .into_var_decl(
                                VarDeclKind::Const,
                                quote_ident!(
                                    span,
                                    uniq_module_default_export_name(self.module_id, self.context)
                                )
                                .into(),
                            )
                            .into();

                        *item = stmt.into();
                    }
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }
            }

            if let Some(to_replace) = stmts {
                replaces.push((index, to_replace));
            }
        }

        for (index, rep) in replaces {
            items.splice(index..index + 1, rep);
        }
    }
}

fn uniq_module_default_export_name(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!("{}_0", uniq_module_prefix(module_id, context))
}

fn uniq_module_namespace_name(module_id: &ModuleId, context: &Arc<Context>) -> String {
    format!("{}_ns", uniq_module_prefix(module_id, context))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    use maplit::{hashmap, hashset};
    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::resolver;
    use swc_core::ecma::visit::VisitMutWith;

    use super::InnerTransform;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::compiler::Context;
    use crate::config::{Config, Mode, OptimizationConfig};
    use crate::module::ModuleId;
    use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::ConcatenateContext;

    #[test]
    fn test_export_as() {
        let mut ccn_ctx = ConcatenateContext::default();

        let code = inner_trans_code("var n = some.named;export { n as named };", &mut ccn_ctx);

        assert_eq!(code, "var n = some.named;");
        assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
        assert_eq!(
            current_export_map(&ccn_ctx),
            &hashmap!("named".to_string() => "n".to_string())
        );
    }

    #[test]
    fn test_export_as_with_conflict() {
        let mut ccn_ctx = ConcatenateContext {
            top_level_vars: hashset!("n".to_string()),
            ..Default::default()
        };

        let code = inner_trans_code("var n = some.named;export { n as named };", &mut ccn_ctx);

        assert_eq!(code, "var n_1 = some.named;");
        assert_eq!(
            ccn_ctx.top_level_vars,
            hashset!("n".to_string(), "n_1".to_string())
        );
        assert_eq!(
            current_export_map(&ccn_ctx),
            &hashmap!("named".to_string() => "n_1".to_string())
        );
    }

    #[test]
    fn test_export_as_twice_with_conflict() {
        let mut ccn_ctx = ConcatenateContext {
            top_level_vars: hashset!("n".to_string()),
            ..Default::default()
        };

        let code = inner_trans_code(
            "var n = some.named;export { n as named, n as foo };",
            &mut ccn_ctx,
        );

        assert_eq!(code, "var n_1 = some.named;");
        assert_eq!(
            ccn_ctx.top_level_vars,
            hashset!("n".to_string(), "n_1".to_string())
        );
        assert_eq!(
            current_export_map(&ccn_ctx),
            &hashmap!(
                "named".to_string() => "n_1".to_string(),
                "foo".to_string() => "n_1".to_string()
            )
        );
    }

    #[test]
    fn test_short_named_export() {
        let mut ccn_ctx = ConcatenateContext::default();

        let code = inner_trans_code("var named = some.named;export { named };", &mut ccn_ctx);

        assert_eq!(code, "var named = some.named;");
        assert_eq!(ccn_ctx.top_level_vars, hashset!("named".to_string()));
        assert_eq!(
            current_export_map(&ccn_ctx),
            &hashmap!(
                "named".to_string() => "named".to_string()
            )
        );
    }

    #[test]
    fn test_short_named_export_with_conflict() {
        let mut ccn_ctx = ConcatenateContext {
            top_level_vars: hashset!("named".to_string()),
            ..Default::default()
        };

        let code = inner_trans_code("var named = some.named;export { named };", &mut ccn_ctx);

        assert_eq!(code, "var named_1 = some.named;");
        assert_eq!(
            ccn_ctx.top_level_vars,
            hashset!("named".to_string(), "named_1".to_string())
        );
        assert_eq!(
            current_export_map(&ccn_ctx),
            &hashmap!(
                "named".to_string() => "named_1".to_string()
            )
        );
    }

    fn inner_trans_code(code: &str, concatenate_context: &mut ConcatenateContext) -> String {
        let context = Arc::new(Context {
            config: Config {
                devtool: None,
                optimization: Some(OptimizationConfig {
                    concatenate_modules: Some(true),
                    skip_modules: Some(true),
                }),
                mode: Mode::Production,
                minify: false,
                ..Default::default()
            },
            ..Default::default()
        });

        let mut ast = build_js_ast("mut.js", code, &context).unwrap();
        let module_id = ModuleId::from("mut.js");

        let src_to_module = Default::default();

        GLOBALS.set(&context.meta.script.globals, || {
            let mut inner = InnerTransform::new(
                &mut concatenate_context.modules_in_scope,
                &mut concatenate_context.top_level_vars,
                &module_id,
                &src_to_module,
                &context,
                ast.top_level_mark,
            );

            ast.ast.visit_mut_with(&mut resolver(
                ast.unresolved_mark,
                ast.top_level_mark,
                false,
            ));
            ast.ast.visit_mut_with(&mut inner);

            {
                // do not need comments
                let mut comment = context.meta.script.origin_comments.write().unwrap();
                *comment = Default::default();
            }

            let (code, _) = js_ast_to_code(&ast.ast, &context, "mut.js").unwrap();
            code.trim().to_string()
        })
    }

    fn current_export_map(ccn_ctx: &ConcatenateContext) -> &HashMap<String, String> {
        ccn_ctx
            .modules_in_scope
            .get(&ModuleId::from("mut.js"))
            .unwrap()
    }
}
