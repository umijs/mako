use oxc_resolver::Resolution;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct ExternalResource {
    pub source: String,
    pub external: String,
    pub script: Option<String>,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct ResolvedResource(pub SimpleResolution);

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub struct SimpleResolution {
    pub full_path: String,
    pub path: String,
    pub pkg_root: Option<String>,
    pub pkg_json: SimplePackageJSON,
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone, Default)]
pub struct SimplePackageJSON {
    pub name: Option<String>,
    pub version: Option<String>,
    pub side_effects: Option<String>,
    pub directory: Option<String>,
}

impl From<Resolution> for SimpleResolution {
    fn from(oxc_resolution: Resolution) -> Self {
        let full_path = oxc_resolution.full_path().to_string_lossy().to_string();
        let path = oxc_resolution.path().to_string_lossy().to_string();
        let pkg_root = oxc_resolution
            .package_json()
            .map(|desc| desc.directory().to_string_lossy().to_string());

        let simple_pkg_json = oxc_resolution
            .package_json()
            .map(|desc| {
                let value = desc.raw_json();
                let name = value.get("name").map(|v| v.as_str().unwrap().to_string());
                let version = value
                    .get("version")
                    .map(|v| v.as_str().unwrap().to_string());
                let side_effects = value
                    .get("sideEffects")
                    .map(|v| serde_json::to_string(v).unwrap());
                let directory = Some(desc.directory().to_string_lossy().to_string());

                println!("resolve_json_side_effects: {} {:?}", path, side_effects);
                SimplePackageJSON {
                    name,
                    version,
                    side_effects,
                    directory,
                }
            })
            .unwrap_or_default();

        Self {
            full_path,
            path,
            pkg_root,
            pkg_json: simple_pkg_json,
        }
    }
}

#[derive(Archive, Deserialize, Serialize, Debug, Clone)]
pub enum ResolverResource {
    External(ExternalResource),
    Resolved(ResolvedResource),
    Ignored(String),
    Virtual(String),
}

impl ResolverResource {
    pub fn get_resolved_path(&self) -> String {
        match self {
            ResolverResource::External(ExternalResource { source, .. }) => source.to_string(),
            ResolverResource::Resolved(ResolvedResource(resolution)) => {
                resolution.full_path.clone()
            }
            ResolverResource::Ignored(path) => path.clone(),
            ResolverResource::Virtual(path) => path.clone(),
        }
    }
    pub fn get_external(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { external, .. }) => Some(external.clone()),
            ResolverResource::Resolved(_) => None,
            ResolverResource::Ignored(_) => None,
            ResolverResource::Virtual(_) => None,
        }
    }
    pub fn get_script(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { script, .. }) => script.clone(),
            ResolverResource::Resolved(_) => None,
            ResolverResource::Ignored(_) => None,
            ResolverResource::Virtual(_) => None,
        }
    }
}
