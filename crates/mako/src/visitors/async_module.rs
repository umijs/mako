use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use swc_core::common::util::take::Take;
use swc_core::common::{Mark, DUMMY_SP};
use swc_core::ecma::ast::{
    ArrayLit, ArrayPat, AssignExpr, AssignOp, AwaitExpr, BlockStmt, BlockStmtOrExpr, CondExpr,
    Expr, Ident, Lit, ModuleItem, ParenExpr, Stmt, VarDeclKind,
};
use swc_core::ecma::utils::{member_expr, quote_expr, quote_ident, ExprFactory};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils::is_commonjs_require;
use crate::ast::DUMMY_CTXT;
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
                    let ident_name =
                        quote_ident!(DUMMY_CTXT, format!("{}{}__", ASYNC_IMPORTED_MODULE, idx));
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
                    left: ArrayPat {
                        span: DUMMY_SP,
                        optional: false,
                        elems: self
                            .async_deps_idents
                            .iter()
                            .map(|ident| Some(ident.clone().into()))
                            .collect(),
                        type_ann: None,
                    }
                    .into(),
                    right: CondExpr {
                        test: member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_async_dependencies__.then)
                            .into(),
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
            member_expr!(DUMMY_CTXT, DUMMY_SP, __mako_require__._async)
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
                                ctxt: DUMMY_CTXT,
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
    let mut async_deps_by_module_id = HashMap::new();
    let mut module_graph = context.module_graph.write().unwrap();

    let mut to_visit_queue = module_graph
        .modules()
        .iter()
        .filter_map(|m| {
            m.info
                .as_ref()
                .and_then(|i| i.is_async.then(|| m.id.clone()))
        })
        .collect::<VecDeque<_>>();
    let mut visited = HashSet::new();

    // polluted async to dependants
    while let Some(module_id) = to_visit_queue.pop_front() {
        if visited.contains(&module_id) {
            continue;
        }

        module_graph
            .get_dependents(&module_id)
            .iter()
            .filter_map(|(dependant, dependency)| {
                if !dependency.resolve_type.is_sync_esm() {
                    return None;
                }
                if !visited.contains(*dependant) {
                    Some((*dependant).clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .iter()
            .for_each(|module_id| {
                let m = module_graph.get_module_mut(module_id).unwrap();
                m.info.as_mut().unwrap().is_async = true;

                to_visit_queue.push_back(module_id.clone());
            });

        visited.insert(module_id.clone());
    }

    module_ids.iter().for_each(|module_id| {
        let deps = module_graph.get_dependencies_info(module_id);
        let async_deps: Vec<Dependency> = deps
            .into_iter()
            .filter(|(_, dep, is_async)| dep.resolve_type.is_sync_esm() && *is_async)
            .map(|(_, dep, _)| dep.clone())
            .collect();
        if !async_deps.is_empty() {
            async_deps_by_module_id.insert(module_id.clone(), async_deps);
        }
    });

    async_deps_by_module_id
}

#[cfg(test)]
mod tests {
    use swc_core::common::GLOBALS;
    use swc_core::ecma::transforms::base::feature::FeatureFlag;
    use swc_core::ecma::transforms::base::helpers::{inject_helpers, Helpers, HELPERS};
    use swc_core::ecma::transforms::module::common_js;
    use swc_core::ecma::transforms::module::import_analysis::import_analyzer;
    use swc_core::ecma::transforms::module::util::ImportInterop;
    use swc_core::ecma::visit::{VisitMutWith, VisitWith};
    use swc_node_comments::SwcComments;

    use super::AsyncModule;
    use crate::ast::tests::TestUtils;
    use crate::generate::chunk::{Chunk, ChunkType};
    use crate::visitors::dep_analyzer::DepAnalyzer;

    #[test]
    fn test_default_import_async_module() {
        let code = run(r#"
import add from './async';
add(1, 2);
        "#
        .trim());
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
    var __mako_async_dependencies__ = handleAsyncDeps([
        _async__mako_imported_module_0__
    ]);
    [_async__mako_imported_module_0__] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _async = _interop_require_default._(_async__mako_imported_module_0__);
    0, _async.default(1, 2);
    asyncResult();
}, true);
            "#.trim()
    );
    }

    #[test]
    fn test_two_import_async_module() {
        let code = run(r#"
import add from './async';
add(1, 2);
import foo from "./async_2"
console.log(foo)
        "#
        .trim());
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
    [_async__mako_imported_module_0__, _async__mako_imported_module_1__] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
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
        let code = run(r#"
import add from './async';
export * from "./async";
add(1, 2);
                "#
        .trim());
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
    [_async__mako_imported_module_0__] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
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
        let code = run(r#"
const _async = require('./async');
_async.add(1, 2);
                "#
        .trim());
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
        let code = run(r#"
import add from "./miexed_async";
async.add(1, 2);
const async = require('./miexed_async');
                "#
        .trim());
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
    [_async__mako_imported_module_0__] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;
    var _miexed_async = _interop_require_default._(_async__mako_imported_module_0__);
    async.add(1, 2);
    const async = require('./miexed_async');
    asyncResult();
}, true);
"#.trim()
    );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code);
        let mut chunk = Chunk::new(
            "./async".to_string().into(),
            ChunkType::Entry("./async".to_string().into(), "async".to_string(), false),
        );
        chunk.add_module("./async".to_string().into());
        test_utils
            .context
            .chunk_graph
            .write()
            .unwrap()
            .add_chunk(chunk);
        let ast = test_utils.ast.js_mut();
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            HELPERS.set(&Helpers::new(true), || {
                let mut dep_collector =
                    DepAnalyzer::new(ast.unresolved_mark, test_utils.context.clone());
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
        test_utils.js_ast_to_code()
    }
}
