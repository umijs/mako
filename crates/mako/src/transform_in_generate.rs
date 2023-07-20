use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use swc_common::errors::HANDLER;
use swc_common::{DUMMY_SP, GLOBALS};
use swc_css_visit::VisitMutWith as CSSVisitMutWith;
use swc_ecma_ast::{
    ArrayLit, ArrowExpr, AssignExpr, AssignOp, AwaitExpr, BindingIdent, BlockStmt, BlockStmtOrExpr,
    CallExpr, Callee, CondExpr, Decl, Expr, ExprOrSpread, ExprStmt, Ident, Lit, MemberExpr,
    MemberProp, ModuleItem, ParenExpr, Pat, PatOrExpr, Stmt, Str, VarDecl, VarDeclKind,
    VarDeclarator,
};
use swc_ecma_transforms::feature::FeatureFlag;
use swc_ecma_transforms::fixer;
use swc_ecma_transforms::helpers::{inject_helpers, Helpers, HELPERS};
use swc_ecma_transforms::hygiene::hygiene_with_config;
use swc_ecma_transforms::modules::common_js;
use swc_ecma_transforms::modules::import_analysis::import_analyzer;
use swc_ecma_transforms::modules::util::{Config, ImportInterop};
use swc_ecma_visit::VisitMutWith;
use swc_error_reporters::handler::try_with_handler;

use crate::ast::{base64_encode, build_js_ast, css_ast_to_code, Ast};
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode};
use crate::module::{Dependency, ModuleAst, ModuleId, ModuleInfo, ResolveType};
use crate::targets;
use crate::transform_dep_replacer::DepReplacer;
use crate::transform_dynamic_import::DynamicImport;
use crate::transform_react::react_refresh_entry_prefix;
use crate::unused_statement_sweep::UnusedStatementSweep;

impl Compiler {
    pub fn transform_all(&self) -> Result<()> {
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        transform_modules(module_ids, context)?;
        Ok(())
    }
}

pub fn transform_modules(module_ids: Vec<ModuleId>, context: &Arc<Context>) -> Result<()> {
    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies_info(module_id);

        let dep_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep, _)| (dep.source, id.generate(context)))
            .collect();
        drop(module_graph);

        // let deps: Vec<(&ModuleId, &crate::module::Dependency)> =
        //     module_graph.get_dependencies(module_id);
        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;

        if let ModuleAst::Script(ast) = ast {
            transform_js_generate(&module.id, context, ast, &dep_map, module.is_entry);
        }

        // 通过开关控制是否单独提取css文件
        if !context.config.extract_css {
            if let ModuleAst::Css(ast) = ast {
                let ast = transform_css(ast, &path, dep_map, context);
                info.set_ast(ModuleAst::Script(ast));
            }
        }
    });
    Ok(())
}

pub fn transform_js_generate(
    id: &ModuleId,
    context: &Arc<Context>,
    ast: &mut Ast,
    dep_map: &HashMap<String, String>,
    is_entry: bool,
) {
    let is_dev = matches!(context.config.mode, Mode::Development);
    GLOBALS
        .set(&context.meta.script.globals, || {
            try_with_handler(
                context.meta.script.cm.clone(),
                Default::default(),
                |handler| {
                    HELPERS.set(&Helpers::new(true), || {
                        HANDLER.set(handler, || {
                            let unresolved_mark = ast.unresolved_mark;
                            let top_level_mark = ast.top_level_mark;
                            // let (code, ..) = js_ast_to_code(&ast.ast, context, "foo").unwrap();
                            // print!("{}", code);
                            {
                                if context.config.minify
                                    && matches!(context.config.mode, Mode::Production)
                                {
                                    let comments =
                                        context.meta.script.output_comments.read().unwrap();
                                    let mut unused_statement_sweep =
                                        UnusedStatementSweep::new(id, &comments);
                                    ast.ast.visit_mut_with(&mut unused_statement_sweep);
                                }
                            }

                            let import_interop = ImportInterop::Swc;
                            // FIXME: 执行两轮 import_analyzer + inject_helpers，第一轮是为了 module_graph，第二轮是为了依赖替换
                            ast.ast
                                .visit_mut_with(&mut import_analyzer(import_interop, true));
                            ast.ast.visit_mut_with(&mut inject_helpers(unresolved_mark));
                            ast.ast.visit_mut_with(&mut common_js(
                                unresolved_mark,
                                Config {
                                    import_interop: Some(import_interop),
                                    // NOTE: 这里后面要调整为注入自定义require
                                    ignore_dynamic: true,
                                    preserve_import_meta: true,
                                    ..Default::default()
                                },
                                FeatureFlag::empty(),
                                Some(
                                    context
                                        .meta
                                        .script
                                        .origin_comments
                                        .read()
                                        .unwrap()
                                        .get_swc_comments(),
                                ),
                            ));

                            if is_entry && is_dev {
                                ast.ast
                                    .visit_mut_with(&mut react_refresh_entry_prefix(context));
                            }

                            let mut dep_replacer = DepReplacer {
                                dep_map: dep_map.clone(),
                                context,
                            };
                            ast.ast.visit_mut_with(&mut dep_replacer);

                            let mut dynamic_import = DynamicImport { context };
                            ast.ast.visit_mut_with(&mut dynamic_import);

                            ast.ast.visit_mut_with(&mut hygiene_with_config(
                                swc_ecma_transforms::hygiene::Config {
                                    top_level_mark,
                                    ..Default::default()
                                },
                            ));
                            ast.ast.visit_mut_with(&mut fixer(Some(
                                context
                                    .meta
                                    .script
                                    .origin_comments
                                    .read()
                                    .unwrap()
                                    .get_swc_comments(),
                            )));

                            Ok(())
                        })
                    })
                },
            )
        })
        .unwrap();
}

const ASYNC_DEPS_IDENT: &str = "__mako_async_dependencies__";
const ASYNC_IMPORTED_MODULE: &str = "_async__mako_imported_module_";

fn transform_async_module(
    ast: &mut Ast,
    dep_map: HashMap<String, (Dependency, ModuleInfo)>,
    top_level_await: bool,
) {
    handle_async_deps(ast, dep_map);
    wrap_async_module(ast, top_level_await);
}

/// handle async module dependency
fn handle_async_deps(ast: &mut Ast, dep_map: HashMap<String, (Dependency, ModuleInfo)>) {
    // get all async deps, such as ['async1', 'async2']
    let mut async_deps = vec![];
    // get the index of last async dep in ast.body
    let mut last_async_dep_index = 0;

    for (i, module_item) in ast.ast.body.iter_mut().enumerate() {
        if let ModuleItem::Stmt(stmt) = module_item {
            match stmt {
                // `require('./async');` => `var _async__mako_imported_module_n__ = require('./async');`
                Stmt::Expr(expr_stmt) => {
                    if let Expr::Call(call_expr) = &*expr_stmt.expr {
                        if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident { sym, .. })) =
                            &call_expr.callee
                        {
                            if sym == "require" {
                                if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                    &*call_expr.args[0].expr
                                {
                                    let source = value.to_string();
                                    if let Some((dep, info)) = dep_map.get(&source) {
                                        if matches!(dep.resolve_type, ResolveType::Import) {
                                            // filter the async deps
                                            if info.is_async {
                                                let ident_name =
                                                    format!("{}{}__", ASYNC_IMPORTED_MODULE, i);
                                                *stmt = Stmt::Decl(Decl::Var(Box::new(VarDecl {
                                                    span: DUMMY_SP,
                                                    kind: VarDeclKind::Var,
                                                    declare: false,
                                                    decls: vec![VarDeclarator {
                                                        span: DUMMY_SP,
                                                        name: Pat::Ident(BindingIdent {
                                                            id: Ident {
                                                                span: DUMMY_SP,
                                                                sym: ident_name.clone().into(),
                                                                optional: false,
                                                            },
                                                            type_ann: None,
                                                        }),
                                                        init: Some(Box::new(
                                                            *expr_stmt.expr.clone(),
                                                        )),
                                                        definite: false,
                                                    }],
                                                })));
                                                async_deps.push(ident_name);
                                                last_async_dep_index = i;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // `var async = require('./async');`
                Stmt::Decl(Decl::Var(var_decl)) => {
                    for decl in &var_decl.decls {
                        if let Some(box Expr::Call(call_expr)) = &decl.init {
                            if let Callee::Expr(box Expr::Ident(swc_ecma_ast::Ident {
                                sym, ..
                            })) = &call_expr.callee
                            {
                                if sym == "require" {
                                    if let Expr::Lit(Lit::Str(Str { value, .. })) =
                                        &*call_expr.args[0].expr
                                    {
                                        let source = value.to_string();
                                        if let Some((dep, info)) = dep_map.get(&source) {
                                            if matches!(dep.resolve_type, ResolveType::Import) {
                                                // filter the async deps
                                                if info.is_async {
                                                    if let Pat::Ident(binding_ident) = &decl.name {
                                                        async_deps
                                                            .push(binding_ident.id.sym.to_string());
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
                _ => {}
            }
        }
    }

    // insert a new stmt after all async deps
    // `var __mako_async_dependencies__ = handleAsyncDeps([async1, async2]);`
    ast.ast.body.insert(
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
    ast.ast.body.insert(
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
fn wrap_async_module(ast: &mut Ast, top_level_await: bool) {
    ast.ast.body.push(ModuleItem::Stmt(Stmt::Expr(ExprStmt {
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

    ast.ast.body = vec![ModuleItem::Stmt(Stmt::Expr(ExprStmt {
        span: DUMMY_SP,
        expr: Box::new(Expr::Call(CallExpr {
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
                                .ast
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
        })),
    }))];
}

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    path: &str,
    dep_map: HashMap<String, String>,
    context: &Arc<Context>,
) -> Ast {
    // prefixer
    let mut prefixer = swc_css_prefixer::prefixer(swc_css_prefixer::options::Options {
        env: Some(targets::swc_preset_env_targets_from_map(
            context.config.targets.clone(),
        )),
    });
    ast.visit_mut_with(&mut prefixer);

    // minifier
    if matches!(context.config.mode, Mode::Production) {
        swc_css_minifier::minify(ast, Default::default());
    }

    // ast to code
    let (mut code, sourcemap) = css_ast_to_code(ast, context);
    // lightingcss
    // let mut code = lightingcss_transform(&code, context);

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
        .filter(|val| val.ends_with(".css"))
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
    use std::sync::Arc;

    use super::transform_css;
    use crate::ast::{build_css_ast, js_ast_to_code};

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
        let context = Arc::new(Default::default());
        let mut ast = build_css_ast(path, content, &context).unwrap();
        let ast = transform_css(&mut ast, path, dep_map, &context);
        let (code, _sourcemap) = js_ast_to_code(&ast.ast, &context, "index.js").unwrap();
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
