mod root_transformer;
mod utils;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;

use heck::ToSnakeCase;
use mako_core::swc_common::util::take::Take;
use root_transformer::RootTransformer;
use swc_core::common::collections::AHashSet;
use swc_core::common::{Span, SyntaxContext, GLOBALS};
use swc_core::ecma::ast::{
    DefaultDecl, Expr, Id, Module, ModuleDecl, ModuleItem, Stmt, VarDeclKind,
};
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::utils::{collect_decls_with_ctxt, quote_ident, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};
use utils::uniq_module_prefix;

use crate::compiler::Context;
use crate::module::ModuleId;
use crate::module_graph::ModuleGraph;
use crate::plugins::farm_tree_shake::module::{AllExports, TreeShakeModule};
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

    for module_id in sorted_module_ids.iter() {
        // TODO async module should not be candidate

        // 环上的节点先不碰
        if all_in_circles.contains(module_id) {
            continue;
        }

        let can_be_root = true;
        let mut can_be_inner = true;

        if let Some(tsm) = tree_shake_modules_map.get(module_id) {
            let tsm = tsm.borrow();

            if tsm.module_system != ModuleSystem::ESModule {
                continue;
            }

            // 必须要有清晰的导出
            // ? 是不是不能有 export * from 'foo' 的语法
            if matches!(tsm.all_exports, AllExports::Ambiguous(_)) {
                can_be_inner = false;
            }
        }

        if can_be_root {
            root_candidates.push(module_id.clone());
        }

        if can_be_inner {
            inner_candidates.insert(module_id.clone());
        }
    }

    let mut concat_configurations = vec![];
    let mut used_as_inner = HashSet::new();

    for root in root_candidates.iter() {
        let mut config = ConcatenateConfig::new(root.clone());

        let deps = module_graph.get_dependencies(root);

        for (dep_module_id, _) in deps {
            if inner_candidates.contains(dep_module_id) {
                config.add_inner(dep_module_id.clone());
                used_as_inner.insert(dep_module_id.clone());
            }
        }

        used_as_inner.insert(config.root.clone());

        concat_configurations.push(config);
    }

    let mut module_items: Vec<ModuleItem> = Vec::new();

    let config = concat_configurations.first().unwrap();

    for id in config.inners.iter() {
        let module = module_graph.get_module_mut(id).unwrap();

        let _module_var_prefix = module.id.generate(context).to_snake_case();

        let module_info = module.info.as_mut().unwrap();

        let script_ast = module_info.ast.script_mut().unwrap();

        GLOBALS.set(&context.meta.script.globals, || {
            let top_ctx = SyntaxContext::empty().apply_mark(script_ast.top_level_mark);

            let mut ter = InnerTransform {
                module_id: id,
                top_level_ctx: top_ctx,
                uniq_prefix: uniq_module_prefix(id, context),
            };

            script_ast.ast.visit_mut_with(&mut ter);
            script_ast.ast.visit_mut_with(&mut CleanSyntaxContext {})
        });

        module_items.append(&mut script_ast.ast.body.clone());
    }

    let root_module = module_graph.get_module_mut(&config.root).unwrap();

    let ast = &mut root_module.info.as_mut().unwrap().ast;

    let ast_script = ast.script_mut().unwrap();

    let mut root_module_ast = ast_script.ast.take();
    let unresolved_mark = ast_script.unresolved_mark;
    let top_level_mark = ast_script.top_level_mark;
    GLOBALS.set(&context.meta.script.globals, || {
        root_module_ast.visit_mut_with(&mut RootTransformer {
            module_graph,
            current_module_id: &config.root,
            context,
        });

        root_module_ast.body.splice(0..0, module_items);
        root_module_ast.visit_mut_with(&mut CleanSyntaxContext {});
        root_module_ast.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, false));
    });

    let root_module = module_graph.get_module_mut(&config.root).unwrap();
    let ast = &mut root_module.info.as_mut().unwrap().ast;
    let ast_script = ast.script_mut().unwrap();
    ast_script.ast = root_module_ast;

    for inner in config.inners.iter() {
        module_graph.remove_module(inner);
    }

    Ok(())
}

struct ConcatenateConfig {
    root: ModuleId,
    inners: Vec<ModuleId>,
}

pub struct CleanSyntaxContext;

impl ConcatenateConfig {
    fn new(root: ModuleId) -> Self {
        Self {
            root,
            inners: vec![],
        }
    }

    fn add_inner(&mut self, inner: ModuleId) {
        self.inners.push(inner);
    }
}

impl VisitMut for CleanSyntaxContext {
    fn visit_mut_span(&mut self, n: &mut Span) {
        n.ctxt = SyntaxContext::empty();
    }
}

struct InnerTransform<'a> {
    module_id: &'a ModuleId,
    top_level_ctx: SyntaxContext,
    uniq_prefix: String,
}

impl<'a> VisitMut for InnerTransform<'a> {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let top_ctx = self.top_level_ctx;
        let _tops: AHashSet<Id> = collect_decls_with_ctxt(n, top_ctx);

        n.visit_mut_children_with(self);
    }
    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        for index in (00..items.len()).rev() {
            let item = items.get_mut(index).unwrap();

            if let Some(module_decl) = item.as_mut_module_decl() {
                match module_decl {
                    ModuleDecl::Import(_) => {}
                    ModuleDecl::ExportDecl(export_decl) => {
                        let decl = export_decl.decl.take();

                        let stmt: Stmt = decl.into();

                        *item = stmt.into();
                    }
                    ModuleDecl::ExportNamed(_) => {}
                    ModuleDecl::ExportDefaultDecl(export_default_dcl) => {
                        match &mut export_default_dcl.decl {
                            DefaultDecl::Class(dcl) => {
                                items[index] = dcl.take().into_stmt().into();
                            }
                            DefaultDecl::Fn(dcl) => {
                                items[index] = dcl.take().into_stmt().into();
                            }
                            DefaultDecl::TsInterfaceDecl(_) => {}
                        }
                    }
                    ModuleDecl::ExportDefaultExpr(export_default_expr) => {
                        println!("ModuleDecl::ExportDefaultExpr :{:?}", export_default_expr);

                        match export_default_expr.expr.deref() {
                            Expr::This(_) => {}
                            Expr::Array(_) => {}
                            Expr::Object(_) => {}
                            Expr::Fn(_) => {}
                            Expr::Unary(_) => {}
                            Expr::Update(_) => {}
                            Expr::Bin(_) => {}
                            Expr::Assign(_) => {}
                            Expr::Member(_) => {}
                            Expr::SuperProp(_) => {}
                            Expr::Cond(_) => {}
                            Expr::Call(_) => {}
                            Expr::New(_) => {}
                            Expr::Seq(_) => {}
                            Expr::Ident(_) => {}
                            Expr::Lit(_) => {
                                let expr = export_default_expr.expr.take();

                                let stmt: Stmt = expr
                                    .to_owned()
                                    .into_var_decl(
                                        VarDeclKind::Const,
                                        quote_ident!(format!("{}_0", self.uniq_prefix)).into(),
                                    )
                                    .into();

                                *item = stmt.into();
                            }
                            Expr::Tpl(_) => {}
                            Expr::TaggedTpl(_) => {}
                            Expr::Arrow(_) => {}
                            Expr::Class(_) => {}
                            Expr::Yield(_) => {}
                            Expr::MetaProp(_) => {}
                            Expr::Await(_) => {}
                            Expr::Paren(_) => {}
                            Expr::JSXMember(_) => {}
                            Expr::JSXNamespacedName(_) => {}
                            Expr::JSXEmpty(_) => {}
                            Expr::JSXElement(_) => {}
                            Expr::JSXFragment(_) => {}
                            Expr::TsTypeAssertion(_) => {}
                            Expr::TsConstAssertion(_) => {}
                            Expr::TsNonNull(_) => {}
                            Expr::TsAs(_) => {}
                            Expr::TsInstantiation(_) => {}
                            Expr::TsSatisfies(_) => {}
                            Expr::PrivateName(_) => {}
                            Expr::OptChain(_) => {}
                            Expr::Invalid(_) => {}
                        }
                    }
                    ModuleDecl::ExportAll(_) => {}
                    ModuleDecl::TsImportEquals(_) => {}
                    ModuleDecl::TsExportAssignment(_) => {}
                    ModuleDecl::TsNamespaceExport(_) => {}
                }
            }
        }
    }
}
