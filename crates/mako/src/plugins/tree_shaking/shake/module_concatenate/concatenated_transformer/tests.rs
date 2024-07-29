use std::sync::Arc;

use maplit::{hashmap, hashset};
use swc_core::common::GLOBALS;
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::utils::quote_ident;
use swc_core::ecma::visit::VisitMutWith;

use super::super::ConcatenateContext;
use super::utils::describe_export_map;
use super::ConcatenatedTransform;
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::{Config, Mode, OptimizationConfig};
use crate::module::ModuleId;

#[test]
fn test_import_default_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import x from "./src";x"#, &mut ccn_ctx);

    assert_eq!(code, "inner_default_export;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_default_from_external() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import x from "external";x"#, &mut ccn_ctx);

    assert_eq!(code, "external_namespace_esm.default;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_default_from_inner_with_original_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"
    import inner_default_export from "./src";
    console.log(inner_default_export)"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "console.log(inner_default_export);");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[ignore]
#[test]
fn test_import_default_from_no_default_export_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("x".to_string());

    let code = inner_trans_code(
        r#"
    import x from "./no_exports";
    console.log(x)"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "var x = undefined;\nconsole.log(x);");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_default_from_inner_and_conflict_with_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("inner_default_export_1".to_string());

    let code = inner_trans_code(
        r#"import x from "./src";x;var inner_default_export =0;"#,
        &mut ccn_ctx,
    );

    assert_eq!(
        code,
        r#"inner_default_export;
var inner_default_export_1 = 0;"#
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {foo} from "./src";foo;"#, &mut ccn_ctx);

    assert_eq!(code, "bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_from_external() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {foo} from "external";foo;"#, &mut ccn_ctx);

    assert_eq!(code, "external_namespace_esm.foo;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_names_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import {foo, named} from "./src"; foo;named;"#,
        &mut ccn_ctx,
    );

    assert_eq!(
        code,
        r#"bar;
named;"#
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_name_and_default_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import v,{foo} from "./src"; foo;v;"#, &mut ccn_ctx);

    assert_eq!(
        code,
        r#"bar;
inner_default_export;"#
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_from_inner_with_same_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {named} from "./src";named"#, &mut ccn_ctx);

    assert_eq!(code, "named;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_as_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {foo as myFoo} from "./src";myFoo"#, &mut ccn_ctx);

    assert_eq!(code, "bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_as_from_external() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import {foo as myFoo} from "external";myFoo"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "external_namespace_esm.foo;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_as_from_inner_with_same_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {foo as bar} from "./src";bar;"#, &mut ccn_ctx);

    assert_eq!(code, "bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_named_as_from_inner_and_conflict_with_other_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import {foo as will_conflict} from "./src";will_conflict;"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_namespace_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import * as ns from "./src";ns"#, &mut ccn_ctx);

    assert_eq!(code, "inner_namespace;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_namespace_from_external() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import * as ns from "external";ns"#, &mut ccn_ctx);

    assert_eq!(code, "external_namespace_esm;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_namespace_from_inner_with_conflict() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import * as bar from "./src";bar;"#, &mut ccn_ctx);

    assert_eq!(code, "inner_namespace;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_import_namespace_from_inner_and_with_origin_namespace() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import * as inner_namespace from "./src";inner_namespace;
        "#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "inner_namespace;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

#[test]
fn test_export_named() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "n => n");
}

#[test]
fn test_export_named_as() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n as named };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "named => n");
}

#[test]
fn test_export_named_as_to_a_conflicted_local_var() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset! {
            "named".to_string(),
        },
        ..ConcatenateContext::default()
    };

    let code = inner_trans_code(
        "let named; var n = some.named;export { n as named};",
        &mut ccn_ctx,
    );

    assert_eq!(code, "let named_1;\nvar n = some.named;");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("n".to_string(), "named".to_string(), "named_1".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "named => n");
}

#[test]
fn test_export_named_as_default() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n as default };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "default => n");
}

#[test]
fn test_export_as_with_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("n".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code("var n = some.named;export { n as named };", &mut ccn_ctx);

    assert_eq!(code, "var n_1 = some.named;");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("n".to_string(), "n_1".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "named => n_1");
}

#[test]
fn test_export_default_expr_with_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("__$m_mut_js_0".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code("export default 1;", &mut ccn_ctx);

    assert_eq!(code, r#"var __$m_mut_js_0_1 = 1;"#);
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("__$m_mut_js_0_1".to_string(), "__$m_mut_js_0".to_string(),)
    );
    assert_eq!(describe_export_map(&ccn_ctx), "default => __$m_mut_js_0_1")
}

#[test]
fn test_export_as_twice_with_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("n".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code(
        "var n = some.named;export { n as named, n as foo };",
        &mut ccn_ctx,
    );

    assert_eq!(code, "var n_1 = some.named;");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("n".to_string(), "n_1".to_string())
    );
    assert_eq!(
        describe_export_map(&ccn_ctx),
        r#"
foo => n_1
named => n_1
"#
        .trim()
    );
}

#[test]
fn test_export_short_named() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var named = some.named;export { named };", &mut ccn_ctx);

    assert_eq!(code, "var named = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("named".to_string()));

    assert_eq!(describe_export_map(&ccn_ctx), "named => named")
}

#[test]
fn test_export_short_named_with_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("named".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code("var named = some.named;export { named };", &mut ccn_ctx);

    assert_eq!(code, "var named_1 = some.named;");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("named".to_string(), "named_1".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "named => named_1");
}

#[test]
fn test_export_default_decl_literal_expr() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default 42", &mut ccn_ctx);

    assert_eq!(code, "var __$m_mut_js_0 = 42;");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("__$m_mut_js_0".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "default => __$m_mut_js_0");
}

#[test]
fn test_export_default_decl_ident_expr() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("let t = 1; export default t", &mut ccn_ctx);

    assert_eq!(code, r#"let t = 1;"#);
    assert_eq!(ccn_ctx.top_level_vars, hashset!("t".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "default => t")
}

#[test]
fn test_export_decl_un_nameable_expr() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default 40+2", &mut ccn_ctx);

    assert_eq!(code, r#"var __$m_mut_js_0 = 40 + 2;"#);
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("__$m_mut_js_0".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "default => __$m_mut_js_0");
}

#[test]
fn test_export_default_decl_named_function() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default function a(){}", &mut ccn_ctx);

    assert_eq!(code, "function a() {}");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("a".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "default => a");
}

#[test]
fn test_export_default_decl_named_function_and_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("a".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code("export default function a(){}", &mut ccn_ctx);

    assert_eq!(code, "function a_1() {}");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("a".to_string(), "a_1".to_string())
    );
}

#[test]
fn test_export_decl_class() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export class A{}", &mut ccn_ctx);

    assert_eq!(
        code,
        "class A {
}"
    );
    assert_eq!(ccn_ctx.top_level_vars, hashset!("A".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "A => A")
}

#[test]
fn test_export_decl_class_and_conflict() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("A".to_string()),
        ..Default::default()
    };
    let code = inner_trans_code("export class A{}", &mut ccn_ctx);

    assert_eq!(
        code,
        "class A_1 {
}"
    );
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("A".to_string(), "A_1".to_string())
    );
}

#[test]
fn test_export_decl_fn() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export function fn(){}", &mut ccn_ctx);

    assert_eq!(code, "function fn() {}");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("fn".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "fn => fn")
}

#[test]
fn test_export_decl_fn_and_conflict_fn_name() {
    let mut ccn_ctx = ConcatenateContext {
        top_level_vars: hashset!("fn".to_string()),
        ..Default::default()
    };

    let code = inner_trans_code("export function fn(){}", &mut ccn_ctx);

    assert_eq!(code, "function fn_1() {}");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("fn".to_string(), "fn_1".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "fn => fn_1")
}

#[test]
fn test_export_decl_var() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export const a =1", &mut ccn_ctx);

    assert_eq!(code, "const a = 1;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("a".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "a => a")
}

#[test]
fn test_export_default_decl_named_class() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default class A{}", &mut ccn_ctx);

    assert_eq!(
        code,
        "class A {
}"
    );
    assert_eq!(ccn_ctx.top_level_vars, hashset!("A".to_string()));
    assert_eq!(describe_export_map(&ccn_ctx), "default => A")
}

#[test]
fn test_export_default_anonymous_decl_class() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default class {}", &mut ccn_ctx);

    assert_eq!(
        code,
        "var __$m_mut_js_0 = class {
};"
    );
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("__$m_mut_js_0".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "default => __$m_mut_js_0")
}

#[test]
fn test_export_default_anonymous_decl_fn() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default function() {}", &mut ccn_ctx);

    assert_eq!(code, "var __$m_mut_js_0 = function() {};");
    assert_eq!(
        ccn_ctx.top_level_vars,
        hashset!("__$m_mut_js_0".to_string())
    );
    assert_eq!(describe_export_map(&ccn_ctx), "default => __$m_mut_js_0");
}

#[test]
fn test_export_from_named() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"export { named } from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(describe_export_map(&ccn_ctx), "named => named")
}

#[test]
fn test_export_from_named_as() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"export { named as foo} from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(describe_export_map(&ccn_ctx), "foo => named")
}

#[test]
fn test_export_from_namespace_as() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"export * as foo from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(describe_export_map(&ccn_ctx), "foo => inner_namespace")
}

#[ignore = "export v from 'mod' not supported now"]
#[test]
fn test_export_from_var() {
    let mut ccn_ctx = ConcatenateContext {
        modules_exports_map: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "default".into() => (quote_ident!("src_index_default"), None)
            }
        },
        ..Default::default()
    };
    let orig_top_level_vars = ccn_ctx.top_level_vars.clone();
    let code = inner_trans_code(r#"export v from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, orig_top_level_vars);
}

#[test]
fn test_export_from_default() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"export { default } from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
}

fn concatenate_context_fixture_with_inner_module() -> ConcatenateContext {
    ConcatenateContext {
        top_level_vars: hashset! {
            "bar".to_string(),
            "inner_default_export".to_string(),
            "inner_namespace".to_string(),
            "named".to_string(),
            "will_conflict".to_string(),
        },
        modules_exports_map: hashmap! {
            ModuleId::from("src/index.js") => hashmap!{
                "*".into() => (quote_ident!("inner_namespace"), None),
                "default".into() => ( quote_ident!("inner_default_export"), None),
                "foo".into() => (quote_ident!("bar") ,None),
                "named".into() => (quote_ident!("named"), None)
            },
            ModuleId::from("src/no_exports.js") => hashmap!{},
        },
        external_module_namespace: hashmap! {
            ModuleId::from("external") => ("external_namespace".to_string(),
                "external_namespace_esm"
                .to_string() )
        },
        ..Default::default()
    }
}

fn inner_trans_code(code: &str, concatenate_context: &mut ConcatenateContext) -> String {
    let context = Arc::new(Context {
        config: Config {
            devtool: None,
            optimization: Some(OptimizationConfig {
                concatenate_modules: Some(true),
                skip_modules: Some(true),
            }),
            mode: Mode::Production,
            minify: false,
            ..Default::default()
        },
        ..Default::default()
    });

    let mut ast = JsAst::build("mut.js", code, context.clone()).unwrap();
    let module_id = ModuleId::from("mut.js");

    let src_to_module = hashmap! {
        "./src".into() => ModuleId::from("src/index.js"),
        "./no_exports".into() => ModuleId::from("src/no_exports.js"),
    "external".into() => ModuleId::from("external")
    };

    GLOBALS.set(&context.meta.script.globals, || {
        let mut inner = ConcatenatedTransform::new(
            concatenate_context,
            &module_id,
            &src_to_module,
            &context,
            ast.top_level_mark,
        );

        ast.ast.visit_mut_with(&mut resolver(
            ast.unresolved_mark,
            ast.top_level_mark,
            false,
        ));
        ast.ast.visit_mut_with(&mut inner);

        {
            // do not need comments
            let mut comment = context.meta.script.origin_comments.write().unwrap();
            *comment = Default::default();
        }

        ast.generate(context.clone())
            .unwrap()
            .code
            .trim()
            .to_string()
    })
}
