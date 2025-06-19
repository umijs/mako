use std::io::Write;

use anyhow::Result;
use indoc::writedoc;
use turbo_rcstr::RcStr;
use turbo_tasks::{Value, Vc};
use turbopack_core::{
    code_builder::{Code, CodeBuilder},
    context::AssetContext,
    environment::{ChunkLoading, Environment},
};
use turbopack_ecmascript::utils::StringifyJs;
use turbopack_ecmascript_runtime::RuntimeType;

use super::{asset_context::get_runtime_asset_context, embed_js::embed_static_code};

/// Returns the code for the ECMAScript runtime.
#[turbo_tasks::function]
pub async fn get_library_runtime_code(
    environment: Vc<Environment>,
    chunk_base_path: Vc<Option<RcStr>>,
    chunk_suffix_path: Vc<Option<RcStr>>,
    runtime_type: Value<RuntimeType>,
    output_root_to_root_path: Vc<RcStr>,
    generate_source_map: bool,
    runtime_root: Vc<Option<RcStr>>,
    runtime_export: Vc<Vec<RcStr>>,
) -> Result<Vc<Code>> {
    let asset_context = get_runtime_asset_context(environment).await?;

    let shared_runtime_utils_code = embed_static_code(
        asset_context,
        "shared/runtime-utils.ts".into(),
        generate_source_map,
    );

    let mut runtime_base_code = vec!["browser/runtime/base/runtime-base.ts"];
    match *runtime_type {
        RuntimeType::Production => runtime_base_code.push("browser/runtime/base/build-base.ts"),
        RuntimeType::Development => {
            runtime_base_code.push("browser/runtime/base/dev-base.ts");
        }
    }

    let chunk_loading = &*asset_context
        .compile_time_info()
        .environment()
        .chunk_loading()
        .await?;

    let mut runtime_backend_code = vec![];
    match (chunk_loading, *runtime_type) {
        (ChunkLoading::Edge, RuntimeType::Development) => {
            runtime_backend_code.push("browser/runtime/edge/runtime-backend-edge.ts");
            runtime_backend_code.push("browser/runtime/edge/dev-backend-edge.ts");
        }
        (ChunkLoading::Edge, RuntimeType::Production) => {
            runtime_backend_code.push("browser/runtime/edge/runtime-backend-edge.ts");
        }
        // This case should never be hit.
        (ChunkLoading::NodeJs, _) => {
            panic!("Node.js runtime is not supported in the browser runtime!")
        }
        (ChunkLoading::Dom, RuntimeType::Development) => {
            runtime_backend_code.push("browser/runtime/dom/runtime-backend-dom.ts");
            runtime_backend_code.push("browser/runtime/dom/dev-backend-dom.ts");
        }
        (ChunkLoading::Dom, RuntimeType::Production) => {
            // TODO
            runtime_backend_code.push("browser/runtime/dom/runtime-backend-dom.ts");
        }
    };

    let mut code: CodeBuilder = CodeBuilder::default();
    let relative_root_path = output_root_to_root_path.await?;
    let chunk_base_path = &*chunk_base_path.await?;
    let chunk_base_path = chunk_base_path.as_ref().map_or_else(|| "", |f| f.as_str());
    let chunk_suffix_path = &*chunk_suffix_path.await?;
    let chunk_suffix_path = chunk_suffix_path
        .as_ref()
        .map_or_else(|| "", |f| f.as_str());

    writedoc!(
        code,
        r#"
            (() => {{
            if (!Array.isArray(globalThis.TURBOPACK)) {{
                return;
            }}

            const CHUNK_BASE_PATH = {};
            const CHUNK_SUFFIX_PATH = {};
            const RELATIVE_ROOT_PATH = {};
            const RUNTIME_PUBLIC_PATH = {};
        "#,
        StringifyJs(chunk_base_path),
        StringifyJs(chunk_suffix_path),
        StringifyJs(relative_root_path.as_str()),
        StringifyJs(chunk_base_path),
    )?;

    code.push_code(&*shared_runtime_utils_code.await?);
    for runtime_code in runtime_base_code {
        code.push_code(
            &*embed_static_code(asset_context, runtime_code.into(), generate_source_map).await?,
        );
    }

    if *environment.supports_commonjs_externals().await? {
        code.push_code(
            &*embed_static_code(
                asset_context,
                "shared-node/base-externals-utils.ts".into(),
                generate_source_map,
            )
            .await?,
        );
    }
    if *environment.node_externals().await? {
        code.push_code(
            &*embed_static_code(
                asset_context,
                "shared-node/node-externals-utils.ts".into(),
                generate_source_map,
            )
            .await?,
        );
    }
    if *environment.supports_wasm().await? {
        code.push_code(
            &*embed_static_code(
                asset_context,
                "shared-node/node-wasm-utils.ts".into(),
                generate_source_map,
            )
            .await?,
        );
    }

    for backend_code in runtime_backend_code {
        code.push_code(
            &*embed_static_code(asset_context, backend_code.into(), generate_source_map).await?,
        );
    }

    // Registering chunks and chunk lists depends on the BACKEND variable, which is set by the
    // specific runtime code, hence it must be appended after it.
    writedoc!(
        code,
        r#"
            const chunksToRegister = globalThis.TURBOPACK;
            globalThis.TURBOPACK = {{ push: registerChunk }};
            chunksToRegister.forEach(registerChunk);
        "#
    )?;
    if matches!(*runtime_type, RuntimeType::Development) {
        writedoc!(
            code,
            r#"
            const chunkListsToRegister = globalThis.TURBOPACK_CHUNK_LISTS || [];
            chunkListsToRegister.forEach(registerChunkList);
            globalThis.TURBOPACK_CHUNK_LISTS = {{ push: registerChunkList }};
        "#
        )?;
    }

    let runtime_root = &*runtime_root.await?;
    let runtime_export = &*runtime_export.await?;
    let runtime_export = if runtime_export.is_empty() {
        "".to_string()
    } else {
        runtime_export
            .iter()
            .map(|e| format!("[{}]", StringifyJs(e)))
            .collect::<Vec<String>>()
            .join("")
    };

    writedoc!(
        code,
        r#"
            function factory () {{
                return esmImport(null, Array.from(runtimeModules));
            }};

            if (typeof exports === 'object' && typeof module === 'object') {{
                module.exports = factory();
            }} else if(typeof define === 'function' && define.amd) {{
                define([], factory);
            }} else if (typeof exports === 'object') {{
        "#,
    )?;

    if let Some(runtime_root) = runtime_root {
        let runtime_root = StringifyJs(runtime_root);
        writedoc!(
            code,
            r#"
                exports[{}] = factory(){};
            }} else {{
                globalThis[{}] = factory(){};
            "#,
            runtime_root,
            runtime_export,
            runtime_root,
            runtime_export,
        )?;
    } else {
        writedoc!(
            code,
            r#"
                var a = factory();
                for(var i in a) exports[i] = a[i]{};
            }} else {{
                var a = factory();
                for(var i in a) globalThis[i] = a[i]{};
            "#,
            runtime_export,
            runtime_export,
        )?;
    }

    writedoc!(
        code,
        r#"
            }}
        "#,
    )?;

    writedoc!(
        code,
        r#"
            }})();
        "#
    )?;

    Ok(Code::cell(code.build()))
}
