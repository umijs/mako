use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use mako_core::lazy_static::lazy_static;
use mako_core::swc_ecma_ast::Module;
use swc_core::ecma::visit::VisitWith;

use crate::compiler::Context;
use crate::config::ModuleIdStrategy;
use crate::transformers::transform_interop_probe::InteropProbe;

pub struct SwcHelpers {
    pub helpers: HashSet<String>,
}

lazy_static! {
    static ref HAHSED_HELPERS: HashMap<String, String> = [
        (
            "d3__vuQ2".to_string(),
            "@swc/helpers/_/_interop_require_default".to_string(),
        ),
        (
            "hSu6qSb4".to_string(),
            "@swc/helpers/_/_interop_require_wildcard".to_string(),
        ),
        (
            "0XUdfEQ8".to_string(),
            "@swc/helpers/_/_export_star".to_string(),
        ),
    ]
    .iter()
    .cloned()
    .collect();
}

lazy_static! {
    static ref RAW_HELPERS: HashMap<String, String> = [
        (
            "@swc/helpers/_/_interop_require_default".to_string(),
            "@swc/helpers/_/_interop_require_default".to_string(),
        ),
        (
            "@swc/helpers/_/_interop_require_wildcard".to_string(),
            "@swc/helpers/_/_interop_require_wildcard".to_string(),
        ),
        (
            "@swc/helpers/_/_export_star".to_string(),
            "@swc/helpers/_/_export_star".to_string(),
        ),
    ]
    .into_iter()
    .collect();
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

    pub fn extends(&mut self, helpers: HashSet<String>) {
        self.helpers.extend(helpers);
    }

    pub fn get_helpers(&self) -> Vec<String> {
        self.helpers.iter().map(|h| h.to_string()).collect()
    }

    // for watch mode
    pub fn full_helpers() -> HashSet<String> {
        RAW_HELPERS.keys().cloned().collect()
    }

    pub fn get_swc_helpers(ast: &Module, context: &Arc<Context>) -> HashSet<String> {
        let is_hashed = matches!(context.config.module_id_strategy, ModuleIdStrategy::Hashed);
        let needles: &HashMap<String, String> = if is_hashed {
            &HAHSED_HELPERS
        } else {
            &RAW_HELPERS
        };

        let mut probe = InteropProbe::new(needles, 1);
        ast.visit_with(&mut probe);

        probe.probed
    }
}

impl Default for SwcHelpers {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ModuleIdStrategy};
    use crate::module::ModuleId;

    #[test]
    fn ensure_hash_consistent() {
        let config = Config {
            module_id_strategy: ModuleIdStrategy::Hashed,
            ..Default::default()
        };

        let context = Arc::new(Context {
            config,
            ..Default::default()
        });

        for (k, v) in HAHSED_HELPERS.iter() {
            assert_eq!(*k, ModuleId::from(v.as_str()).generate(&context));
        }
    }
}
