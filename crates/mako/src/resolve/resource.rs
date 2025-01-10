use std::path::PathBuf;

use crate::build::analyze_deps::AnalyzeDepsResult;
use crate::resolve::Resolution;

#[derive(Debug, Clone)]
pub struct RemoteInfo {
    pub module_id: String,
    pub external_reference_id: String,
    pub external_type: String,
    pub sub_path: String,
    pub name: String,
    pub share_scope: String,
}

#[derive(Debug, Clone)]
pub struct ConsumeSharedInfo {
    pub module_id: String,
    pub name: String,
    pub share_scope: String,
    pub version: String,
    pub full_path: String,
    pub eager: bool,
    pub required_version: Option<String>,
    pub strict_version: bool,
    pub singletion: bool,
    pub deps: AnalyzeDepsResult,
}

#[derive(Debug, Clone)]
pub struct ExternalResource {
    pub source: String,
    pub external: String,
    pub script: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedResource(pub Resolution);

#[derive(Debug, Clone)]
pub enum ResolverResource {
    External(ExternalResource),
    Resolved(ResolvedResource),
    Ignored(PathBuf),
    Virtual(PathBuf),
    Remote(RemoteInfo),
    Shared(ConsumeSharedInfo),
}

impl ResolverResource {
    pub fn get_resolved_path(&self) -> String {
        match self {
            ResolverResource::External(ExternalResource { source, .. }) => source.to_string(),
            ResolverResource::Resolved(ResolvedResource(resolution)) => {
                resolution.full_path().to_string_lossy().to_string()
            }
            ResolverResource::Ignored(path) => path.to_string_lossy().to_string(),
            ResolverResource::Virtual(path) => path.to_string_lossy().to_string(),
            ResolverResource::Remote(info) => info.module_id.to_string(),
            ResolverResource::Shared(info) => info.full_path.clone(),
        }
    }
    pub fn get_external(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { external, .. }) => Some(external.clone()),
            _ => None,
        }
    }
    pub fn get_script(&self) -> Option<String> {
        match self {
            ResolverResource::External(ExternalResource { script, .. }) => script.clone(),
            _ => None,
        }
    }

    pub fn get_remote_info(&self) -> Option<&RemoteInfo> {
        match &self {
            ResolverResource::Remote(remote_info) => Some(remote_info),
            _ => None,
        }
    }

    pub fn get_pkg_info(&self) -> Option<PkgInfo> {
        match self {
            ResolverResource::Resolved(ResolvedResource(resolution)) => Some(PkgInfo {
                file_path: resolution.full_path().to_string_lossy().to_string(),
                name: resolution.package_json().and_then(|p| {
                    p.raw_json()
                        .get("name")
                        .and_then(|v| v.as_str().map(|v| v.to_string()))
                }),
                version: resolution.package_json().and_then(|p| {
                    p.raw_json()
                        .get("version")
                        .and_then(|v| v.as_str().map(|v| v.to_string()))
                }),
            }),
            ResolverResource::Shared(info) => {
                info.deps.resolved_deps[0].resolver_resource.get_pkg_info()
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct PkgInfo {
    pub file_path: String,
    pub name: Option<String>,
    pub version: Option<String>,
}
