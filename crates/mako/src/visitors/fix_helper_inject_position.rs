use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_ecma_ast::{
    Decl, ExportDecl, ExportNamedSpecifier, ExportSpecifier, FnDecl, Function, ModuleDecl,
    ModuleExportName, ModuleItem, NamedExport, Pat, Stmt,
};
use mako_core::swc_ecma_utils::quote_ident;
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};

pub struct FixHelperInjectPosition {
    pub exports: Vec<String>,
}

impl FixHelperInjectPosition {
    pub fn new() -> Self {
        Self { exports: vec![] }
    }
}

impl VisitMut for FixHelperInjectPosition {
    fn visit_mut_module_item(&mut self, n: &mut ModuleItem) {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl, .. })) = n {
            let mut has_array_param = false;
            let mut export_name = "".to_string();
            if let Decl::Fn(FnDecl {
                ident,
                function: box Function { params, .. },
                ..
            }) = decl
            {
                has_array_param = params
                    .iter()
                    .any(|param| matches!(param.pat, Pat::Array(_)));
                if has_array_param {
                    export_name = ident.sym.to_string();
                }
            }
            if has_array_param {
                *n = ModuleItem::Stmt(Stmt::Decl(decl.clone()));
                self.exports.push(export_name);
            }
        }
        n.visit_mut_children_with(self);
    }

    fn visit_mut_module_items(&mut self, n: &mut Vec<ModuleItem>) {
        n.visit_mut_children_with(self);

        if !self.exports.is_empty() {
            let mut new_items = vec![];
            for export in self.exports.iter() {
                new_items.push(ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(
                    NamedExport {
                        span: DUMMY_SP,
                        specifiers: vec![ExportSpecifier::Named(ExportNamedSpecifier {
                            span: DUMMY_SP,
                            orig: ModuleExportName::Ident(quote_ident!(export.clone())),
                            exported: None,
                            is_type_only: false,
                        })],
                        src: None,
                        type_only: false,
                        with: None,
                    },
                )));
            }
            n.extend(new_items);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use mako_core::swc_ecma_preset_env::{self as swc_preset_env};
    use mako_core::swc_ecma_transforms::feature::FeatureFlag;
    use mako_core::swc_ecma_transforms::Assumptions;
    use mako_core::swc_ecma_transforms_proposals::decorators;
    use mako_core::swc_ecma_visit::{Fold, VisitMut, VisitMutWith};
    use swc_core::common::GLOBALS;

    use super::FixHelperInjectPosition;
    use crate::ast::file::File;
    use crate::ast::tests::TestUtils;
    use crate::build::targets::swc_preset_env_targets_from_map;

    #[test]
    fn test_normal() {
        let code = run(r#"
export function foo([ a, b ]) {
    return a + b;
}
                "#);
        assert_eq!(
            code,
            r#"
function foo(ref) {
    let _ref = _sliced_to_array(ref, 2), a = _ref[0], b = _ref[1];
    return a + b;
}
export { foo };
        "#
            .trim()
        );
    }

    fn run(js_code: &str) -> String {
        let mut test_utils = TestUtils::gen_js_ast(js_code.to_string());
        let ast = test_utils.ast.js_mut();
        let unresolved_mark = ast.unresolved_mark;
        GLOBALS.set(&test_utils.context.meta.script.globals, || {
            let mut v = FixHelperInjectPosition { exports: vec![] };
            ast.ast.visit_mut_with(&mut v);

            // preset_env
            let mut folders: Vec<Box<dyn Fold>> = vec![];
            folders.push(Box::new(decorators(decorators::Config {
                legacy: true,
                emit_metadata: false,
                ..Default::default()
            })));
            let origin_comments = test_utils
                .context
                .meta
                .script
                .origin_comments
                .read()
                .unwrap();
            let comments = origin_comments.get_swc_comments().clone();
            let mut targets = HashMap::new();
            targets.insert("chrome".to_string(), 50.0);
            folders.push(Box::new(swc_preset_env::preset_env(
                unresolved_mark,
                Some(comments),
                swc_preset_env::Config {
                    mode: Some(swc_preset_env::Mode::Entry),
                    targets: Some(swc_preset_env_targets_from_map(targets)),
                    ..Default::default()
                },
                Assumptions::default(),
                &mut FeatureFlag::default(),
            )));
            let mut visitors: Vec<Box<dyn VisitMut>> = vec![];
            let context = test_utils.context.clone();
            let file = File::new("test.ts".to_string(), context.clone());
            ast.transform(&mut visitors, &mut folders, &file, false, context)
                .unwrap();
        });
        test_utils.js_ast_to_code()
    }
}
