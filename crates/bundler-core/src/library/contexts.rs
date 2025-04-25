use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::{
    chunk::{
        module_id_strategies::ModuleIdStrategy, ChunkingContext, MangleType, MinifyType,
        SourceMapsType,
    },
    environment::Environment,
};

use crate::mode::Mode;

use super::LibraryChunkingContext;

#[turbo_tasks::function]
pub async fn get_library_chunking_context(
    root_path: ResolvedVc<FileSystemPath>,
    output_root: ResolvedVc<FileSystemPath>,
    output_root_to_root_path: ResolvedVc<RcStr>,
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
        output_root,
        output_root_to_root_path,
        environment,
        mode.runtime_type(),
    )
    .minify_type(if *minify.await? {
        MinifyType::Minify {
            mangle: (!*no_mangling.await?).then_some(MangleType::OptimalSize),
        }
    } else {
        MinifyType::NoMinify
    })
    .source_maps(if *source_maps.await? {
        SourceMapsType::Full
    } else {
        SourceMapsType::None
    })
    .module_id_strategy(module_id_strategy);

    Ok(Vc::upcast(builder.build()))
}
