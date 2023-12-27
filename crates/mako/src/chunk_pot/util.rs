use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use cached::proc_macro::cached;
use mako_core::anyhow::{anyhow, Result};
use mako_core::cached::SizedCache;
use mako_core::sailfish::TemplateOnce;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    ArrayLit, AssignOp, BinaryOp, BlockStmt, Expr, ExprOrSpread, FnExpr, Function, Ident,
    KeyValueProp, Module as SwcModule, ObjectLit, Program, Prop, PropOrSpread,
};
use mako_core::swc_ecma_codegen::text_writer::JsWriter;
use mako_core::swc_ecma_codegen::{Config as JsCodegenConfig, Emitter};
use mako_core::swc_ecma_utils::{quote_ident, quote_str, ExprFactory};
use mako_core::twox_hash::XxHash64;

use crate::chunk_pot::ChunkPot;
use crate::compiler::Context;
use crate::config::{DevtoolConfig, Mode};
use crate::load::file_content_hash;
use crate::module::{Module, ModuleAst};
use crate::runtime::AppRuntimeTemplate;
use crate::sourcemap::build_source_map;

pub(crate) fn render_module_js(
    ast: &Program,
    context: &Arc<Context>,
) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
    mako_core::mako_profile_function!();

    let mut buf = vec![];
    let mut source_map_buf = Vec::new();
    let cm = context.meta.script.cm.clone();
    let comments = context.meta.script.output_comments.read().unwrap();
    let swc_comments = comments.get_swc_comments();

    let mut emitter = Emitter {
        cfg: JsCodegenConfig::default()
            .with_minify(context.config.minify && matches!(context.config.mode, Mode::Production))
            .with_target(context.config.output.es_version)
            .with_ascii_only(false)
            .with_omit_last_semi(true),
        cm: cm.clone(),
        comments: Some(swc_comments),
        wr: Box::new(JsWriter::new(cm, "\n", &mut buf, Some(&mut source_map_buf))),
    };
    emitter.emit_program(ast)?;
    let cm = &context.meta.script.cm;
    let source_map = {
        mako_core::mako_profile_scope!("build_source_map");
        match context.config.devtool {
            DevtoolConfig::None => None,
            _ => Some(build_source_map(&source_map_buf, cm)),
        }
    };

    Ok((buf, source_map))
}

pub(crate) fn empty_module_fn_expr() -> FnExpr {
    let func = Function {
        span: DUMMY_SP,
        params: vec![
            quote_ident!("module").into(),
            quote_ident!("exports").into(),
            quote_ident!("__mako_require__").into(),
        ],
        decorators: vec![],
        body: Some(BlockStmt {
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

#[cached(
    result = true,
    key = "u64",
    type = "SizedCache<u64, String>",
    convert = r#"{context.config_hash}"#,
    create = "{ SizedCache::with_size(5) }"
)]
pub(crate) fn runtime_code(context: &Arc<Context>) -> Result<String> {
    let umd = if context.config.umd != "none" {
        Some(context.config.umd.clone())
    } else {
        None
    };
    let chunk_graph = context.chunk_graph.read().unwrap();
    let has_dynamic_chunks = chunk_graph.get_all_chunks().len() > 1;
    let has_hmr = context.args.watch;
    let app_runtime = AppRuntimeTemplate {
        has_dynamic_chunks,
        has_hmr,
        umd,
        chunk_loading_global: context.config.output.chunk_loading_global.clone(),
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
    let mut sorted_kv = map.iter().map(|(k, v)| (k, v)).collect::<Vec<_>>();
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

pub(crate) fn pot_to_module_object(pot: &ChunkPot) -> Result<ObjectLit> {
    mako_core::mako_profile_function!();

    let mut sorted_kv = pot
        .module_map
        .iter()
        .map(|(k, v)| (k, v))
        .collect::<Vec<_>>();
    sorted_kv.sort_by_key(|(k, _)| *k);

    let mut props = Vec::new();

    for (module_id_str, module) in sorted_kv {
        let fn_expr = to_module_fn_expr(module.0)?;

        let pv: PropOrSpread = Prop::KeyValue(KeyValueProp {
            key: quote_str!(module_id_str.clone()).into(),
            value: fn_expr.into(),
        })
        .into();

        props.push(pv);
    }

    Ok(ObjectLit {
        span: DUMMY_SP,
        props,
    })
}

pub(crate) fn pot_to_chunk_module(pot: &ChunkPot, global: String) -> Result<SwcModule> {
    mako_core::mako_profile_function!();

    let module_object = pot_to_module_object(pot)?;

    // (globalThis['makoChunk_global'] = globalThis['makoChunk_global'] || []).push([["module_id"], { module object }])
    let chunk_global_expr =
        quote_ident!("globalThis").computed_member::<Expr>(global.clone().into());
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
        .make_assign_to(AssignOp::Assign, chunk_global_expr.clone().as_pat_or_expr())
        .wrap_with_paren()
        .make_member::<Ident>(quote_ident!("push"));
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

#[cached(
    result = true,
    key = "String",
    type = "SizedCache<String, FnExpr>",
    create = "{ SizedCache::with_size(20000) }",
    convert = r#"{format!("{}.{:x}",file_content_hash(&module.id.id),module.info.as_ref().unwrap().raw_hash)}"#
)]
pub fn to_module_fn_expr(module: &Module) -> Result<FnExpr> {
    mako_core::mako_profile_function!(&module.id.id);

    match &module.info.as_ref().unwrap().ast {
        ModuleAst::Script(script) => {
            let stmts = script.get_stmts()?;
            let func = Function {
                span: DUMMY_SP,
                params: vec![
                    quote_ident!("module").into(),
                    quote_ident!("exports").into(),
                    quote_ident!("__mako_require__").into(),
                ],
                decorators: vec![],
                body: Some(BlockStmt {
                    span: DUMMY_SP,
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
        // TODO:  css module will be removed in the future
        ModuleAst::Css(_) => Ok(empty_module_fn_expr()),
        ModuleAst::None => Err(anyhow!("ModuleAst::None({}) cannot concert", module.id.id)),
    }
}
