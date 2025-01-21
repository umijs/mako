use std::fs;
use std::sync::Arc;

use serde::Serialize;

use super::constants::FEDERATION_EXPOSE_CHUNK_PREFIX;
use super::util::{parse_remote, serialize_none_to_false};
use super::{constants, ModuleFederationPlugin};
use crate::compiler::Context;
use crate::generate::chunk_graph::ChunkGraph;
use crate::module::ModuleId;
use crate::plugin::PluginGenerateEndParams;
use crate::stats::StatsJsonMap;
use crate::utils::get_app_info;

impl ModuleFederationPlugin {
    pub(super) fn generate_federation_manifest(
        &self,
        context: &Arc<Context>,
        params: &PluginGenerateEndParams,
    ) -> Result<(), anyhow::Error> {
        let app_info = get_app_info(&context.root);
        let manifest = Manifest {
            id: self.config.name.clone(),
            name: self.config.name.clone(),
            exposes: self.config.exposes.as_ref().map_or(Vec::new(), |exposes| {
                exposes
                    .iter()
                    .map(|(path, module)| {
                        let name = path.replace("./", "");
                        let remote_module_id: ModuleId = context
                            .root
                            .join(module)
                            .canonicalize()
                            .unwrap()
                            .to_string_lossy()
                            .to_string()
                            .into();
                        // FIXME: this may be slow
                        let chunk_graph = context.chunk_graph.read().unwrap();
                        let sync_chunks = chunk_graph
                            .graph
                            .node_weights()
                            .filter_map(|c| {
                                if c.id.id.starts_with(FEDERATION_EXPOSE_CHUNK_PREFIX)
                                    && c.has_module(&remote_module_id)
                                {
                                    Some(c.id.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<ModuleId>>();

                        let assets = extrac_chunk_assets(sync_chunks, &chunk_graph, params);
                        ManifestExpose {
                            id: format!("{}:{}", self.config.name, name),
                            name,
                            path: path.clone(),
                            assets,
                        }
                    })
                    .collect()
            }),
            shared: {
                let chunk_graph = context.chunk_graph.read().unwrap();
                let provide_shared_map = self.provide_shared_map.read().unwrap();
                provide_shared_map
                    .iter()
                    .map(|(_, config)| {
                        let module_id: ModuleId = config.file_path.clone().into();
                        let chunk_id = chunk_graph
                            .get_chunk_for_module(&module_id)
                            .as_ref()
                            .unwrap()
                            .id
                            .clone();
                        let assets = extrac_chunk_assets(vec![chunk_id], &chunk_graph, params);
                        ManifestShared {
                            id: format!("{}:{}", self.config.name, config.share_key),
                            name: config.share_key.clone(),
                            require_version: config.shared_config.required_version.clone(),
                            version: config.version.clone(),
                            singleton: config.shared_config.singleton,
                            assets,
                        }
                    })
                    .collect()
            },
            remotes: params
                .stats
                .chunk_modules
                .iter()
                .filter_map(|cm| {
                    if cm
                        .id
                        .starts_with(constants::FEDERATION_REMOTE_MODULE_PREFIX)
                    {
                        let data = cm.id.split('/').collect::<Vec<&str>>();
                        Some(ManifestRemote {
                            entry: parse_remote(
                                self.config.remotes.as_ref().unwrap().get(data[3]).unwrap(),
                            )
                            .unwrap()
                            .1,
                            module_name: data[4].to_string(),
                            alias: data[3].to_string(),
                            federation_container_name: data[3].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
            meta_data: ManifestMetaData {
                name: self.config.name.clone(),
                build_info: ManifestMetaBuildInfo {
                    build_name: app_info.0.unwrap_or("default".to_string()),
                    build_version: app_info.1.unwrap_or("".to_string()),
                },
                global_name: self.config.name.clone(),
                public_path: "auto".to_string(),
                r#type: "global".to_string(),
                remote_entry: self.config.exposes.as_ref().and_then(|exposes| {
                    if exposes.is_empty() {
                        None
                    } else {
                        Some(ManifestMetaRemoteEntry {
                            name: format!("{}.js", self.config.name),
                            path: "".to_string(),
                            r#type: "global".to_string(),
                        })
                    }
                }),
                ..Default::default()
            },
        };
        fs::write(
            context.root.join("./dist/mf-manifest.json"),
            serde_json::to_string_pretty(&manifest)?,
        )
        .unwrap();
        Ok(())
    }
}

fn extrac_chunk_assets(
    sync_chunks: Vec<ModuleId>,
    chunk_graph: &ChunkGraph,
    params: &PluginGenerateEndParams,
) -> ManifestAssets {
    let sync_chunk_dependencies = sync_chunks.iter().fold(Vec::new(), |mut acc, cur| {
        let sync_deps = chunk_graph.sync_dependencies_chunk(cur);
        acc.splice(..0, sync_deps);
        acc
    });

    let all_sync_chunks = [sync_chunk_dependencies, sync_chunks].concat();
    let all_async_chunks: Vec<ModuleId> = all_sync_chunks.iter().fold(vec![], |mut acc, cur| {
        acc.extend(chunk_graph.installable_descendants_chunk(cur));
        acc
    });

    let (sync_js_files, sync_css_files) = extract_assets(all_sync_chunks, &params.stats);

    let (async_js_files, async_css_files) = extract_assets(all_async_chunks, &params.stats);

    let async_js_files = async_js_files
        .into_iter()
        .filter(|f| !sync_js_files.contains(f))
        .collect();

    let async_css_files = async_css_files
        .into_iter()
        .filter(|f| !sync_js_files.contains(f))
        .collect();

    ManifestAssets {
        js: ManifestAssetsItem {
            sync: sync_js_files,
            r#async: async_js_files,
        },
        css: ManifestAssetsItem {
            sync: sync_css_files,
            r#async: async_css_files,
        },
    }
}

fn extract_assets(
    all_exposes_sync_chunks: Vec<ModuleId>,
    stats: &StatsJsonMap,
) -> (Vec<String>, Vec<String>) {
    all_exposes_sync_chunks.iter().fold(
        (Vec::<String>::new(), Vec::<String>::new()),
        |mut acc, cur| {
            if let Some(c) = stats.chunks.iter().find(|c| c.id == cur.id) {
                c.files.iter().for_each(|f| {
                    if f.ends_with(".js") {
                        acc.0.push(f.clone());
                    }
                    if f.ends_with(".css") {
                        acc.1.push(f.clone());
                    }
                });
            }
            acc
        },
    )
}

#[derive(Serialize, Default)]
pub struct ManifestAssetsItem {
    pub sync: Vec<String>,
    pub r#async: Vec<String>,
}

#[derive(Serialize, Default)]
pub struct ManifestAssets {
    pub js: ManifestAssetsItem,
    pub css: ManifestAssetsItem,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestExpose {
    pub id: String,
    pub name: String,
    pub assets: ManifestAssets,
    pub path: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestShared {
    id: String,
    name: String,
    assets: ManifestAssets,
    version: String,
    #[serde(serialize_with = "serialize_none_to_false")]
    require_version: Option<String>,
    singleton: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestRemote {
    pub entry: String,
    pub alias: String,
    pub module_name: String,
    pub federation_container_name: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMetaTypes {
    path: String,
    name: String,
    zip: String,
    api: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMetaData {
    pub remote_entry: Option<ManifestMetaRemoteEntry>,
    pub global_name: String,
    pub public_path: String,
    pub r#type: String,
    pub build_info: ManifestMetaBuildInfo,
    pub name: String,
    pub types: ManifestMetaTypes,
    pub plugin_version: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMetaBuildInfo {
    pub build_version: String,
    pub build_name: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ManifestMetaRemoteEntry {
    pub name: String,
    pub path: String,
    pub r#type: String,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub meta_data: ManifestMetaData,
    pub shared: Vec<ManifestShared>,
    pub remotes: Vec<ManifestRemote>,
    pub exposes: Vec<ManifestExpose>,
}
