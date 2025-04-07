use anyhow::{bail, Context, Result};
use rustc_hash::FxHashMap;
use tracing::Instrument;
use turbo_rcstr::RcStr;
use turbo_tasks::{ResolvedVc, TryJoinIterExt, Value, ValueToString, Vc};
use turbo_tasks_fs::FileSystemPath;
use turbopack_core::{
    chunk::{
        availability_info::AvailabilityInfo,
        chunk_group::{make_chunk_group, MakeChunkGroupResult},
        module_id_strategies::{DevModuleIdStrategy, ModuleIdStrategy},
        Chunk, ChunkGroupResult, ChunkItem, ChunkableModule, ChunkingConfig, ChunkingConfigs,
        ChunkingContext, EntryChunkGroupResult, EvaluatableAssets, MinifyType, ModuleId,
        SourceMapsType,
    },
    environment::Environment,
    ident::AssetIdent,
    module::Module,
    module_graph::{chunk_group_info::ChunkGroup, ModuleGraph},
    output::{OutputAsset, OutputAssets},
};
use turbopack_ecmascript::chunk::{EcmascriptChunk, EcmascriptChunkType};
use turbopack_ecmascript_runtime::RuntimeType;

use crate::library::ecmascript::chunk::EcmascriptLibraryChunk;

pub struct LibraryChunkingContextBuilder {
    chunking_context: LibraryChunkingContext,
}

impl LibraryChunkingContextBuilder {
    pub fn name(mut self, name: RcStr) -> Self {
        self.chunking_context.name = Some(name);
        self
    }

    pub fn tracing(mut self, enable_tracing: bool) -> Self {
        self.chunking_context.enable_tracing = enable_tracing;
        self
    }

    pub fn asset_base_path(mut self, asset_base_path: ResolvedVc<Option<RcStr>>) -> Self {
        self.chunking_context.asset_base_path = asset_base_path;
        self
    }

    pub fn chunk_base_path(mut self, chunk_base_path: ResolvedVc<Option<RcStr>>) -> Self {
        self.chunking_context.chunk_base_path = chunk_base_path;
        self
    }

    pub fn chunk_suffix_path(mut self, chunk_suffix_path: ResolvedVc<Option<RcStr>>) -> Self {
        self.chunking_context.chunk_suffix_path = chunk_suffix_path;
        self
    }

    pub fn runtime_type(mut self, runtime_type: RuntimeType) -> Self {
        self.chunking_context.runtime_type = runtime_type;
        self
    }

    pub fn minify_type(mut self, minify_type: MinifyType) -> Self {
        self.chunking_context.minify_type = minify_type;
        self
    }

    pub fn source_maps(mut self, source_maps: SourceMapsType) -> Self {
        self.chunking_context.source_maps_type = source_maps;
        self
    }

    pub fn module_id_strategy(
        mut self,
        module_id_strategy: ResolvedVc<Box<dyn ModuleIdStrategy>>,
    ) -> Self {
        self.chunking_context.module_id_strategy = module_id_strategy;
        self
    }

    pub fn build(self) -> Vc<LibraryChunkingContext> {
        LibraryChunkingContext::new(Value::new(self.chunking_context))
    }
}

/// A chunking context for development mode.
///
/// It uses readable filenames and module ids to improve development.
/// It also uses a chunking heuristic that is incremental and cacheable.
/// It splits "node_modules" separately as these are less likely to change
/// during development
#[turbo_tasks::value(serialization = "auto_for_input")]
#[derive(Debug, Clone, Hash)]
pub struct LibraryChunkingContext {
    name: Option<RcStr>,
    /// The root path of the project
    root_path: ResolvedVc<FileSystemPath>,
    /// Whether to write file sources as file:// paths in source maps
    should_use_file_source_map_uris: bool,
    /// This path is used to compute the url to request chunks from
    output_root: ResolvedVc<FileSystemPath>,
    /// The relative path from the output_root to the root_path.
    output_root_to_root_path: ResolvedVc<RcStr>,
    /// This path is used to compute the url to request assets from
    client_root: ResolvedVc<FileSystemPath>,
    /// Chunks are placed at this path
    chunk_root_path: ResolvedVc<FileSystemPath>,
    /// Static assets are placed at this path
    asset_root_path: ResolvedVc<FileSystemPath>,
    /// Base path that will be prepended to all chunk URLs when loading them.
    /// This path will not appear in chunk paths or chunk data.
    chunk_base_path: ResolvedVc<Option<RcStr>>,
    /// Suffix path that will be appended to all chunk URLs when loading them.
    /// This path will not appear in chunk paths or chunk data.
    chunk_suffix_path: ResolvedVc<Option<RcStr>>,
    /// URL prefix that will be prepended to all static asset URLs when loading
    /// them.
    asset_base_path: ResolvedVc<Option<RcStr>>,
    /// Enable tracing for this chunking
    enable_tracing: bool,
    /// The environment chunks will be evaluated in.
    environment: ResolvedVc<Environment>,
    /// The kind of runtime to include in the output.
    runtime_type: RuntimeType,
    /// Whether to minify resulting chunks
    minify_type: MinifyType,
    /// Whether to generate source maps
    source_maps_type: SourceMapsType,
    /// The module id strategy to use
    module_id_strategy: ResolvedVc<Box<dyn ModuleIdStrategy>>,
}

impl LibraryChunkingContext {
    pub fn builder(
        root_path: ResolvedVc<FileSystemPath>,
        output_root: ResolvedVc<FileSystemPath>,
        output_root_to_root_path: ResolvedVc<RcStr>,
        client_root: ResolvedVc<FileSystemPath>,
        chunk_root_path: ResolvedVc<FileSystemPath>,
        asset_root_path: ResolvedVc<FileSystemPath>,
        environment: ResolvedVc<Environment>,
        runtime_type: RuntimeType,
    ) -> LibraryChunkingContextBuilder {
        LibraryChunkingContextBuilder {
            chunking_context: LibraryChunkingContext {
                name: None,
                root_path,
                output_root,
                output_root_to_root_path,
                client_root,
                chunk_root_path,
                should_use_file_source_map_uris: false,
                asset_root_path,
                chunk_base_path: ResolvedVc::cell(None),
                chunk_suffix_path: ResolvedVc::cell(None),
                asset_base_path: ResolvedVc::cell(None),
                enable_tracing: false,
                environment,
                runtime_type,
                minify_type: MinifyType::NoMinify,
                source_maps_type: SourceMapsType::Full,
                module_id_strategy: ResolvedVc::upcast(DevModuleIdStrategy::new_resolved()),
            },
        }
    }
}

impl LibraryChunkingContext {
    /// Returns the kind of runtime to include in output chunks.
    ///
    /// This is defined directly on `LibraryChunkingContext` so it is zero-cost
    /// when `RuntimeType` has a single variant.
    pub fn runtime_type(&self) -> RuntimeType {
        self.runtime_type
    }

    /// Returns the asset base path.
    pub fn chunk_base_path(&self) -> Vc<Option<RcStr>> {
        *self.chunk_base_path
    }

    /// Returns the asset suffix path.
    pub fn chunk_suffix_path(&self) -> Vc<Option<RcStr>> {
        *self.chunk_suffix_path
    }

    /// Returns the minify type.
    pub fn source_maps_type(&self) -> SourceMapsType {
        self.source_maps_type
    }

    /// Returns the minify type.
    pub fn minify_type(&self) -> MinifyType {
        self.minify_type
    }
}

#[turbo_tasks::value_impl]
impl LibraryChunkingContext {
    #[turbo_tasks::function]
    fn new(this: Value<LibraryChunkingContext>) -> Vc<Self> {
        this.into_value().cell()
    }

    #[turbo_tasks::function]
    async fn generate_chunk(
        self: Vc<Self>,
        ident: Vc<AssetIdent>,
        chunk: Vc<Box<dyn Chunk>>,
        evaluatable_assets: Vc<EvaluatableAssets>,
    ) -> Result<Vc<Box<dyn OutputAsset>>> {
        Ok(
            if let Some(ecmascript_chunk) =
                Vc::try_resolve_downcast_type::<EcmascriptChunk>(chunk).await?
            {
                Vc::upcast(EcmascriptLibraryChunk::new(
                    self,
                    ident,
                    ecmascript_chunk,
                    evaluatable_assets,
                ))
            } else if let Some(output_asset) =
                Vc::try_resolve_sidecast::<Box<dyn OutputAsset>>(chunk).await?
            {
                output_asset
            } else {
                bail!("Unable to generate output asset for chunk");
            },
        )
    }
}

#[turbo_tasks::value_impl]
impl ChunkingContext for LibraryChunkingContext {
    #[turbo_tasks::function]
    fn name(&self) -> Vc<RcStr> {
        if let Some(name) = &self.name {
            Vc::cell(name.clone())
        } else {
            Vc::cell("unknown".into())
        }
    }

    #[turbo_tasks::function]
    fn root_path(&self) -> Vc<FileSystemPath> {
        *self.root_path
    }

    #[turbo_tasks::function]
    fn output_root(&self) -> Vc<FileSystemPath> {
        *self.output_root
    }

    #[turbo_tasks::function]
    fn output_root_to_root_path(&self) -> Vc<RcStr> {
        *self.output_root_to_root_path
    }

    #[turbo_tasks::function]
    fn environment(&self) -> Vc<Environment> {
        *self.environment
    }

    #[turbo_tasks::function]
    async fn chunk_path(
        &self,
        ident: Vc<AssetIdent>,
        extension: RcStr,
    ) -> Result<Vc<FileSystemPath>> {
        let root_path = self.chunk_root_path;
        let name = ident
            .output_name(*self.root_path, extension)
            .owned()
            .await?;
        Ok(root_path.join(name))
    }

    #[turbo_tasks::function]
    async fn asset_url(&self, ident: Vc<FileSystemPath>) -> Result<Vc<RcStr>> {
        let asset_path = ident.await?.to_string();
        let asset_path = asset_path
            .strip_prefix(&format!("{}/", self.client_root.await?.path))
            .context("expected asset_path to contain client_root")?;

        Ok(Vc::cell(
            format!(
                "{}{}",
                self.asset_base_path
                    .await?
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or("/"),
                asset_path
            )
            .into(),
        ))
    }

    #[turbo_tasks::function]
    fn reference_chunk_source_maps(&self, _chunk: Vc<Box<dyn OutputAsset>>) -> Vc<bool> {
        Vc::cell(match self.source_maps_type {
            SourceMapsType::Full => true,
            SourceMapsType::None => false,
        })
    }

    #[turbo_tasks::function]
    fn reference_module_source_maps(&self, _module: Vc<Box<dyn Module>>) -> Vc<bool> {
        Vc::cell(match self.source_maps_type {
            SourceMapsType::Full => true,
            SourceMapsType::None => false,
        })
    }

    #[turbo_tasks::function]
    async fn asset_path(
        &self,
        content_hash: RcStr,
        original_asset_ident: Vc<AssetIdent>,
    ) -> Result<Vc<FileSystemPath>> {
        let source_path = original_asset_ident.path().await?;
        let basename = source_path.file_name();
        let asset_path = match source_path.extension_ref() {
            Some(ext) => format!(
                "{basename}.{content_hash}.{ext}",
                basename = &basename[..basename.len() - ext.len() - 1],
                content_hash = &content_hash[..8]
            ),
            None => format!(
                "{basename}.{content_hash}",
                content_hash = &content_hash[..8]
            ),
        };
        Ok(self.asset_root_path.join(asset_path.into()))
    }

    #[turbo_tasks::function]
    async fn chunking_configs(&self) -> Result<Vc<ChunkingConfigs>> {
        let mut map = FxHashMap::default();
        map.insert(
            ResolvedVc::upcast(Vc::<EcmascriptChunkType>::default().to_resolved().await?),
            ChunkingConfig {
                min_chunk_size: usize::MAX,
                max_chunk_count_per_group: 1,
                max_merge_chunk_size: usize::MAX,
                ..Default::default()
            },
        );
        Ok(Vc::cell(map))
    }

    #[turbo_tasks::function]
    fn should_use_file_source_map_uris(&self) -> Vc<bool> {
        Vc::cell(self.should_use_file_source_map_uris)
    }

    #[turbo_tasks::function]
    fn is_tracing_enabled(&self) -> Vc<bool> {
        Vc::cell(self.enable_tracing)
    }

    #[turbo_tasks::function]
    async fn chunk_group(
        self: ResolvedVc<Self>,
        _ident: Vc<AssetIdent>,
        _chunk_group: ChunkGroup,
        _module_graph: Vc<ModuleGraph>,
        _availability_info: Value<AvailabilityInfo>,
    ) -> Result<Vc<ChunkGroupResult>> {
        bail!("Library chunking context does not support chunk groups")
    }

    #[turbo_tasks::function]
    async fn evaluated_chunk_group(
        self: ResolvedVc<Self>,
        ident: Vc<AssetIdent>,
        evaluatable_assets: Vc<EvaluatableAssets>,
        module_graph: Vc<ModuleGraph>,
        availability_info: Value<AvailabilityInfo>,
    ) -> Result<Vc<ChunkGroupResult>> {
        let span = {
            let ident = ident.to_string().await?.to_string();
            tracing::info_span!("chunking", chunking_type = "evaluated", ident = ident)
        };
        async move {
            let availability_info = availability_info.into_value();

            let evaluatable_assets_ref = evaluatable_assets.await?;

            let entries = evaluatable_assets_ref
                .iter()
                .map(|&evaluatable| ResolvedVc::upcast(evaluatable));

            let MakeChunkGroupResult {
                chunks,
                availability_info,
            } = make_chunk_group(
                entries,
                module_graph,
                ResolvedVc::upcast(self),
                availability_info,
            )
            .await?;

            let assets: Vec<ResolvedVc<Box<dyn OutputAsset>>> = chunks
                .iter()
                .map(|chunk| {
                    self.generate_chunk(ident, **chunk, evaluatable_assets)
                        .to_resolved()
                })
                .try_join()
                .await?;

            Ok(ChunkGroupResult {
                assets: ResolvedVc::cell(assets),
                availability_info,
            }
            .cell())
        }
        .instrument(span)
        .await
    }

    #[turbo_tasks::function]
    fn entry_chunk_group(
        self: Vc<Self>,
        _path: Vc<FileSystemPath>,
        _evaluatable_assets: Vc<EvaluatableAssets>,
        _module_graph: Vc<ModuleGraph>,
        _extra_chunks: Vc<OutputAssets>,
        _availability_info: Value<AvailabilityInfo>,
    ) -> Result<Vc<EntryChunkGroupResult>> {
        bail!("Library chunking context does not support entry chunk groups")
    }

    #[turbo_tasks::function]
    fn chunk_item_id_from_ident(&self, ident: Vc<AssetIdent>) -> Vc<ModuleId> {
        self.module_id_strategy.get_module_id(ident)
    }

    #[turbo_tasks::function]
    async fn async_loader_chunk_item(
        self: Vc<Self>,
        _module: Vc<Box<dyn ChunkableModule>>,
        _module_graph: Vc<ModuleGraph>,
        _availability_info: Value<AvailabilityInfo>,
    ) -> Result<Vc<Box<dyn ChunkItem>>> {
        bail!("Library chunking context does not support async loader chunk item")
    }

    #[turbo_tasks::function]
    async fn async_loader_chunk_item_id(
        self: Vc<Self>,
        _module: Vc<Box<dyn ChunkableModule>>,
    ) -> Result<Vc<ModuleId>> {
        bail!("Library chunking context does not support async loader chunk item id")
    }
}
