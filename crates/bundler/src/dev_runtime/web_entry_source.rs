use anyhow::{anyhow, Context, Result};
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, TryJoinIterExt, Value, Vc};
use turbo_tasks_env::ProcessEnv;
use turbo_tasks_fs::FileSystemPath;
use turbopack_browser::BrowserChunkingContext;
use turbopack_core::{
    chunk::{ChunkGroupType, ChunkableModule, ChunkingContext, EvaluatableAsset, SourceMapsType},
    environment::Environment,
    file_source::FileSource,
    module::Module,
    module_graph::ModuleGraph,
    reference_type::{EntryReferenceSubType, ReferenceType},
    resolve::{
        origin::{PlainResolveOrigin, ResolveOriginExt},
        parse::Request,
    },
};
use turbopack_dev_server::{
    html::{DevHtmlAsset, DevHtmlEntry},
    source::{asset_graph::AssetGraphContentSource, ContentSource},
};
use turbopack_ecmascript_runtime::RuntimeType;
use turbopack_node::execution_context::ExecutionContext;

use crate::{
    contexts::{
        get_client_asset_context, get_client_compile_time_info, get_client_resolve_options_context,
        NodeEnv,
    },
    dev_runtime::embed_js::embed_file_path,
};

use super::{
    react_refresh::assert_can_resolve_react_refresh,
    runtime_entry::{RuntimeEntries, RuntimeEntry},
};

#[turbo_tasks::function]
pub async fn get_client_chunking_context(
    root_path: ResolvedVc<FileSystemPath>,
    server_root: ResolvedVc<FileSystemPath>,
    server_root_to_root_path: ResolvedVc<RcStr>,
    environment: ResolvedVc<Environment>,
) -> Result<Vc<Box<dyn ChunkingContext>>> {
    Ok(Vc::upcast(
        BrowserChunkingContext::builder(
            root_path,
            server_root,
            server_root_to_root_path,
            server_root,
            server_root,
            server_root,
            environment,
            RuntimeType::Development,
        )
        .hot_module_replacement()
        .use_file_source_map_uris()
        .build(),
    ))
}

#[turbo_tasks::function]
pub async fn get_client_runtime_entries(
    project_path: ResolvedVc<FileSystemPath>,
    node_env: Vc<NodeEnv>,
) -> Result<Vc<RuntimeEntries>> {
    let resolve_options_context = get_client_resolve_options_context(*project_path, node_env);

    let mut runtime_entries = Vec::new();

    let enable_react_refresh =
        assert_can_resolve_react_refresh(*project_path, resolve_options_context)
            .await?
            .as_request();

    if let Some(request) = enable_react_refresh {
        runtime_entries.push(
            RuntimeEntry::Request(
                request.to_resolved().await?,
                project_path.join("_".into()).to_resolved().await?,
            )
            .resolved_cell(),
        )
    }

    runtime_entries.push(
        RuntimeEntry::Source(ResolvedVc::upcast(
            FileSource::new(embed_file_path("entry/bootstrap.ts".into()))
                .to_resolved()
                .await?,
        ))
        .resolved_cell(),
    );

    Ok(Vc::cell(runtime_entries))
}

#[turbo_tasks::function]
pub async fn create_web_entry_source(
    // TODO: Here is different with turbopack_cli, we need to know why.
    // Maybe turbopack_cli is a different design or a mistake.
    // See https://github.com/vercel/next.js/blob/f4552826e1ed15fbeb951be552d67c5a08ad0672/turbopack/crates/turbopack-cli/src/dev/mod.rs#L311
    project_path: Vc<FileSystemPath>,
    execution_context: Vc<ExecutionContext>,
    entry_requests: Vec<Vc<Request>>,
    server_root: Vc<FileSystemPath>,
    server_root_to_root_path: ResolvedVc<RcStr>,
    _env: Vc<Box<dyn ProcessEnv>>,
    eager_compile: bool,
    node_env: Vc<NodeEnv>,
    source_maps_type: SourceMapsType,
    browserslist_query: RcStr,
) -> Result<Vc<Box<dyn ContentSource>>> {
    let compile_time_info = get_client_compile_time_info(browserslist_query, node_env);

    let asset_context = get_client_asset_context(
        project_path,
        execution_context,
        compile_time_info,
        node_env,
        source_maps_type,
    );

    let chunking_context = get_client_chunking_context(
        project_path,
        server_root,
        *server_root_to_root_path,
        compile_time_info.environment(),
    )
    .to_resolved()
    .await?;

    let entries = get_client_runtime_entries(project_path, node_env);

    let runtime_entries = entries.resolve_entries(asset_context);

    let origin = PlainResolveOrigin::new(asset_context, project_path.join("_".into()));
    let project_dir = &project_path.await?.path;
    let entries = entry_requests
        .into_iter()
        .map(|request_vc| async move {
            let ty = Value::new(ReferenceType::Entry(EntryReferenceSubType::Undefined));

            let request = request_vc.await?;
            origin
                .resolve_asset(request_vc, origin.resolve_options(ty.clone()), ty)
                .await?
                .first_module()
                .await?
                .with_context(|| {
                    format!(
                        "Unable to resolve entry {} from directory {}.",
                        request.request().unwrap(),
                        project_dir
                    )
                })
        })
        .try_join()
        .await?;

    let all_modules = entries
        .iter()
        .copied()
        .chain(
            runtime_entries
                .await?
                .iter()
                .map(|&entry| ResolvedVc::upcast(entry)),
        )
        .collect::<Vec<ResolvedVc<Box<dyn Module>>>>();

    let module_graph =
        ModuleGraph::from_modules(Vc::cell(vec![(all_modules, ChunkGroupType::Evaluated)]))
            .to_resolved()
            .await?;

    let entries: Vec<_> = entries
        .into_iter()
        .map(|module| async move {
            if let (Some(chunkable_module), Some(entry)) = (
                ResolvedVc::try_sidecast::<Box<dyn ChunkableModule>>(module),
                ResolvedVc::try_sidecast::<Box<dyn EvaluatableAsset>>(module),
            ) {
                Ok(DevHtmlEntry {
                    chunkable_module,
                    module_graph,
                    chunking_context,
                    runtime_entries: Some(runtime_entries.with_entry(*entry).to_resolved().await?),
                })
            } else if let Some(chunkable_module) =
                ResolvedVc::try_sidecast::<Box<dyn ChunkableModule>>(module)
            {
                // TODO this is missing runtime code, so it's probably broken and we should also
                // add an ecmascript chunk with the runtime code
                Ok(DevHtmlEntry {
                    chunkable_module,
                    module_graph,
                    chunking_context,
                    runtime_entries: None,
                })
            } else {
                // TODO: convert into a serve-able asset
                Err(anyhow!(
                    "Entry module is not chunkable, so it can't be used to bootstrap the \
                     application"
                ))
            }
        })
        .try_join()
        .await?;

    let entry_asset = Vc::upcast(DevHtmlAsset::new_with_body(
        server_root.join("index.html".into()).to_resolved().await?,
        entries,
        // FIXME: Root node in body is not recommended in next.js
        r#"<div id="root"></div>"#.into(),
    ));

    let graph = Vc::upcast(if eager_compile {
        AssetGraphContentSource::new_eager(server_root, entry_asset)
    } else {
        AssetGraphContentSource::new_lazy(server_root, entry_asset)
    });

    Ok(graph)
}
