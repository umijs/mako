use anyhow::Result;
use qstring::QString;
use serde::Serialize;
use turbo_rcstr::RcStr;
use turbo_tasks::{FxIndexMap, FxIndexSet, ResolvedVc, TryJoinIterExt, Vc};
use turbopack::css::chunk::CssChunk;
use turbopack_browser::ecmascript::{EcmascriptBrowserChunk, EcmascriptBrowserEvaluateChunk};
use turbopack_core::{
    chunk::{Chunk, ChunkItem, ChunkableModule},
    output::OutputAsset,
};

pub async fn generate_webpack_stats<'a, I>(output_assets: I) -> Result<WebpackStats>
where
    I: IntoIterator<Item = &'a ResolvedVc<Box<dyn OutputAsset>>>,
{
    let mut assets = vec![];
    let mut chunks = vec![];
    let mut chunk_items: FxIndexMap<Vc<Box<dyn ChunkItem>>, FxIndexSet<RcStr>> =
        FxIndexMap::default();
    let mut modules = vec![];
    let mut entrypoints = FxIndexMap::default();
    for asset in output_assets {
        let asset_len = asset.size_bytes().await?.unwrap_or_default();

        if let Some(chunk) = ResolvedVc::try_downcast_type::<EcmascriptBrowserEvaluateChunk>(*asset)
        {
            let entry_path = chunk.path().await?.path.clone();
            chunks.push(WebpackStatsChunk {
                size: asset_len,
                files: vec![entry_path.clone()],
                id: entry_path.clone(),
                ..Default::default()
            });

            chunk
                .evaluatable_assets()
                .await?
                .iter()
                .for_each(|evaluatable_asset| {
                    let item = evaluatable_asset
                        .as_chunk_item(chunk.module_graph(), chunk.chunking_context());
                    chunk_items
                        .entry(item)
                        .or_default()
                        .insert(entry_path.clone());
                });

            let entry_referenced_assets = (*chunk).chunks_data().await?;
            let mut entry_chunks = entry_referenced_assets
                .iter()
                .map(async |asset| Ok(asset.await?.path.as_str().into()))
                .try_join()
                .await?;
            entry_chunks.push(entry_path.clone());

            let mut entry_assets = entry_referenced_assets
                .iter()
                .map(|asset| async move {
                    Ok(WebpackStatsEntrypointAssets {
                        name: asset.await?.path.as_str().into(),
                    })
                })
                .try_join()
                .await?;
            entry_assets.push(WebpackStatsEntrypointAssets {
                name: entry_path.clone(),
            });

            let entry_name: RcStr = QString::from((*chunk).ident().query().await?.as_str())
                .get("name")
                .unwrap_or(remove_extension_from_str(entry_path.as_str()))
                .into();

            entrypoints.insert(
                entry_name.clone(),
                WebpackStatsEntrypoint {
                    name: entry_name.clone(),
                    chunks: entry_chunks,
                    assets: entry_assets,
                },
            );
        }

        if let Some(chunk) = ResolvedVc::try_downcast_type::<EcmascriptBrowserChunk>(*asset) {
            let chunk_ident = &chunk.path().await?.path;
            chunks.push(WebpackStatsChunk {
                size: asset_len,
                files: vec![chunk_ident.clone()],
                id: chunk_ident.clone(),
                ..Default::default()
            });

            for item in chunk.chunk().chunk_items().await? {
                chunk_items
                    .entry(**item)
                    .or_default()
                    .insert(chunk_ident.clone());
            }
        }

        if let Some(chunk) = ResolvedVc::try_downcast_type::<CssChunk>(*asset) {
            let chunk_ident = &chunk.path().await?.path;
            chunks.push(WebpackStatsChunk {
                size: asset_len,
                files: vec![chunk_ident.clone()],
                id: chunk_ident.clone(),
                ..Default::default()
            });
        }

        let path = &asset.path().await?.path;
        assets.push(WebpackStatsAsset {
            ty: "asset".into(),
            name: path.clone(),
            chunks: vec![path.clone()],
            size: asset_len,
            ..Default::default()
        });
    }

    for (chunk_item, chunks) in chunk_items {
        let size = *chunk_item.content_ident().path().read().len().await?;
        let path = chunk_item.asset_ident().path().await?.path.clone();
        modules.push(WebpackStatsModule {
            name: path.clone(),
            id: path.clone(),
            chunks: chunks.into_iter().collect(),
            size,
        });
    }

    Ok(WebpackStats {
        assets,
        entrypoints,
        chunks,
        modules,
    })
}

fn remove_extension_from_str(filename: &str) -> &str {
    if let Some(dot_index) = filename.rfind('.') {
        if dot_index > 0 {
            return &filename[..dot_index];
        }
    }
    filename
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsAssetInfo {}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsAsset {
    #[serde(rename = "type")]
    pub ty: RcStr,
    pub name: RcStr,
    pub info: WebpackStatsAssetInfo,
    pub size: u64,
    pub emitted: bool,
    pub compared_for_emit: bool,
    pub cached: bool,
    pub chunks: Vec<RcStr>,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsChunk {
    pub rendered: bool,
    pub initial: bool,
    pub entry: bool,
    pub recorded: bool,
    pub id: RcStr,
    pub size: u64,
    pub hash: RcStr,
    pub files: Vec<RcStr>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsModule {
    pub name: RcStr,
    pub id: RcStr,
    pub chunks: Vec<RcStr>,
    pub size: Option<u64>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsEntrypointAssets {
    pub name: RcStr,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStatsEntrypoint {
    pub name: RcStr,
    pub chunks: Vec<RcStr>,
    pub assets: Vec<WebpackStatsEntrypointAssets>,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WebpackStats {
    pub assets: Vec<WebpackStatsAsset>,
    pub entrypoints: FxIndexMap<RcStr, WebpackStatsEntrypoint>,
    pub chunks: Vec<WebpackStatsChunk>,
    pub modules: Vec<WebpackStatsModule>,
}
