use indexmap::IndexSet;

pub struct SwcHelpers {
    #[allow(dead_code)]
    pub helpers: IndexSet<String>,
}

impl SwcHelpers {
    pub fn new(helpers: Option<IndexSet<String>>) -> Self {
        let helpers = if let Some(helpers) = helpers {
            helpers
        } else {
            IndexSet::new()
        };
        Self { helpers }
    }

    pub fn full_helpers() -> IndexSet<String> {
        let mut helpers = IndexSet::new();
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
