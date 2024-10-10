use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use oxc_resolver::PackageJson;

#[derive(Clone)]
pub struct Resolution {
    pub path: PathBuf,
    pub query: Option<String>,
    pub fragment: Option<String>,
    pub package_json: Option<Arc<PackageJson>>,
}

impl Resolution {
    /// Returns the path without query and fragment
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the path without query and fragment
    pub fn into_path_buf(self) -> PathBuf {
        self.path
    }

    /// Returns the path query `?query`, contains the leading `?`
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Returns the path fragment `#fragment`, contains the leading `#`
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }

    /// Returns serialized package_json
    pub fn package_json(&self) -> Option<&Arc<PackageJson>> {
        self.package_json.as_ref()
    }

    /// Returns the full path with query and fragment
    pub fn full_path(&self) -> PathBuf {
        let mut path = self.path.clone().into_os_string();
        if let Some(query) = &self.query {
            path.push(query);
        }
        if let Some(fragment) = &self.fragment {
            path.push(fragment);
        }
        PathBuf::from(path)
    }
}

impl fmt::Debug for Resolution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Resolution")
            .field("path", &self.path)
            .field("query", &self.query)
            .field("fragment", &self.fragment)
            .field("package_json", &self.package_json.as_ref().map(|p| &p.path))
            .finish()
    }
}
