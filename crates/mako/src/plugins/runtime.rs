use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_common::DUMMY_SP as span;
use mako_core::swc_ecma_ast::{
    BlockStmt, FnExpr, Function, Module, ModuleItem, ObjectLit, PropOrSpread, Stmt, UnaryExpr,
    UnaryOp,
};
use mako_core::swc_ecma_utils::{quote_ident, ExprFactory, StmtOrModuleItem};
use mako_core::tracing::debug;

use crate::ast::{build_js_ast, js_ast_to_code};
use crate::compiler::Context;
use crate::generate_chunks::build_props;
use crate::load::read_content;
use crate::module::ModuleAst::Script;
use crate::module::{Dependency, ImportType, ModuleAst, ResolveType};
use crate::plugin::Plugin;
use crate::resolve::resolve;
use crate::task::Task;
use crate::transform::transform;
use crate::transform_in_generate::{transform_js_generate, TransformJsParam};
use crate::transformers::transform_dep_replacer::DependenciesToReplace;

pub struct MakoRuntime {}

impl Plugin for MakoRuntime {
    fn name(&self) -> &str {
        "mako/runtime"
    }

    fn runtime_plugins(&self, context: &Arc<Context>) -> Result<Vec<String>> {
        let plugins = vec![
            self.public_path(context),
            self.helper_runtime(context).unwrap(),
        ];
        Ok(plugins)
    }
}

impl MakoRuntime {
    fn public_path(&self, context: &Arc<Context>) -> String {
        let public_path = context.config.public_path.clone();
        let public_path = if public_path == "runtime" {
            "(typeof globalThis !== 'undefined' ? globalThis : self).publicPath || '/'".to_string()
        } else {
            format!("\"{}\"", public_path)
        };

        format!(
            r#"
  /* mako/runtime/publicPath */
  !function () {{
    requireModule.publicPath= {};
  }}();"#,
            public_path
        )
    }

    fn helper_runtime(&self, context: &Arc<Context>) -> Result<String> {
        let helpers = context.swc_helpers.lock().unwrap().get_helpers();
        debug!("swc helpers: {:?}", helpers);

        if helpers.is_empty() {
            return Ok("".to_string());
        }

        let props = helpers
            .into_iter()
            .map(|source| self.build_module_prop(source.to_string(), context).unwrap())
            .collect::<Vec<_>>();

        let obj_expr = ObjectLit { span, props };

        let module = Module {
            span,
            body: vec![ModuleItem::Stmt(
                UnaryExpr {
                    op: UnaryOp::Bang,
                    span,
                    arg: FnExpr {
                        ident: None,
                        function: Function {
                            params: vec![],
                            decorators: vec![],
                            span,
                            body: Some(BlockStmt {
                                span,
                                stmts: vec![quote_ident!("registerModules")
                                    // registerModules({})
                                    .as_call(span, vec![obj_expr.as_arg()])
                                    .into_stmt()],
                            }),
                            is_generator: false,
                            is_async: false,
                            type_params: None,
                            return_type: None,
                        }
                        .into(),
                    }
                    .as_iife()
                    .into(),
                }
                .into_stmt(),
            )],
            shebang: None,
        };

        let (code, _) = js_ast_to_code(&module, context, "dummy.js").unwrap();

        Ok(code)
    }

    fn build_module_prop(&self, source: String, context: &Arc<Context>) -> Result<PropOrSpread> {
        let virtual_js = context.root.join("__v.js");

        let resolved = resolve(
            virtual_js.to_str().unwrap(),
            &Dependency {
                source: source.clone(),
                resolve_as: None,
                order: 0,
                span: None,
                resolve_type: ResolveType::Import(ImportType::empty()),
            },
            &context.resolvers,
            context,
        )?
        .get_resolved_path();

        let content = read_content(&resolved)?;

        let ast = build_js_ast(&resolved, &content, context)?;
        let mut script = ModuleAst::Script(ast);

        transform(&mut script, context, &Task::from_normal_path(resolved))?;

        let module_id = source.into();

        let mut ast = if let Script(ast) = script {
            ast
        } else {
            unreachable!()
        };

        transform_js_generate(TransformJsParam {
            wrap_async: false,
            top_level_await: false,
            dep_map: &DependenciesToReplace {
                resolved: Default::default(),
                missing: Default::default(),
                ignored: vec![],
            },
            async_deps: &Vec::<Dependency>::new(),
            module_id: &module_id,
            context,
            ast: &mut ast,
        })?;

        let stmts: Result<Vec<Stmt>> = ast
            .ast
            .body
            .into_iter()
            .map(|s| {
                s.into_stmt()
                    .map_err(|e| anyhow!("{:?} not a statement!", e))
            })
            .collect();
        let stmts = stmts.unwrap();

        let factor_decl = FnExpr {
            ident: None,
            function: Function {
                params: vec![
                    quote_ident!("module").into(),
                    quote_ident!("exports").into(),
                    quote_ident!("__mako_require__").into(),
                ],
                is_async: false,
                span,
                decorators: vec![],
                return_type: None,
                type_params: None,
                body: Some(BlockStmt { stmts, span }),
                is_generator: false,
            }
            .into(),
        };

        let obj_prop = build_props(&module_id.generate(context), factor_decl.into());

        Ok(obj_prop)
    }
}
