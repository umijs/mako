use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::{
    chunk::{module_id_strategies::ModuleIdStrategy, ChunkingContext, MinifyType, SourceMapsType},
    environment::Environment,
};

use crate::mode::Mode;

use super::LibraryChunkingContext;

#[turbo_tasks::function]
pub async fn get_library_chunking_context(
    root_path: ResolvedVc<FileSystemPath>,
    library_root: ResolvedVc<FileSystemPath>,
    library_root_to_root_path: ResolvedVc<RcStr>,
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
    let builder = LibraryChunkingContext::builder(
        root_path,
        library_root,
        library_root_to_root_path,
        library_root,
        library_root,
        library_root,
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

    Ok(Vc::upcast(builder.build()))
}
