use std::collections::HashMap;
use std::sync::Arc;

use maplit::{hashmap, hashset};
use swc_core::common::GLOBALS;
use swc_core::ecma::transforms::base::resolver;
use swc_core::ecma::visit::VisitMutWith;

use super::InnerTransform;
use crate::ast::{build_js_ast, js_ast_to_code};
use crate::compiler::Context;
use crate::config::{Config, Mode, OptimizationConfig};
use crate::module::ModuleId;
use crate::plugins::farm_tree_shake::shake::module_concatenate::concatenate_context::ConcatenateContext;

#[test]
fn test_import_default_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("x".to_string());

    let code = inner_trans_code(r#"import x from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "var x = inner_default_export;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}
#[test]
fn test_import_default_from_inner_and_conflict_with_other_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("will_conflict_1".to_string());

    let code = inner_trans_code(
        r#"import will_conflict from "./src";will_conflict;"#,
        &mut ccn_ctx,
    );

    assert_eq!(
        code,
        "var will_conflict_1 = inner_default_export;\nwill_conflict_1;"
    );
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_default_from_inner_and_conflict_with_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import inner_default_export from "./src";inner_default_export;"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "inner_default_export;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_named_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("foo".to_string());

    let code = inner_trans_code(r#"import {foo} from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "var foo = bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_named_from_inner_conflict_with_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {named} from "./src";named"#, &mut ccn_ctx);

    assert_eq!(code, "named;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_named_as_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("myFoo".to_string());

    let code = inner_trans_code(r#"import {foo as myFoo} from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "var myFoo = bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_named_as_from_inner_and_conflict_with_orig_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(r#"import {foo as bar} from "./src";bar;"#, &mut ccn_ctx);

    assert_eq!(code, "bar;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_named_as_from_inner_and_conflict_with_other_name() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("will_conflict_1".to_string());

    let code = inner_trans_code(
        r#"import {foo as will_conflict} from "./src";will_conflict;"#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "var will_conflict_1 = bar;\nwill_conflict_1;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_namespace_from_inner() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("ns".to_string());

    let code = inner_trans_code(r#"import * as ns from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "var ns = inner_namespace;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_namespace_from_inner_with_conflict() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let mut expected_top_vars = ccn_ctx.top_level_vars.clone();
    expected_top_vars.insert("bar_1".to_string());

    let code = inner_trans_code(r#"import * as bar from "./src";bar;"#, &mut ccn_ctx);

    assert_eq!(code, "var bar_1 = inner_namespace;\nbar_1;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_import_namespace_from_inner_and_conflict_with_namespace() {
    let mut ccn_ctx = concatenate_context_fixture_with_inner_module();
    let expected_top_vars = ccn_ctx.top_level_vars.clone();

    let code = inner_trans_code(
        r#"import * as inner_namespace from "./src";inner_namespace;
        "#,
        &mut ccn_ctx,
    );

    assert_eq!(code, "inner_namespace;");
    assert_eq!(ccn_ctx.top_level_vars, expected_top_vars);
    assert_eq!(current_export_map(&ccn_ctx), &hashmap!());
}

#[test]
fn test_export_named() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!("n".to_string() => "n".to_string())
    );
}

#[test]
fn test_export_named_as() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n as named };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!("named".to_string() => "n".to_string())
    );
}

#[test]
fn test_export_named_as_default() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var n = some.named;export { n as default };", &mut ccn_ctx);

    assert_eq!(code, "var n = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("n".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!("default".to_string() => "n".to_string())
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!("named".to_string() => "n_1".to_string())
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!("default".to_string() => "__$m_mut_js_0_1".to_string())
    );
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
        current_export_map(&ccn_ctx),
        &hashmap!(
            "named".to_string() => "n_1".to_string(),
            "foo".to_string() => "n_1".to_string()
        )
    );
}

#[test]
fn test_short_named_export() {
    let mut ccn_ctx = ConcatenateContext::default();

    let code = inner_trans_code("var named = some.named;export { named };", &mut ccn_ctx);

    assert_eq!(code, "var named = some.named;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("named".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "named".to_string() => "named".to_string()
        )
    );
}

#[test]
fn test_short_named_export_with_conflict() {
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "named".to_string() => "named_1".to_string()
        )
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "__$m_mut_js_0".to_string()
        )
    );
}

#[test]
fn test_export_default_decl_ident_expr() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("let t = 1; export default t", &mut ccn_ctx);

    assert_eq!(code, r#"let t = 1;"#);
    assert_eq!(ccn_ctx.top_level_vars, hashset!("t".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "t".to_string()
        )
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "__$m_mut_js_0".to_string()
        )
    );
}

#[test]
fn test_export_default_decl_named_function() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export default function a(){}", &mut ccn_ctx);

    assert_eq!(code, "function a() {}");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("a".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "a".to_string()
        )
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "A".to_string() => "A".to_string()
        )
    );
}

#[test]
fn test_export_decl_fn() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export function fn(){}", &mut ccn_ctx);

    assert_eq!(code, "function fn() {}");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("fn".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "fn".to_string() => "fn".to_string()
        )
    );
}

#[test]
fn test_export_decl_var() {
    let mut ccn_ctx = ConcatenateContext::default();
    let code = inner_trans_code("export const a =1", &mut ccn_ctx);

    assert_eq!(code, "const a = 1;");
    assert_eq!(ccn_ctx.top_level_vars, hashset!("a".to_string()));
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "a".to_string() => "a".to_string()
        )
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "A".to_string()
        )
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "__$m_mut_js_0".to_string()
        )
    );
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
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "__$m_mut_js_0".to_string()
        )
    );
}

#[test]
fn test_export_from_named() {
    let mut ccn_ctx = ConcatenateContext {
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "named".to_string() => "named".to_string()
            }
        },
        ..Default::default()
    };
    let code = inner_trans_code(r#"export { named } from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert!(ccn_ctx.top_level_vars.is_empty());
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "named".to_string() => "named".to_string()
        )
    );
}

#[test]
fn test_export_from_named_as() {
    let mut ccn_ctx = ConcatenateContext {
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "named".to_string() => "named".to_string()
            }
        },
        ..Default::default()
    };
    let code = inner_trans_code(r#"export { named as foo} from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert!(ccn_ctx.top_level_vars.is_empty());
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "foo".to_string() => "named".to_string()
        )
    );
}

#[test]
fn test_export_from_namespace_as() {
    let mut ccn_ctx = ConcatenateContext {
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "*".to_string() => "src_namespace".to_string()
            }
        },
        ..Default::default()
    };
    let code = inner_trans_code(r#"export * as foo from "./src""#, &mut ccn_ctx);

    assert_eq!(code, "");
    assert!(ccn_ctx.top_level_vars.is_empty());
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "foo".to_string() => "src_namespace".to_string()
        )
    );
}

#[ignore = "export v from 'mod' not supported now"]
#[test]
fn test_export_from_var() {
    let mut ccn_ctx = ConcatenateContext {
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "default".to_string() => "src_index_default".to_string()
            }
        },
        ..Default::default()
    };
    let orig_top_level_vars = ccn_ctx.top_level_vars.clone();
    let code = inner_trans_code(r#"export v from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, orig_top_level_vars);
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "v".to_string() => "src_index_default".to_string()
        )
    );
}

#[test]
fn test_export_from_default() {
    let mut ccn_ctx = ConcatenateContext {
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap! {
                "default".to_string() => "src_index_default".to_string()
            }
        },
        ..Default::default()
    };
    let orig_top_level_vars = ccn_ctx.top_level_vars.clone();
    let code = inner_trans_code(r#"export { default } from "./src""#, &mut ccn_ctx);

    assert_eq!(code, r#""#);
    assert_eq!(ccn_ctx.top_level_vars, orig_top_level_vars);
    assert_eq!(
        current_export_map(&ccn_ctx),
        &hashmap!(
            "default".to_string() => "src_index_default".to_string()
        )
    );
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
        modules_in_scope: hashmap! {
            ModuleId::from("src/index.js") => hashmap!{
                "*".to_string() => "inner_namespace".to_string(),
                "default".to_string() => "inner_default_export".to_string(),
                "foo".to_string() => "bar".to_string(),
                "named".to_string() => "named".to_string(),
            }
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

    let mut ast = build_js_ast("mut.js", code, &context).unwrap();
    let module_id = ModuleId::from("mut.js");

    let src_to_module = hashmap! {
        "./src".to_string() => ModuleId::from("src/index.js")
    };

    GLOBALS.set(&context.meta.script.globals, || {
        let mut inner = InnerTransform::new(
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

        let (code, _) = js_ast_to_code(&ast.ast, &context, "mut.js").unwrap();
        code.trim().to_string()
    })
}

fn current_export_map(ccn_ctx: &ConcatenateContext) -> &HashMap<String, String> {
    ccn_ctx
        .modules_in_scope
        .get(&ModuleId::from("mut.js"))
        .unwrap()
}
