use std::path::PathBuf;

use oxc_resolver::Resolution;

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
}

impl ResolverResource {
    pub fn get_resolved_path(&self) -> String {
        match self {
            ResolverResource::External(ExternalResource { source, .. }) => source.to_string(),
            ResolverResource::Resolved(ResolvedResource(resolution)) => {
                resolution.full_path().to_string_lossy().to_string()
            }
            ResolverResource::Ignored(path) => path.to_string_lossy().to_string(),
        }
    }
}
