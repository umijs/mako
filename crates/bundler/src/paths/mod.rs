use serde::{Deserialize, Serialize};
use turbo_tasks::{trace::TraceRawVcs, NonLocalValue};

/// A reference to a server file with content hash for change detection
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TraceRawVcs, NonLocalValue)]
pub struct ServerPath {
    /// Relative to the root_path
    pub path: String,
    pub content_hash: u64,
}

/// A list of server paths
#[turbo_tasks::value(transparent)]
pub struct ServerPaths(Vec<ServerPath>);
