use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use md5;
use sailfish::TemplateOnce;
use swc_core::base::try_with_handler;
use swc_core::common::comments::{Comment, CommentKind, Comments};
use swc_core::common::errors::HANDLER;
use swc_core::common::{Span, DUMMY_SP, GLOBALS};
use swc_core::ecma::ast::{
    ArrayLit, AssignOp, BinaryOp, BlockStmt, CondExpr, Expr, ExprOrSpread, FnExpr, Function,
    KeyValueProp, Module as SwcModule, ObjectLit, Prop, PropOrSpread, UnaryExpr, UnaryOp,
};
use swc_core::ecma::atoms::js_word;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::codegen::{Config as JsCodegenConfig, Emitter};
use swc_core::ecma::utils::{quote_ident, quote_str, ExprFactory};
use twox_hash::XxHash64;

use crate::ast::sourcemap::build_source_map_to_buf;
use crate::compiler::Context;
use crate::config::{get_pkg_name, Mode};
use crate::generate::chunk_pot::ChunkPot;
use crate::generate::runtime::AppRuntimeTemplate;
use crate::module::{relative_to_root, Module, ModuleAst};

pub(crate) fn render_module_js(
    ast: &SwcModule,
    context: &Arc<Context>,
) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    crate::mako_profile_function!();

    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let with_minify = context.config.minify && matches!(context.config.mode, Mode::Production);
    let comments = context.meta.script.origin_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();

    let mut emitter = Emitter {
        cfg: JsCodegenConfig::default()
            .with_minify(with_minify)
            .with_target(context.config.output.es_version)
            .with_ascii_only(with_minify)
            .with_omit_last_semi(true),
        cm: cm.clone(),
        comments: if with_minify {
            None
        } else {
            Some(swc_comments)
        },
        wr: Box::new(JsWriter::new(cm, "\n", &mut buf, Some(&mut source_map_buf))),
    };
    emitter.emit_module(ast)?;

    let cm = &context.meta.script.cm;
    let source_map = {
        crate::mako_profile_scope!("build_source_map");
        match context.config.devtool {
            None => None,
            _ => Some(build_source_map_to_buf(&source_map_buf, cm)),
        }
    };

    Ok((buf, source_map))
}

pub(crate) fn empty_module_fn_expr() -> FnExpr {
    let func = Function {
        span: DUMMY_SP,
        ctxt: Default::default(),
        params: vec![
            quote_ident!("module").into(),
            quote_ident!("exports").into(),
            quote_ident!("__mako_require__").into(),
        ],
        decorators: vec![],
        body: Some(BlockStmt {
            ctxt: Default::default(),
            span: DUMMY_SP,
            stmts: vec![],
        }),
        is_generator: false,
        is_async: false,
        type_params: None,
        return_type: None,
    };
    FnExpr {
        ident: None,
        function: func.into(),
    }
}

pub(crate) fn runtime_code(context: &Arc<Context>) -> Result<String> {
    let umd = context.config.umd.clone();
    let chunk_graph = context.chunk_graph.read().unwrap();
    let has_dynamic_chunks = chunk_graph.get_all_chunks().len() > 1;
    let has_hmr = context.args.watch;
    let app_runtime = AppRuntimeTemplate {
        has_dynamic_chunks,
        has_hmr,
        umd,
        is_browser: matches!(context.config.platform, crate::config::Platform::Browser),
        cjs: context.config.cjs,
        chunk_loading_global: context.config.output.chunk_loading_global.clone(),
        cross_origin_loading: context
            .config
            .output
            .cross_origin_loading
            .clone()
            .map(|s| s.to_string()),
        pkg_name: get_pkg_name(&context.root),
        concatenate_enabled: context
            .config
            .optimization
            .as_ref()
            .map_or(false, |o| o.concatenate_modules.unwrap_or(false)),
    };
    let app_runtime = app_runtime.render_once()?;
    let app_runtime = app_runtime.replace(
        "// __inject_runtime_code__",
        &context.plugin_driver.runtime_plugins_code(context)?,
    );
    Ok(app_runtime)
}

pub(crate) fn hash_hashmap<K, V>(map: &HashMap<K, V>) -> u64
where
    K: Hash + Eq + Ord,
    V: Hash,
{
    let mut sorted_kv = map.iter().collect::<Vec<_>>();
    sorted_kv.sort_by_key(|(k, _)| *k);

    let mut hasher: XxHash64 = Default::default();
    for c in sorted_kv {
        c.0.hash(&mut hasher);
        c.1.hash(&mut hasher);
    }
    hasher.finish()
}

pub(crate) fn hash_vec<V>(vec: &[V]) -> u64
where
    V: Hash,
{
    let mut hasher: XxHash64 = Default::default();

    for v in vec {
        v.hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn to_array_lit(elems: Vec<ExprOrSpread>) -> ArrayLit {
    ArrayLit {
        span: DUMMY_SP,
        elems: elems.into_iter().map(Some).collect::<Vec<_>>(),
    }
}

pub(crate) fn pot_to_module_object(pot: &ChunkPot, context: &Arc<Context>) -> Result<ObjectLit> {
    crate::mako_profile_function!();

    let mut sorted_kv = pot.module_map.iter().collect::<Vec<_>>();
    sorted_kv.sort_by_key(|(k, _)| *k);

    let mut props = Vec::new();

    let origin_comments = context.meta.script.origin_comments.read().unwrap();
    let comments = origin_comments.get_swc_comments();

    let cm = context.meta.script.cm.clone();
    GLOBALS.set(&context.meta.script.globals, || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            HANDLER.set(handler, || {
                for (module_id_str, module) in sorted_kv {
                    let fn_expr = to_module_fn_expr(module.0)?;

                    let span = Span::dummy_with_cmt();
                    let id = relative_to_root(&module.0.id.id, &context.root);
                    // to avoid comment broken by glob=**/* for context module
                    let id = id.replace("*/", "*\\/");
                    comments.add_leading(
                        span.hi,
                        Comment {
                            kind: CommentKind::Block,
                            span: DUMMY_SP,
                            text: id.into(),
                        },
                    );
                    let pv: PropOrSpread = Prop::KeyValue(KeyValueProp {
                        key: quote_str!(span, module_id_str.clone()).into(),
                        value: fn_expr.into(),
                    })
                    .into();

                    props.push(pv);
                }
                Ok(())
            })
        })
    })?;

    Ok(ObjectLit {
        span: DUMMY_SP,
        props,
    })
}

pub(crate) fn pot_to_chunk_module(
    pot: &ChunkPot,
    global: String,
    context: &Arc<Context>,
) -> Result<SwcModule> {
    crate::mako_profile_function!();

    let module_object = pot_to_module_object(pot, context)?;

    // ((typeof globalThis !== 'undefined' ? globalThis : self)['makoChunk_global'] = (typeof globalThis !== 'undefined' ? globalThis : self)['makoChunk_global'] || []).push([["module_id"], { module object }])
    let chunk_global_expr = CondExpr {
        span: DUMMY_SP,
        test: UnaryExpr {
            span: DUMMY_SP,
            op: UnaryOp::TypeOf,
            arg: quote_ident!("globalThis").into(),
        }
        .make_bin::<Expr>(BinaryOp::NotEqEq, js_word!("undefined").into())
        .into(),
        cons: quote_ident!("globalThis").into(),
        alt: quote_ident!("self").into(),
    }
    .wrap_with_paren()
    .computed_member::<Expr>(global.clone().into());
    let chunk_global_obj = chunk_global_expr
        .clone()
        .make_bin::<Expr>(
            BinaryOp::LogicalOr,
            ArrayLit {
                span: DUMMY_SP,
                elems: vec![],
            }
            .into(),
        )
        .make_assign_to(AssignOp::Assign, chunk_global_expr.clone().into())
        .wrap_with_paren()
        .make_member(quote_ident!("push"));
    let chunk_register_stmt = chunk_global_obj
        .as_call(
            DUMMY_SP,
            // [[ "module id"], { module object }]
            vec![to_array_lit(vec![
                to_array_lit(vec![quote_str!(pot.chunk_id.clone()).as_arg()]).as_arg(),
                module_object.as_arg(),
            ])
            .as_arg()],
        )
        .into_stmt();

    Ok(SwcModule {
        body: vec![chunk_register_stmt.into()],
        shebang: None,
        span: DUMMY_SP,
    })
}

// #[cached(
//     result = true,
//     key = "String",
//     type = "SizedCache<String, FnExpr>",
//     create = "{ SizedCache::with_size(20000) }",
//     convert = r#"{format!("{}.{:x}",file_content_hash(&module.id.id),module.info.as_ref().unwrap().raw_hash)}"#
// )]
fn to_module_fn_expr(module: &Module) -> Result<FnExpr> {
    crate::mako_profile_function!(&module.id.id);

    match &module.info.as_ref().unwrap().ast {
        ModuleAst::Script(script) => {
            let mut stmts = Vec::new();

            for n in script.ast.body.iter() {
                match n.as_stmt() {
                    None => {
                        return Err(anyhow!(
                            "Error: not a stmt found in {:?}, ast: {:?}",
                            module.id.id,
                            n,
                        ));
                    }
                    Some(stmt) => {
                        stmts.push(stmt.clone());
                    }
                }
            }

            let func = Function {
                span: DUMMY_SP,
                ctxt: Default::default(),
                params: vec![
                    quote_ident!("module").into(),
                    quote_ident!("exports").into(),
                    quote_ident!("__mako_require__").into(),
                ],
                decorators: vec![],
                body: Some(BlockStmt {
                    span: DUMMY_SP,
                    ctxt: Default::default(),
                    stmts,
                }),
                is_generator: false,
                is_async: false,
                type_params: None,
                return_type: None,
            };
            Ok(FnExpr {
                ident: None,
                function: func.into(),
            })
        }
        // TODO: css module should be removed
        ModuleAst::Css(_) => Ok(empty_module_fn_expr()),
        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) cannot concert", module.id.id)),
    }
}

pub const CHUNK_FILE_NAME_HASH_LENGTH: usize = 8;

pub fn file_content_hash<T: AsRef<[u8]>>(content: T) -> String {
    let digest = md5::compute(content);
    let mut hash = format!("{:x}", digest);
    hash.truncate(CHUNK_FILE_NAME_HASH_LENGTH);
    hash
}
