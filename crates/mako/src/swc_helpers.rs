use std::collections::HashSet;

pub struct SwcHelpers {
    pub helpers: HashSet<String>,
}

impl SwcHelpers {
    pub fn new(helpers: Option<HashSet<String>>) -> Self {
        let helpers = if let Some(helpers) = helpers {
            helpers
        } else {
            HashSet::new()
        };
        Self { helpers }
    }

    pub fn full_helpers() -> HashSet<String> {
        let mut helpers = HashSet::new();
        helpers.insert("@swc/helpers/_/_interop_require_default".into());
        helpers.insert("@swc/helpers/_/_interop_require_wildcard".into());
        helpers.insert("@swc/helpers/_/_export_star".into());
        helpers
    }
}

impl Default for SwcHelpers {
    fn default() -> Self {
        Self::new(None)
    }
}
