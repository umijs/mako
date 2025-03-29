use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack::{
    module_options::ModuleOptionsContext, resolve_options_context::ResolveOptionsContext,
};
use turbopack_browser::BrowserChunkingContext;
use turbopack_core::{
    chunk::{
        module_id_strategies::ModuleIdStrategy, ChunkingConfig, ChunkingContext, MinifyType,
        SourceMapsType,
    },
    environment::Environment,
};
use turbopack_node::execution_context::ExecutionContext;

use crate::{client::runtime_entry::RuntimeEntries, config::Config, mode::Mode};

use super::{react_refresh::assert_can_resolve_react_refresh, runtime_entry::RuntimeEntry};

#[turbo_tasks::function]
pub async fn get_client_runtime_entries(
    project_root: Vc<FileSystemPath>,
    mode: Vc<Mode>,
    config: Vc<Config>,
    execution_context: Vc<ExecutionContext>,
) -> Result<Vc<RuntimeEntries>> {
    let mut runtime_entries = vec![];
    let resolve_options_context =
        get_client_resolve_options_context(project_root, mode, config, execution_context);

    if mode.await?.is_development() {
        let enable_react_refresh =
            assert_can_resolve_react_refresh(project_root, resolve_options_context)
                .await?
                .as_request();

        // It's important that React Refresh come before the regular bootstrap file,
        // because the bootstrap contains JSX which requires Refresh's global
        // functions to be available.
        if let Some(request) = enable_react_refresh {
            runtime_entries.push(
                RuntimeEntry::Request(
                    request.to_resolved().await?,
                    project_root.join("_".into()).to_resolved().await?,
                )
                .resolved_cell(),
            )
        };
    }

    Ok(Vc::cell(runtime_entries))
}

#[turbo_tasks::function]
pub async fn get_client_module_options_context(
    project_path: ResolvedVc<FileSystemPath>,
    execution_context: ResolvedVc<ExecutionContext>,
    env: ResolvedVc<Environment>,
    mode: Vc<Mode>,
    config: Vc<Config>,
    no_mangling: Vc<bool>,
) -> Result<Vc<ModuleOptionsContext>> {
    todo!()
}

#[turbo_tasks::function]
pub async fn get_client_resolve_options_context(
    project_path: ResolvedVc<FileSystemPath>,
    mode: Vc<Mode>,
    config: Vc<Config>,
    execution_context: Vc<ExecutionContext>,
) -> Result<Vc<ResolveOptionsContext>> {
    todo!()
}

#[turbo_tasks::function]
pub async fn get_client_chunking_context(
    root_path: ResolvedVc<FileSystemPath>,
    client_root: ResolvedVc<FileSystemPath>,
    client_root_to_root_path: ResolvedVc<RcStr>,
    asset_prefix: ResolvedVc<Option<RcStr>>,
    chunk_suffix_path: ResolvedVc<Option<RcStr>>,
    environment: ResolvedVc<Environment>,
    mode: Vc<Mode>,
    module_id_strategy: ResolvedVc<Box<dyn ModuleIdStrategy>>,
    minify: Vc<bool>,
    source_maps: Vc<bool>,
    no_mangling: Vc<bool>,
) -> Result<Vc<Box<dyn ChunkingContext>>> {
    let mode = mode.await?;
    let mut builder = BrowserChunkingContext::builder(
        root_path,
        client_root,
        client_root_to_root_path,
        client_root,
        client_root.join("dist".into()).to_resolved().await?,
        client_root.join("dist".into()).to_resolved().await?,
        environment,
        mode.runtime_type(),
    )
    .chunk_base_path(asset_prefix)
    .chunk_suffix_path(chunk_suffix_path)
    .minify_type(if *minify.await? {
        MinifyType::Minify {
            mangle: !*no_mangling.await?,
        }
    } else {
        MinifyType::NoMinify
    })
    .source_maps(if *source_maps.await? {
        SourceMapsType::Full
    } else {
        SourceMapsType::None
    })
    .asset_base_path(asset_prefix)
    .module_id_strategy(module_id_strategy);

    if mode.is_development() {
        builder = builder.hot_module_replacement().use_file_source_map_uris();
    } else {
        builder = builder.ecmascript_chunking_config(ChunkingConfig {
            min_chunk_size: 50_000,
            max_chunk_count_per_group: 40,
            max_merge_chunk_size: 200_000,
            ..Default::default()
        })
    }

    Ok(Vc::upcast(builder.build()))
}
