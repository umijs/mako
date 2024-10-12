use std::sync::Arc;

use swc_core::common::Mark;
use swc_core::ecma::ast::Module;
use swc_core::ecma::transforms::base::feature::FeatureFlag;
use swc_core::ecma::transforms::module::common_js as swc_common_js;
use swc_core::ecma::transforms::module::util::{Config, ImportInterop};
use swc_core::ecma::utils::IsDirective;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::ast::utils;
use crate::compiler::Context;

pub struct Commonjs {
    context: Arc<Context>,
    unresolved_mark: Mark,
    import_interop: ImportInterop,
}

impl VisitMut for Commonjs {
    fn visit_mut_module(&mut self, n: &mut Module) {
        let use_strict = if utils::is_esm(n) {
            true
        } else {
            n.body
                .iter()
                .take_while(|s| s.directive_continue())
                .any(|s| s.is_use_strict())
        };

        n.visit_mut_with(&mut swc_common_js(
            self.unresolved_mark,
            Config {
                import_interop: Some(self.import_interop),
                // NOTE: 这里后面要调整为注入自定义require
                ignore_dynamic: true,
                preserve_import_meta: true,
                // TODO: set to false when esm
                allow_top_level_this: true,
                strict_mode: use_strict,
                ..Default::default()
            },
            FeatureFlag::empty(),
            Some(
                self.context
                    .meta
                    .script
                    .origin_comments
                    .read()
                    .unwrap()
                    .get_swc_comments(),
            ),
        ));
    }
}

pub fn common_js(
    context: Arc<Context>,
    unresolved_mark: Mark,
    import_interop: ImportInterop,
) -> impl VisitMut {
    Commonjs {
        unresolved_mark,
        import_interop,
        context,
    }
}
