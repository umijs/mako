use anyhow::{bail, Result};
use turbo_tasks::{ReadRef, Vc};
use turbopack_core::source_map::{OptionSourceMap, OptionStringifiedSourceMap, SourceMap};
use url::Url;

use crate::project::ProjectContainer;

pub async fn get_source_map_rope(
    container: Vc<ProjectContainer>,
    file_path: String,
) -> Result<Option<Vc<OptionStringifiedSourceMap>>> {
    let (file, module) = match Url::parse(&file_path) {
        Ok(url) => match url.scheme() {
            "file" => {
                let path = urlencoding::decode(url.path())?.to_string();
                let module = url.query_pairs().find(|(k, _)| k == "id");
                (
                    path,
                    match module {
                        Some(module) => Some(urlencoding::decode(&module.1)?.into_owned().into()),
                        None => None,
                    },
                )
            }
            _ => bail!("Unknown url scheme"),
        },
        Err(_) => (file_path.to_string(), None),
    };

    let Some(chunk_base) = file.strip_prefix(
        &(format!(
            "{}/{}/",
            container.project().await?.project_path,
            container.project().dist_dir().await?
        )),
    ) else {
        // File doesn't exist within the dist dir
        return Ok(None);
    };

    let server_path = container.project().node_root().join(chunk_base.into());

    let client_path = container.project().client_root().join(chunk_base.into());

    let mut map = container.get_source_map(server_path, module.clone());

    if map.await?.is_none() {
        // If the chunk doesn't exist as a server chunk, try a client chunk.
        // TODO: Properly tag all server chunks and use the `isServer` query param.
        // Currently, this is inaccurate as it does not cover RSC server
        // chunks.
        map = container.get_source_map(client_path, module);
    }

    if map.await?.is_none() {
        bail!("chunk/module is missing a sourcemap");
    }

    Ok(Some(map))
}

pub async fn get_source_map(
    container: Vc<ProjectContainer>,
    file_path: String,
) -> Result<Option<ReadRef<OptionSourceMap>>> {
    let Some(map) = get_source_map_rope(container, file_path).await? else {
        return Ok(None);
    };
    let map = SourceMap::new_from_rope_cached(map).await?;
    Ok(Some(map))
}
