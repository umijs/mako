use indexmap::IndexSet;

use crate::share::helpers::SWC_HELPERS;

pub struct SwcHelpers {
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
        SWC_HELPERS.iter().for_each(|h| {
            helpers.insert(h.to_string());
        });
        helpers
    }
}

impl Default for SwcHelpers {
    fn default() -> Self {
        Self::new(None)
    }
}
