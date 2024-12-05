use serde::Serialize;

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
    require_version: String,
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
