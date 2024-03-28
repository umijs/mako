use std::collections::HashMap;
use std::sync::Arc;

use mako_core::swc_common::util::take::Take;
use mako_core::swc_common::{Mark, DUMMY_SP};
use mako_core::swc_ecma_ast::{
    ArrayLit, AssignExpr, AssignOp, AwaitExpr, BlockStmt, BlockStmtOrExpr, CondExpr, Expr, Ident,
    Lit, ModuleItem, Stmt, VarDeclKind,
};
use mako_core::swc_ecma_visit::VisitMut;
use swc_core::ecma::ast::ParenExpr;
use swc_core::ecma::utils::{member_expr, quote_expr, quote_ident, ExprFactory};
use swc_core::ecma::visit::VisitMutWith;

use crate::ast_2::utils::is_commonjs_require;
use crate::compiler::Context;
use crate::module::{Dependency, ModuleId};

const ASYNC_IMPORTED_MODULE: &str = "_async__mako_imported_module_";

pub struct AsyncModule<'a> {
    async_deps: &'a Vec<Dependency>,
    async_deps_idents: Vec<Ident>,
    found: bool,
    first_async_dep_import_pos: usize,
    prepend_module_items: Vec<ModuleItem>,
    top_level_await: bool,
    unresolved_mark: Mark,
}

impl<'a> AsyncModule<'a> {
    pub fn new(
        async_deps: &'a Vec<Dependency>,
        unresolved_mark: Mark,
        top_level_await: bool,
    ) -> Self {
        Self {
            async_deps,
            async_deps_idents: vec![],
            found: false,
            first_async_dep_import_pos: 0,
            prepend_module_items: vec![],
            top_level_await,
            unresolved_mark,
        }
    }
}

impl VisitMut for AsyncModule<'_> {
    fn visit_mut_expr(&mut self, n: &mut Expr) {
        if let Expr::Call(call_expr) = n
            && is_commonjs_require(call_expr, &self.unresolved_mark)
            && let box Expr::Lit(Lit::Str(str)) = &call_expr.args[0].expr
        {
            let source = str.value.to_string();
            for (idx, dep) in self.async_deps.iter().enumerate() {
                // not only source map, but also span compare
                if dep.source == source
                    && dep.resolve_type.is_esm()
                    && let Some(dep_span) = dep.span
                    && dep_span.contains(str.span)
                {
                    let ident_name = quote_ident!(format!("{}{}__", ASYNC_IMPORTED_MODULE, idx));
                    if !self.async_deps_idents.contains(&ident_name) {
                        self.async_deps_idents.push(ident_name.clone());

                        let require_stmt: Stmt = n
                            .take()
                            .into_var_decl(VarDeclKind::Var, ident_name.clone().into())
                            .into();
                        self.prepend_module_items.push(require_stmt.into());

                        *n = ident_name.into();

                        self.found = true;
                        return;
                    } else {
                        *n = ident_name.into();
                        self.found = true;
                        return;
                    }
                }
            }
        }

        n.visit_mut_children_with(self);
    }

    fn visit_mut_module_items(&mut self, module_items: &mut Vec<ModuleItem>) {
        let mut fist_import_pos: Option<usize> = None;

        // Collect the idents of all async deps, while recording the position of the last import statement
        for (i, module_item) in module_items.iter_mut().enumerate() {
            self.found = false;
            module_item.visit_mut_with(self);
            if self.found && fist_import_pos.is_none() {
                fist_import_pos = Some(i);
                self.first_async_dep_import_pos = i;
            }
        }

        if !self.async_deps_idents.is_empty() {
            // Insert code after the last import statement: `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
            self.prepend_module_items.push(ModuleItem::Stmt(
                quote_ident!("handleAsyncDeps")
                    .as_call(
                        DUMMY_SP,
                        vec![ArrayLit {
                            span: DUMMY_SP,
                            elems: self
                                .async_deps_idents
                                .iter()
                                .map(|ident| Some(ident.clone().as_arg()))
                                .collect(),
                        }
                        .as_arg()],
                    )
                    .into_var_decl(
                        VarDeclKind::Var,
                        quote_ident!("__mako_async_dependencies__").into(),
                    )
                    .into(),
            ));

            // Insert code: `[async1, async2] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;`
            self.prepend_module_items.push(ModuleItem::Stmt(
                AssignExpr {
                    op: AssignOp::Assign,
                    left: ArrayLit {
                        span: DUMMY_SP,
                        elems: self
                            .async_deps_idents
                            .iter()
                            .map(|ident| Some(ident.clone().as_arg()))
                            .collect(),
                    }
                    .as_pat_or_expr(),
                    right: CondExpr {
                        test: member_expr!(DUMMY_SP, __mako_async_dependencies__.then),
                        cons: ParenExpr {
                            expr: AwaitExpr {
                                span: DUMMY_SP,
                                arg: quote_ident!("__mako_async_dependencies__").into(),
                            }
                            .into(),
                            span: DUMMY_SP,
                        }
                        .as_call(DUMMY_SP, vec![])
                        .into(),
                        alt: quote_ident!("__mako_async_dependencies__").into(),
                        span: DUMMY_SP,
                    }
                    .into(),
                    span: DUMMY_SP,
                }
                .into_stmt(),
            ));
        }

        module_items.splice(
            self.first_async_dep_import_pos..self.first_async_dep_import_pos,
            self.prepend_module_items.take(),
        );

        // Insert code: `asyncResult()`
        let call_async_result = quote_ident!("asyncResult")
            .as_call(DUMMY_SP, vec![])
            .into_stmt();
        module_items.push(call_async_result.into());

        // Wrap async module with `__mako_require__._async(
        //   module, async (handleAsyncDeps, asyncResult) => { }, bool
        // );`
        *module_items = vec![ModuleItem::Stmt(
            member_expr!(DUMMY_SP, __mako_require__._async)
                .as_call(
                    DUMMY_SP,
                    vec![
                        quote_ident!("module").as_arg(),
                        {
                            let mut arrow_fn = quote_expr!(DUMMY_SP, null).into_lazy_arrow(vec![
                                quote_ident!("handleAsyncDeps").into(),
                                quote_ident!("asyncResult").into(),
                            ]);
                            arrow_fn.is_async = true;
                            arrow_fn.body = BlockStmtOrExpr::BlockStmt(BlockStmt {
                                span: DUMMY_SP,
                                stmts: module_items
                                    .iter()
                                    .map(|stmt| stmt.as_stmt().unwrap().clone())
                                    .collect(),
                            })
                            .into();
                            arrow_fn.as_arg()
                        },
                        Lit::from(self.top_level_await).as_arg(),
                    ],
                )
                .into_stmt(),
        )];
    }
}

pub fn mark_async(
    module_ids: &[ModuleId],
    context: &Arc<Context>,
) -> HashMap<ModuleId, Vec<Dependency>> {
    mako_core::mako_profile_function!();
    let mut async_deps_by_module_id = HashMap::new();
    let mut module_graph = context.module_graph.write().unwrap();
    // TODO: 考虑成环的场景
    module_ids.iter().for_each(|module_id| {
        let deps = module_graph.get_dependencies_info(module_id);
        let async_deps: Vec<Dependency> = deps
            .into_iter()
            .filter(|(_, dep, is_async)| dep.resolve_type.is_sync_esm() && *is_async)
            .map(|(_, dep, _)| dep.clone())
            .collect();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        // a module with async deps need to be polluted into async module
        if !info.is_async && !async_deps.is_empty() {
            info.is_async = true;
        }
        async_deps_by_module_id.insert(module_id.clone(), async_deps);
    });
    async_deps_by_module_id
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mako_core::swc_common::GLOBALS;
    use mako_core::swc_ecma_transforms::resolver;
    use mako_core::swc_ecma_visit::{VisitMutWith, VisitWith};
    use mako_core::swc_node_comments::SwcComments;
    use swc_core::ecma::transforms::base::feature::FeatureFlag;
    use swc_core::ecma::transforms::base::helpers::{inject_helpers, Helpers, HELPERS};
    use swc_core::ecma::transforms::module::common_js;
    use swc_core::ecma::transforms::module::import_analysis::import_analyzer;
    use swc_core::ecma::transforms::module::util::ImportInterop;

    use super::AsyncModule;
    use crate::ast::{build_js_ast, js_ast_to_code};
    use crate::chunk::{Chunk, ChunkType};
    use crate::compiler::Context;
    use crate::config::Config;
    use crate::module::ModuleId;
    use crate::visitors::dep_analyzer::DepAnalyzer;

    #[test]
    fn test_default_import_async_module() {
        let code = r#"
import add from './async';
add(1, 2);
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    "use strict";
    Object.defineProperty(exports, "__esModule", {
        value: true
    });
    var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
    var _async__mako_imported_module_0__ = require("./async");
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async__mako_imported_module_0__
    ]);
    [
        _async__mako_imported_module_0__
    ] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _async = _interop_require_default._(_async__mako_imported_module_0__);
    0, _async.default(1, 2);
    asyncResult();
}, true);
"#
                .trim()
        );
    }

    #[test]
    fn test_two_import_async_module() {
        let code = r#"
import add from './async';
add(1, 2);
import foo from "./async_2"
console.log(foo)
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    "use strict";
    Object.defineProperty(exports, "__esModule", {
        value: true
    });
    var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
    var _async__mako_imported_module_0__ = require("./async");
    var _async__mako_imported_module_1__ = require("./async_2");
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async__mako_imported_module_0__,
        _async__mako_imported_module_1__
    ]);
    [
        _async__mako_imported_module_0__,
        _async__mako_imported_module_1__
    ] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _async = _interop_require_default._(_async__mako_imported_module_0__);
    var _async_2 = _interop_require_default._(_async__mako_imported_module_1__);
    0, _async.default(1, 2);
    console.log(_async_2.default);
    asyncResult();
}, true);
"#
                .trim()
        );
    }

    #[test]
    fn test_deep_interop_async_module() {
        let code = r#"
import add from './async';
export * from "./async";
add(1, 2);
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    "use strict";
    Object.defineProperty(exports, "__esModule", {
        value: true
    });
    var _export_star = require("@swc/helpers/_/_export_star");
    var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
    var _async__mako_imported_module_0__ = require("./async");
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async__mako_imported_module_0__
    ]);
    [
        _async__mako_imported_module_0__
    ] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _async = _interop_require_default._(_export_star._(_async__mako_imported_module_0__, exports));
    0, _async.default(1, 2);
    asyncResult();
}, true);
"#
                .trim()
        );
    }

    #[test]
    fn test_require_async_module() {
        let code = r#"
const _async = require('./async');
_async.add(1, 2);
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    "use strict";
    const _async = require('./async');
    _async.add(1, 2);
    asyncResult();
}, true);
"#
            .trim()
        );
    }

    #[test]
    fn test_mix_async_module() {
        let code = r#"
import add from "./miexed_async";
async.add(1, 2);
const async = require('./miexed_async');
        "#
        .trim();
        let (code, _) = transform_code(code, None);
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
__mako_require__._async(module, async (handleAsyncDeps, asyncResult)=>{
    "use strict";
    Object.defineProperty(exports, "__esModule", {
        value: true
    });
    var _interop_require_default = require("@swc/helpers/_/_interop_require_default");
    var _async__mako_imported_module_0__ = require("./miexed_async");
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async__mako_imported_module_0__
    ]);
    [
        _async__mako_imported_module_0__
    ] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _miexed_async = _interop_require_default._(_async__mako_imported_module_0__);
    async.add(1, 2);
    const async = require('./miexed_async');
    asyncResult();
}, true);
"#.trim()
        );
    }

    fn transform_code(origin: &str, path: Option<&str>) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let config = Config {
            devtool: None,
            ..Default::default()
        };
        let context: Arc<Context> = Arc::new(Context {
            config,
            ..Default::default()
        });

        let module_id: ModuleId = "./async".to_string().into();
        let mut chunk = Chunk::new(
            "./async".to_string().into(),
            ChunkType::Entry(module_id, "async".to_string(), false),
        );
        chunk.add_module("./async".to_string().into());

        context.chunk_graph.write().unwrap().add_chunk(chunk);

        let mut ast = build_js_ast(path, origin, &context).unwrap();

        GLOBALS.set(&context.meta.script.globals, || {
            HELPERS.set(&Helpers::new(true), || {
                ast.ast.visit_mut_with(&mut resolver(
                    ast.unresolved_mark,
                    ast.top_level_mark,
                    false,
                ));

                let mut dep_collector = DepAnalyzer::new(ast.unresolved_mark);
                ast.ast.visit_with(&mut dep_collector);

                let import_interop = ImportInterop::Swc;
                ast.ast
                    .visit_mut_with(&mut import_analyzer(import_interop, true));
                ast.ast
                    .visit_mut_with(&mut inject_helpers(ast.unresolved_mark));

                ast.ast.visit_mut_with(&mut common_js::<SwcComments>(
                    ast.unresolved_mark,
                    Default::default(),
                    FeatureFlag::empty(),
                    None,
                ));

                let mut async_module =
                    AsyncModule::new(&dep_collector.dependencies, ast.unresolved_mark, true);

                ast.ast.visit_mut_with(&mut async_module);
            })
        });

        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
