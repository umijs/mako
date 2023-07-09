use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs, vec};

use swc_common::DUMMY_SP;
use swc_ecma_ast::{
    ArrayLit, ArrowExpr, AssignExpr, AssignOp, AwaitExpr, BindingIdent, BlockStmt, BlockStmtOrExpr,
    CallExpr, Callee, CondExpr, Decl, Expr, ExprOrSpread, ExprStmt, Ident, Lit, MemberExpr,
    MemberProp, Module, ModuleItem, ParenExpr, Pat, PatOrExpr, Stmt, Str, VarDecl, VarDeclKind,
    VarDeclarator,
};
use tracing::debug;

use crate::ast::{base64_encode, build_js_ast, css_ast_to_code};
use crate::compiler::{Compiler, Context};
use crate::config::DevtoolConfig;
use crate::lightningcss::lightingcss_transform;
use crate::module::{Dependency, ModuleAst, ModuleId, ModuleInfo, ResolveType};

impl Compiler {
    pub fn transform_all(&self) {
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        debug!("module ids: {:?}", module_ids);
        transform_modules(module_ids, context);
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies_info(module_id);
        drop(module_graph);

        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;

        if let ModuleAst::Script(ast) = ast {
            let dep_map: HashMap<String, (Dependency, ModuleInfo)> = deps
                .into_iter()
                .map(|(id, dep, module_info)| (id.generate(context), (dep, module_info)))
                .collect();
            // transform async module
            if info.is_async {
                let ast = transform_js(ast, dep_map, info.top_level_await);
                info.set_ast(ModuleAst::Script(ast));
            }
        } else if let ModuleAst::Css(ast) = ast {
            let dep_map: HashMap<String, String> = deps
                .into_iter()
                // 仅保留 .css 后缀的 require，避免不必要的计算和内存使用
                // .filter(|(id, _dep)| id.id.ends_with(".css"))
                .map(|(id, dep, _)| (dep.source, id.generate(context)))
                .collect();
            let ast = transform_css(ast, &path, dep_map, context);
            info.set_ast(ModuleAst::Script(ast));
        }
    });
}

const ASYNC_DEPS_IDENT: &str = "__mako_async_dependencies__";

fn transform_js(
    ast: &mut Module,
    dep_map: HashMap<String, (Dependency, ModuleInfo)>,
    top_level_await: bool,
) -> Module {
    handle_async_deps(ast, dep_map);
    wrap_async_module(ast, top_level_await)
}

/// handle async module dependency
fn handle_async_deps(ast: &mut Module, dep_map: HashMap<String, (Dependency, ModuleInfo)>) {
    // get all async deps, such as ['async1', 'async2']
    let mut async_deps = vec![];
    // get the index of last async dep in ast.body
    let mut last_async_dep_index = 0;

    for (i, module_item) in ast.body.iter().enumerate() {
        match module_item {
            ModuleItem::Stmt(stmt) => {
                match stmt {
                    Stmt::Expr(expr_stmt) => {
                        if let Expr::Call(call_expr) = &*expr_stmt.expr {
                            if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident {
                                sym, ..
                            })) = &call_expr.callee
                            {
                                if sym == "require" {
                                    // println!("{:?}", call_expr.args);
                                }
                            }
                        }
                    }
                    Stmt::Decl(decl_stmt) => {
                        if let Decl::Var(var_decl) = decl_stmt {
                            for decl in &var_decl.decls {
                                if let Some(box Expr::Call(call_expr)) = &decl.init {
                                    if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident {
                                        sym,
                                        ..
                                    })) = &call_expr.callee
                                    {
                                        // 1. filter which is not require()
                                        if sym == "require" {
                                            if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                                &*call_expr.args[0].expr
                                            {
                                                let source = value.to_string();
                                                if let Some((dep, info)) = dep_map.get(&source) {
                                                    if matches!(
                                                        dep.resolve_type,
                                                        ResolveType::Import
                                                    ) {
                                                        // 2. get the deps which is async module
                                                        if info.is_async {
                                                            if let Pat::Ident(binding_ident) =
                                                                &decl.name
                                                            {
                                                                async_deps.push(
                                                                    binding_ident
                                                                        .id
                                                                        .sym
                                                                        .to_string(),
                                                                );
                                                                last_async_dep_index = i;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // insert a new stmt after all async deps
    // `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
    ast.body.insert(
        last_async_dep_index + 1,
        ModuleItem::Stmt(Stmt::Decl(Decl::Var(Box::new(VarDecl {
            span: DUMMY_SP,
            kind: VarDeclKind::Var,
            declare: false,
            decls: vec![VarDeclarator {
                span: DUMMY_SP,
                name: Pat::Ident(BindingIdent {
                    id: Ident {
                        span: DUMMY_SP,
                        sym: ASYNC_DEPS_IDENT.into(),
                        optional: false,
                    },
                    type_ann: None,
                }),
                init: Some(Box::new(Expr::Call(CallExpr {
                    span: DUMMY_SP,
                    callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                        span: DUMMY_SP,
                        sym: "handleAsyncDeps".into(),
                        optional: false,
                    }))),
                    args: vec![ExprOrSpread {
                        spread: None,
                        expr: Box::new(Expr::Array(ArrayLit {
                            span: DUMMY_SP,
                            elems: async_deps
                                .iter()
                                .map(|dep| {
                                    Some(ExprOrSpread {
                                        spread: None,
                                        expr: Box::new(Expr::Ident(Ident {
                                            span: DUMMY_SP,
                                            sym: dep.clone().into(),
                                            optional: false,
                                        })),
                                    })
                                })
                                .collect(),
                        })),
                    }],
                    type_args: None,
                }))),
                definite: false,
            }],
        })))),
    );

    // insert a new stmt after above stmt
    // `[async1, async2] = __mako_async_dependencies__.then ? (await __mako_async_dependencies__)() : __mako_async_dependencies__;`
    ast.body.insert(
        last_async_dep_index + 2,
        ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(Expr::Assign(AssignExpr {
                span: DUMMY_SP,
                left: PatOrExpr::Expr(Box::new(Expr::Array(ArrayLit {
                    span: DUMMY_SP,
                    elems: async_deps
                        .iter()
                        .map(|dep| {
                            Some(ExprOrSpread {
                                spread: None,
                                expr: Box::new(Expr::Ident(Ident {
                                    span: DUMMY_SP,
                                    sym: dep.clone().into(),
                                    optional: false,
                                })),
                            })
                        })
                        .collect(),
                }))),
                right: Box::new(Expr::Cond(CondExpr {
                    span: DUMMY_SP,
                    test: Box::new(Expr::Member(MemberExpr {
                        span: DUMMY_SP,
                        obj: Box::new(Expr::Ident(Ident {
                            span: DUMMY_SP,
                            sym: ASYNC_DEPS_IDENT.into(),
                            optional: false,
                        })),
                        prop: MemberProp::Ident(Ident {
                            span: DUMMY_SP,
                            sym: "then".into(),
                            optional: false,
                        }),
                    })),
                    cons: Box::new(Expr::Call(CallExpr {
                        span: DUMMY_SP,
                        callee: Callee::Expr(Box::new(Expr::Paren(ParenExpr {
                            span: DUMMY_SP,
                            expr: Box::new(Expr::Await(AwaitExpr {
                                span: DUMMY_SP,
                                arg: Box::new(Expr::Ident(Ident {
                                    span: DUMMY_SP,
                                    sym: ASYNC_DEPS_IDENT.into(),
                                    optional: false,
                                })),
                            })),
                        }))),
                        args: vec![],
                        type_args: None,
                    })),
                    alt: Box::new(Expr::Ident(Ident {
                        span: DUMMY_SP,
                        sym: ASYNC_DEPS_IDENT.into(),
                        optional: false,
                    })),
                })),
                op: AssignOp::Assign,
            })),
        })),
    );
}

/// Wrap async module with `require._async(module, async (handleAsyncDeps, asyncResult) => { });`
fn wrap_async_module(ast: &mut Module, top_level_await: bool) -> Module {
    ast.body.push(ModuleItem::Stmt(Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(Expr::Call(CallExpr {
            span: DUMMY_SP,
            callee: Callee::Expr(Box::new(Expr::Ident(Ident {
                span: DUMMY_SP,
                sym: "asyncResult".into(),
                optional: false,
            }))),
            type_args: None,
            args: vec![],
        })),
    })));

    let require_expr = Expr::Call(CallExpr {
        span: DUMMY_SP,
        callee: Callee::Expr(Box::new(Expr::Ident(Ident {
            span: DUMMY_SP,
            sym: "require._async".into(),
            optional: false,
        }))),
        type_args: None,
        args: vec![
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "module".into(),
                    optional: false,
                })),
            },
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Arrow(ArrowExpr {
                    is_async: true,
                    is_generator: false,
                    type_params: None,
                    return_type: None,
                    span: DUMMY_SP,
                    params: vec![
                        Pat::Ident(BindingIdent {
                            id: Ident {
                                span: DUMMY_SP,
                                sym: "handleAsyncDeps".into(),
                                optional: false,
                            },
                            type_ann: None,
                        }),
                        Pat::Ident(BindingIdent {
                            id: Ident {
                                span: DUMMY_SP,
                                sym: "asyncResult".into(),
                                optional: false,
                            },
                            type_ann: None,
                        }),
                    ],
                    body: Box::new(BlockStmtOrExpr::BlockStmt(BlockStmt {
                        span: DUMMY_SP,
                        stmts: ast
                            .body
                            .iter()
                            .map(|stmt| stmt.as_stmt().unwrap().clone())
                            .collect(),
                    })),
                })),
            },
            ExprOrSpread {
                spread: None,
                expr: Box::new(Expr::Ident(Ident {
                    span: DUMMY_SP,
                    sym: if top_level_await { "1" } else { "0" }.into(),
                    optional: false,
                })),
            },
        ],
    });

    Module {
        shebang: None,
        span: DUMMY_SP,
        body: vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            span: DUMMY_SP,
            expr: Box::new(require_expr),
        }))],
    }
}

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Module {
    // ast to code
    let (code, sourcemap) = css_ast_to_code(ast, context);
    // lightingcss
    let mut code = lightingcss_transform(&code, context);

    // TODO: 后续支持生成单独的 css 文件后需要优化
    if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
        let path_buf = PathBuf::from(path);
        let filename = path_buf.file_name().unwrap();
        fs::write(
            format!(
                "{}.map",
                context.config.output.path.join(filename).to_string_lossy()
            ),
            &sourcemap,
        )
        .unwrap_or(());
        code = format!(
            "{code}\n/*# sourceMappingURL={}.map*/",
            filename.to_string_lossy()
        );
    } else if matches!(context.config.devtool, DevtoolConfig::InlineSourceMap) {
        code = format!(
            "{code}\n/*# sourceMappingURL=data:application/json;charset=utf-8;base64,{}*/",
            base64_encode(&sourcemap)
        );
    }

    // code to js ast
    let content = include_str!("runtime/runtime_css.ts").to_string();
    let content = content.replace("__CSS__", code.as_str());
    let require_code: Vec<String> = dep_map
        .values()
        .map(|val| format!("require(\"{}\");\n", val))
        .collect();
    let content = format!("{}{}", require_code.join(""), content);
    let path = format!("{}.ts", path);
    let path = path.as_str();
    // TODO: handle error
    build_js_ast(path, content.as_str(), context).unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, RwLock};

    use super::transform_css;
    use crate::ast::{build_css_ast, js_ast_to_code};
    use crate::chunk_graph::ChunkGraph;
    use crate::compiler::{Context, Meta};
    use crate::module_graph::ModuleGraph;

    #[test]
    fn test_transform_css() {
        let code = r#"
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None, Default::default());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
let css = `.foo {
  color: red;
}

/*# sourceMappingURL=test.tsx.map*/`;
let style = document.createElement('style');
style.innerHTML = css;
document.head.appendChild(style);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_import() {
        let code = r#"
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) =
            transform_css_code(code, None, HashMap::from([("1".into(), "bar.css".into())]));
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
require("bar.css");
let css = `.foo {
  color: red;
}

/*# sourceMappingURL=test.tsx.map*/`;
let style = document.createElement('style');
style.innerHTML = css;
document.head.appendChild(style);

//# sourceMappingURL=index.js.map
        "#
            .trim()
        );
    }

    fn transform_css_code(
        content: &str,
        path: Option<&str>,
        dep_map: HashMap<String, String>,
    ) -> (String, String) {
        let path = if let Some(p) = path { p } else { "test.tsx" };
        let root = PathBuf::from("/path/to/root");
        let context = Arc::new(Context {
            config: Default::default(),
            root,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            meta: Meta::new(),
        });
        let mut ast = build_css_ast(path, content, &context).unwrap();
        let ast = transform_css(&mut ast, path, dep_map, &context);
        let (code, _sourcemap) = js_ast_to_code(&ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
