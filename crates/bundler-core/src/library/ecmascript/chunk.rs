use anyhow::Result;
use turbo_rcstr::RcStr;
use turbo_tasks::{FxIndexSet, ResolvedVc, TryJoinIterExt, Value, ValueToString, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::{
    asset::{Asset, AssetContent},
    chunk::{Chunk, ChunkingContext, EvaluatableAssets, OutputChunk, OutputChunkRuntimeInfo},
    ident::AssetIdent,
    introspect::{Introspectable, IntrospectableChildren},
    module::Module,
    output::{OutputAsset, OutputAssets},
    source_map::{GenerateSourceMap, OptionStringifiedSourceMap, SourceMapAsset},
    version::VersionedContent,
};
use turbopack_ecmascript::chunk::EcmascriptChunk;

use crate::library::{ecmascript::content::EcmascriptLibraryChunkContent, LibraryChunkingContext};

/// Development Ecmascript chunk.
#[turbo_tasks::value(shared)]
pub struct EcmascriptLibraryChunk {
    chunking_context: ResolvedVc<LibraryChunkingContext>,
    ident: ResolvedVc<AssetIdent>,
    chunk: ResolvedVc<EcmascriptChunk>,
    pub(crate) evaluatable_assets: ResolvedVc<EvaluatableAssets>,
}

#[turbo_tasks::value_impl]
impl EcmascriptLibraryChunk {
    /// Creates a new [`Vc<EcmascriptDevChunk>`].
    #[turbo_tasks::function]
    pub fn new(
        chunking_context: ResolvedVc<LibraryChunkingContext>,
        ident: ResolvedVc<AssetIdent>,
        chunk: ResolvedVc<EcmascriptChunk>,
        evaluatable_assets: ResolvedVc<EvaluatableAssets>,
    ) -> Vc<Self> {
        EcmascriptLibraryChunk {
            chunking_context,
            ident,
            chunk,
            evaluatable_assets,
        }
        .cell()
    }

    #[turbo_tasks::function]
    async fn ident_for_path(&self) -> Result<Vc<AssetIdent>> {
        let mut ident = self.ident.owned().await?;

        ident.add_modifier(modifier().to_resolved().await?);

        let evaluatable_assets = self.evaluatable_assets.await?;
        ident.modifiers.extend(
            evaluatable_assets
                .iter()
                .map(|entry| entry.ident().to_string().to_resolved())
                .try_join()
                .await?,
        );

        Ok(AssetIdent::new(Value::new(ident)))
    }

    #[turbo_tasks::function]
    async fn source_map(self: Vc<Self>) -> Result<Vc<SourceMapAsset>> {
        let this = self.await?;
        Ok(SourceMapAsset::new(
            Vc::upcast(*this.chunking_context),
            self.ident_for_path(),
            Vc::upcast(self),
        ))
    }
}

#[turbo_tasks::value_impl]
impl ValueToString for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    fn to_string(&self) -> Vc<RcStr> {
        Vc::cell("Ecmascript Dev Chunk".into())
    }
}

#[turbo_tasks::value_impl]
impl OutputChunk for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    async fn runtime_info(&self) -> Result<Vc<OutputChunkRuntimeInfo>> {
        Ok(OutputChunkRuntimeInfo {
            included_ids: Some(self.chunk.entry_ids().to_resolved().await?),
            ..Default::default()
        }
        .cell())
    }
}

#[turbo_tasks::function]
fn modifier() -> Vc<RcStr> {
    Vc::cell("ecmascript dev chunk".into())
}

#[turbo_tasks::value_impl]
impl EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    async fn own_content(self: Vc<Self>) -> Result<Vc<EcmascriptLibraryChunkContent>> {
        let this = self.await?;
        Ok(EcmascriptLibraryChunkContent::new(
            *this.chunking_context,
            self,
            this.chunk.chunk_content(),
        ))
    }

    #[turbo_tasks::function]
    pub fn chunk(&self) -> Result<Vc<Box<dyn Chunk>>> {
        Ok(Vc::upcast(*self.chunk))
    }
}

#[turbo_tasks::value_impl]
impl OutputAsset for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    async fn path(self: Vc<Self>) -> Result<Vc<FileSystemPath>> {
        let this = self.await?;
        let ident = self.ident_for_path();
        Ok(this
            .chunking_context
            .chunk_path(Some(Vc::upcast(self)), ident, ".js".into()))
    }

    #[turbo_tasks::function]
    fn size_bytes(self: Vc<Self>) -> Vc<Option<u64>> {
        self.own_content().content().len()
    }

    #[turbo_tasks::function]
    async fn references(self: Vc<Self>) -> Result<Vc<OutputAssets>> {
        let this = self.await?;
        let chunk_references = this.chunk.references().await?;
        let include_source_map = *this
            .chunking_context
            .reference_chunk_source_maps(Vc::upcast(self))
            .await?;
        let mut references =
            Vec::with_capacity(chunk_references.len() + if include_source_map { 1 } else { 0 });

        references.extend(chunk_references.iter().copied());

        if include_source_map {
            references.push(ResolvedVc::upcast(self.source_map().to_resolved().await?));
        }

        Ok(Vc::cell(references))
    }
}

#[turbo_tasks::value_impl]
impl Asset for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    fn content(self: Vc<Self>) -> Vc<AssetContent> {
        self.own_content().content()
    }

    #[turbo_tasks::function]
    fn versioned_content(self: Vc<Self>) -> Vc<Box<dyn VersionedContent>> {
        Vc::upcast(self.own_content())
    }
}

#[turbo_tasks::value_impl]
impl GenerateSourceMap for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    fn generate_source_map(self: Vc<Self>) -> Vc<OptionStringifiedSourceMap> {
        self.own_content().generate_source_map()
    }

    #[turbo_tasks::function]
    fn by_section(self: Vc<Self>, section: RcStr) -> Vc<OptionStringifiedSourceMap> {
        self.own_content().by_section(section)
    }
}

#[turbo_tasks::function]
fn introspectable_type() -> Vc<RcStr> {
    Vc::cell("dev ecmascript chunk".into())
}

#[turbo_tasks::function]
fn introspectable_details() -> Vc<RcStr> {
    Vc::cell("generates a development ecmascript chunk".into())
}

#[turbo_tasks::value_impl]
impl Introspectable for EcmascriptLibraryChunk {
    #[turbo_tasks::function]
    fn ty(&self) -> Vc<RcStr> {
        introspectable_type()
    }

    #[turbo_tasks::function]
    fn title(self: Vc<Self>) -> Vc<RcStr> {
        self.path().to_string()
    }

    #[turbo_tasks::function]
    fn details(&self) -> Vc<RcStr> {
        introspectable_details()
    }

    #[turbo_tasks::function]
    async fn children(&self) -> Result<Vc<IntrospectableChildren>> {
        let mut children = FxIndexSet::default();
        let chunk = ResolvedVc::upcast::<Box<dyn Introspectable>>(self.chunk);
        children.insert((ResolvedVc::cell("chunk".into()), chunk));
        Ok(Vc::cell(children))
    }
}
