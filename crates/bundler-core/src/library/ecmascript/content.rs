use std::io::Write;

use anyhow::{bail, Result};
use indoc::writedoc;
use serde::Serialize;
use turbo_tasks::{ReadRef, ResolvedVc, TryJoinIterExt, Value, Vc};
use turbo_tasks_fs::{rope::RopeBuilder, File};
use turbopack_core::{
    asset::AssetContent,
    chunk::{ChunkingContext, MinifyType, ModuleChunkItemIdExt, ModuleId},
    code_builder::{Code, CodeBuilder},
    output::OutputAsset,
    source_map::{GenerateSourceMap, OptionStringifiedSourceMap, SourceMapAsset},
    version::{Version, VersionedContent},
};
use turbopack_ecmascript::{
    chunk::{EcmascriptChunkContent, EcmascriptChunkData, EcmascriptChunkPlaceable},
    minify::minify,
    utils::StringifyJs,
};
use turbopack_ecmascript_runtime::RuntimeType;

use super::{chunk::EcmascriptLibraryChunk, version::EcmascriptLibraryChunkVersion};
use crate::library::LibraryChunkingContext;

#[turbo_tasks::value(serialization = "none")]
pub struct EcmascriptLibraryChunkContent {
    pub(super) chunking_context: ResolvedVc<LibraryChunkingContext>,
    pub(super) chunk: ResolvedVc<EcmascriptLibraryChunk>,
    pub(super) content: ResolvedVc<EcmascriptChunkContent>,
    pub(super) source_map: ResolvedVc<SourceMapAsset>,
}

#[turbo_tasks::value_impl]
impl EcmascriptLibraryChunkContent {
    #[turbo_tasks::function]
    pub(crate) async fn new(
        chunking_context: ResolvedVc<LibraryChunkingContext>,
        chunk: ResolvedVc<EcmascriptLibraryChunk>,
        content: ResolvedVc<EcmascriptChunkContent>,
        source_map: ResolvedVc<SourceMapAsset>,
    ) -> Result<Vc<Self>> {
        Ok(EcmascriptLibraryChunkContent {
            chunking_context,
            chunk,
            content,
            source_map,
        }
        .cell())
    }
}

#[turbo_tasks::value_impl]
impl EcmascriptLibraryChunkContent {
    #[turbo_tasks::function]
    pub(crate) async fn own_version(&self) -> Result<Vc<EcmascriptLibraryChunkVersion>> {
        Ok(EcmascriptLibraryChunkVersion::new(
            self.chunking_context.output_root(),
            self.chunk.path(),
            *self.content,
            self.chunking_context.await?.minify_type(),
        ))
    }

    #[turbo_tasks::function]
    async fn code(self: Vc<Self>) -> Result<Vc<Code>> {
        let this = self.await?;
        let environment = this.chunking_context.environment();

        let output_root = this.chunking_context.output_root().await?;
        let output_root_to_root_path = this.chunking_context.output_root_to_root_path();
        let source_maps = *this
            .chunking_context
            .reference_chunk_source_maps(*ResolvedVc::upcast(this.chunk))
            .await?;
        let chunk_path_vc = this.chunk.path();
        let chunk_path = chunk_path_vc.await?;
        let chunk_public_path = if let Some(path) = output_root.get_path_to(&chunk_path) {
            path
        } else {
            bail!(
                "chunk path {} is not in output root {}",
                chunk_path.to_string(),
                output_root.to_string()
            );
        };

        let runtime_module_ids: Vec<ReadRef<ModuleId>> = this
            .chunk
            .await?
            .evaluatable_assets
            .await?
            .iter()
            .map({
                let chunking_context = this.chunking_context;
                move |entry| async move {
                    if let Some(placeable) =
                        ResolvedVc::try_sidecast::<Box<dyn EcmascriptChunkPlaceable>>(*entry)
                    {
                        Ok(Some(
                            placeable
                                .chunk_item_id(Vc::upcast(*chunking_context))
                                .await?,
                        ))
                    } else {
                        Ok(None)
                    }
                }
            })
            .try_join()
            .await?
            .into_iter()
            .flatten()
            .collect();

        let mut code = CodeBuilder::default();

        // When a chunk is executed, it will either register itself with the current
        // instance of the runtime, or it will push itself onto the list of pending
        // chunks (`self.TURBOPACK`).
        //
        // When the runtime executes (see the `evaluate` module), it will pick up and
        // register all pending chunks, and replace the list of pending chunks
        // with itself so later chunks can register directly with it.
        writedoc!(
            code,
            r#"
                (globalThis.TURBOPACK = globalThis.TURBOPACK || []).push([{chunk_path}, {{
            "#,
            chunk_path = StringifyJs(chunk_public_path)
        )?;

        let content = this.content.await?;
        let chunk_items = content.chunk_item_code_and_ids().await?;
        for item in chunk_items {
            for (id, item_code) in item {
                write!(code, "\n{}: ", StringifyJs(&id))?;
                code.push_code(item_code);
                write!(code, ",")?;
            }
        }

        let params = EcmascriptBrowserChunkRuntimeParams {
            other_chunks: &Vec::<EcmascriptChunkData<'_>>::new(),
            runtime_module_ids,
        };

        write!(code, "\n}},")?;
        write!(code, "\n{},", StringifyJs(&params))?;
        writeln!(code, "\n]);")?;

        let runtime_code = turbopack_ecmascript_runtime::get_browser_runtime_code(
            environment,
            Vc::cell(None),
            Vc::cell(None),
            Value::new(RuntimeType::Production),
            output_root_to_root_path,
            source_maps,
        );
        code.push_code(&*runtime_code.await?);

        let mut code = code.build();

        if let MinifyType::Minify { mangle } = this.chunking_context.await?.minify_type() {
            code = minify(&code, source_maps, mangle)?;
        }

        Ok(code.cell())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EcmascriptBrowserChunkRuntimeParams<'a, T> {
    /// Other chunks in the chunk group this chunk belongs to, if any. Does not
    /// include the chunk itself.
    ///
    /// These chunks must be loaed before the runtime modules can be
    /// instantiated.
    other_chunks: &'a [T],
    /// List of module IDs that this chunk should instantiate when executed.
    runtime_module_ids: Vec<ReadRef<ModuleId>>,
}

#[turbo_tasks::value_impl]
impl VersionedContent for EcmascriptLibraryChunkContent {
    #[turbo_tasks::function]
    async fn content(self: Vc<Self>) -> Result<Vc<AssetContent>> {
        let this = self.await?;
        let code = self.code().await?;

        let rope = if code.has_source_map() {
            use std::io::Write;
            let mut rope_builder = RopeBuilder::default();
            rope_builder.concat(code.source_code());
            let source_map_path = this.source_map.path().await?;
            write!(
                rope_builder,
                "\n\n//# sourceMappingURL={}",
                urlencoding::encode(source_map_path.file_name())
            )?;
            rope_builder.build()
        } else {
            code.source_code().clone()
        };

        Ok(AssetContent::file(File::from(rope).into()))
    }

    #[turbo_tasks::function]
    fn version(self: Vc<Self>) -> Vc<Box<dyn Version>> {
        Vc::upcast(self.own_version())
    }
}

#[turbo_tasks::value_impl]
impl GenerateSourceMap for EcmascriptLibraryChunkContent {
    #[turbo_tasks::function]
    fn generate_source_map(self: Vc<Self>) -> Vc<OptionStringifiedSourceMap> {
        self.code().generate_source_map()
    }
}
