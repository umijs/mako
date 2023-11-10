use std::sync::Arc;

use mako_core::swc_ecma_visit::{as_folder, Fold, VisitMut};

use crate::compiler::Context;

pub fn optimize_package_imports(path: String, context: Arc<Context>) -> impl Fold + VisitMut {
    as_folder(OptimizePackageImports { path, context })
}

struct OptimizePackageImports {
    path: String,
    context: Arc<Context>,
}

impl VisitMut for OptimizePackageImports {}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use mako_core::swc_common::{chain, Mark};
    use mako_core::swc_ecma_parser::{EsConfig, Syntax};
    use mako_core::swc_ecma_transforms::resolver;
    use swc_ecma_transforms_testing::test_fixture;
    use testing::fixture;

    use super::optimize_package_imports;
    use crate::compiler::{Compiler, Context};
    use crate::config::Config;

    #[fixture("test/fixture/optimize_package_imports/**/input.js")]
    fn optimize_package_imports_fixture(input: PathBuf) {
        let output = input.parent().unwrap().join("output.js");
        test_fixture(
            self::syntax(),
            &|_tr| {
                let unresolved_mark = Mark::new();
                let top_level_mark = Mark::new();

                chain!(
                    resolver(unresolved_mark, top_level_mark, false),
                    optimize_package_imports(
                        input.to_string_lossy().to_string(),
                        self::context(&input)
                    ),
                )
            },
            &input,
            &output,
            Default::default(),
        );
    }

    fn syntax() -> Syntax {
        Syntax::Es(EsConfig {
            jsx: true,
            ..Default::default()
        })
    }

    fn context(input: &PathBuf) -> Arc<Context> {
        let root = input.parent().unwrap().to_path_buf();
        let config = Config::new(&root, None, None).unwrap();
        let compiler = Compiler::new(config, root.clone(), Default::default()).unwrap();
        compiler.context
    }
}
