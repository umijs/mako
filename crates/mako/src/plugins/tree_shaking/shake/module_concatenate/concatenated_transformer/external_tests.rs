use std::sync::Arc;

use maplit::{hashmap, hashset};
use swc_core::common::GLOBALS;
use swc_core::ecma::visit::VisitMutWith;

use super::super::{ConcatenateContext, ConcatenatedTransform};
use super::utils::describe_export_map;
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::{Config, Mode, OptimizationConfig};
use crate::module::ModuleId;

#[test]
fn test_import_default_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("import v from 'external';v", &mut ccn_ctx),
        "external_esm.default;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("import { foo } from 'external';foo", &mut ccn_ctx),
        "external_esm.foo;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_as_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("import { foo as bar } from 'external';bar", &mut ccn_ctx),
        "external_esm.foo;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_namespace_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("import * as n from 'external';n", &mut ccn_ctx),
        "external_esm;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_export_default_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("export { default } from 'external'", &mut ccn_ctx),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(
        describe_export_map(&ccn_ctx),
        "default => external_esm.default"
    );
}

#[test]
fn test_export_default_as_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("export { default as foo } from 'external'", &mut ccn_ctx),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "foo => external_esm.default");
}

#[test]
fn test_export_named_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("export { named } from 'external'", &mut ccn_ctx),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "named => external_esm.named");
}

#[test]
fn test_export_named_as_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("export { named as foo } from 'external'", &mut ccn_ctx),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "foo => external_esm.named");
}

#[test]
fn test_export_named_as_default_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test("export { named as default } from 'external'", &mut ccn_ctx),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(
        describe_export_map(&ccn_ctx),
        "default => external_esm.named"
    );
}

#[test]
fn test_export_namespace_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(run_test("export * as ns from 'external'", &mut ccn_ctx), "");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "ns => external_esm");
}

#[ignore = "not allowed export star in inner module"]
#[test]
fn test_export_star_from_external() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(run_test("export * from 'external'", &mut ccn_ctx), "");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "ns => external_esm");
}

#[test]
fn test_import_default_from_external_then_export_default() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"import v from "external"; export { v as default}; v"#,
            &mut ccn_ctx
        ),
        "external_esm.default;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(
        describe_export_map(&ccn_ctx),
        "default => external_esm.default"
    );
}

#[test]
fn test_import_named_from_external_then_export_named() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"import {named} from "external"; export { named }; named"#,
            &mut ccn_ctx
        ),
        "external_esm.named;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "named => external_esm.named");
}

#[test]
fn test_import_named_from_external_then_export_named_as() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"import {named} from "external"; export { named as foo }; named"#,
            &mut ccn_ctx
        ),
        "external_esm.named;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "foo => external_esm.named");
}

#[test]
fn test_import_namespace_from_external_then_export_named() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"import * as ns from "external"; export { ns }; ns"#,
            &mut ccn_ctx
        ),
        "external_esm;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "ns => external_esm");
}

#[test]
fn test_import_all_in_one() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"import x, { imported as named, named_2 } from "external";x;named;named_2;"#,
            &mut ccn_ctx
        ),
        "external_esm.default;external_esm.imported;external_esm.named_2;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(describe_export_map(&ccn_ctx), "");
}

#[test]
fn test_export_all_in_one() {
    let mut ccn_ctx = fixture_concatenate_context();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    assert_eq!(
        run_test(
            r#"export {default as foo, imported as named, named_2 } from "external""#,
            &mut ccn_ctx
        ),
        ""
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);

    assert_eq!(
        describe_export_map(&ccn_ctx),
        r#"
foo => external_esm.default
named => external_esm.imported
named_2 => external_esm.named_2"#
            .trim()
    );
}

fn run_test(code: &str, ccn_ctx: &mut ConcatenateContext) -> String {
    let context = Arc::new(Context {
        config: Config {
            devtool: None,
            optimization: Some(OptimizationConfig {
                concatenate_modules: Some(true),
                skip_modules: Some(true),
            }),
            mode: Mode::Production,
            minify: true,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut ast = JsAst::build("mut.js", code, context.clone()).unwrap();

    let current_module_id = ModuleId::from("mut.js");
    let module_map = hashmap! {
       "external".to_string() => ModuleId::from("external")
    };

    GLOBALS.set(&context.meta.script.globals, || {
        let mut t = ConcatenatedTransform::new(
            ccn_ctx,
            &current_module_id,
            &module_map,
            &context,
            ast.top_level_mark,
        );

        ast.ast.visit_mut_with(&mut t);
    });

    ast.generate(context.clone())
        .unwrap()
        .code
        .trim()
        .to_string()
}

fn fixture_concatenate_context() -> ConcatenateContext {
    ConcatenateContext {
        modules_exports_map: Default::default(),
        top_level_vars: hashset! {
            "external_cjs".to_string(),
            "external_esm".to_string(),
        },
        external_module_namespace: hashmap! {
           ModuleId::from("external") => ( "external_cjs".to_string(), "external_esm".to_string() )
        },
        interop_idents: Default::default(),
        interop_module_items: vec![],
    }
}
