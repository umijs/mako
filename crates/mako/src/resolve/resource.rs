use std::path::PathBuf;

use crate::resolve::Resolution;

#[derive(Debug, Clone)]
pub struct RemoteInfo {
    pub module_id: String,
    pub external_refenrence_id: String,
    pub external_type: String,
    pub sub_path: String,
    pub name: String,
    pub share_scope: String,
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
}
