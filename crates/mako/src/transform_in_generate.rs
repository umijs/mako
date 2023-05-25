use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use std::collections::HashMap;
use swc_atoms::JsWord;
use swc_common::collections::AHashMap;
use swc_common::sync::Lrc;
use swc_common::{Globals, SourceMap, DUMMY_SP, GLOBALS};
use swc_css_visit::VisitMutWith as CssVisitMutWith;
use swc_ecma_ast::{Expr, ExprOrSpread, ExprStmt, Lit, Module, ModuleItem, Stmt, Str};
use swc_ecma_visit::VisitMutWith;
use tracing::{debug, info};

use crate::ast::{build_js_ast, css_ast_to_code};
use crate::compiler::Context;
use crate::module::ModuleId;
use crate::transform_env_replacer::EnvReplacer;
use crate::{compiler::Compiler, module::ModuleAst, transform_dep_replacer::DepReplacer};

impl Compiler {
    pub fn transform_all(&self) {
        info!("transform all modules");
        let context = &self.context;
        let module_graph = context.module_graph.read().unwrap();
        let module_ids = module_graph.get_module_ids();
        drop(module_graph);
        debug!("module ids: {:?}", module_ids);
        transform_modules(module_ids, context);
    }
}

fn transform_modules(module_ids: Vec<ModuleId>, context: &Context) {
    // build env map
    let env_map = build_env_map(HashMap::from([("NODE_ENV".into(), "production".into())]));

    module_ids.iter().for_each(|module_id| {
        let module_graph = context.module_graph.read().unwrap();
        let deps = module_graph.get_dependencies(module_id);

        let dep_map: HashMap<String, String> = deps
            .into_iter()
            .map(|(id, dep)| (dep.source.clone(), id.id.clone()))
            .collect();
        drop(module_graph);

        let mut module_graph = context.module_graph.write().unwrap();
        let module = module_graph.get_module_mut(module_id).unwrap();
        let info = module.info.as_mut().unwrap();
        let path = info.path.clone();
        let ast = &mut info.ast;
        match ast {
            ModuleAst::Script(ast) => {
                transform_js(ast, &module.id.id, &path, dep_map, env_map.clone());
            }
            ModuleAst::Css(ast) => {
                let (ast, _cm) = transform_css(ast, &module.id.id, &path, dep_map);
                info.set_ast(ModuleAst::Script(ast));
            }
            ModuleAst::None => {}
        }
    });
}

fn transform_css(
    ast: &mut swc_css_ast::Stylesheet,
    id: &str,
    path: &str,
    dep_map: HashMap<String, String>,
) -> (Module, Lrc<SourceMap>) {
    // remove @import
    let mut dep_replacer = DepReplacer {
        dep_map: Default::default(),
    };
    ast.visit_mut_with(&mut dep_replacer);

    // ast to code
    let code = css_ast_to_code(ast);

    // lightingcss
    let mut lightingcss_stylesheet = StyleSheet::parse(&code, ParserOptions::default()).unwrap();
    lightingcss_stylesheet
        .minify(MinifyOptions::default())
        .unwrap();
    let out = lightingcss_stylesheet
        .to_css(PrinterOptions::default())
        .unwrap();
    let code = out.code.as_str();

    // code to js ast
    let content = include_str!("runtime/runtime_css.ts").to_string();
    let content = content.replace("__CSS__", code);
    let require_code: Vec<String> = dep_map
        .values()
        .map(|val| format!("require(\"{}\");", val))
        .collect();
    let content = format!("{}{}", require_code.join("\n"), content);
    let path = format!("{}.ts", path);
    let path = path.as_str();
    let (cm, mut ast) = build_js_ast(path, content.as_str());

    // wrap js module
    wrap_js_module(&mut ast, id, path);

    (ast, cm)
}

fn build_env_map(env_map: HashMap<String, String>) -> AHashMap<JsWord, Expr> {
    let mut map = AHashMap::default();
    env_map.into_iter().for_each(|(k, v)| {
        map.insert(
            k.into(),
            Expr::Lit(Lit::Str(Str {
                span: DUMMY_SP,
                raw: None,
                value: v.into(),
            })),
        );
    });
    map
}

fn transform_js(
    ast: &mut swc_ecma_ast::Module,
    id: &str,
    path: &str,
    dep_map: HashMap<String, String>,
    env_map: AHashMap<JsWord, Expr>,
) {
    let globals = Globals::default();
    GLOBALS.set(&globals, || {
        let mut dep_replacer = DepReplacer { dep_map };
        ast.visit_mut_with(&mut dep_replacer);

        let mut env_replacer = EnvReplacer::new(Lrc::new(env_map));
        ast.visit_mut_with(&mut env_replacer);
    });

    wrap_js_module(ast, id, path);
}

fn wrap_js_module(ast: &mut Module, id: &str, path: &str) {
    // 找到 call_expr 的第二个 fn 参数，将原来的 stmts 加入到新的 fn 的 body 中
    // 用字符串生成 ast 的方式是为了容易维护，因为走 ast 拼接的方式不易懂，同时前期修改可能比较频繁
    let origin_stmts: Vec<Stmt> = ast
        .body
        .iter()
        .map(|stmt| stmt.as_stmt().unwrap().clone())
        .collect();
    let content = include_str!("runtime/runtime_module.ts").replace("__ID__", id);
    let (_cm, mut new_ast) = build_js_ast(path, content.as_str());
    for stmt in &mut new_ast.body {
        if let ModuleItem::Stmt(Stmt::Expr(expr)) = stmt {
            if let ExprStmt {
                expr: box Expr::Call(call_expr),
                ..
            } = expr
            {
                if let ExprOrSpread {
                    expr: box Expr::Fn(func),
                    ..
                } = &mut call_expr.args[1]
                {
                    func.function
                        .body
                        .as_mut()
                        .unwrap()
                        .stmts
                        .extend(origin_stmts);
                    break;
                }
            }
        }
    }
    *ast = new_ast;
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::ast::{build_css_ast, build_js_ast, js_ast_to_code};

    use super::{build_env_map, transform_css};

    #[test]
    fn test_transform_js_dep_replacer() {
        let code = r#"
require("foo");
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(
            code,
            None,
            HashMap::from([("foo".into(), "bar".into())]),
            Default::default(),
        );
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
g_define('test', function(module, exports, require) {
    require("bar");
});
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_js_env_replacer() {
        let code = r#"
if (process.env.NODE_ENV === "production") 1;
        "#
        .trim();
        let (code, _sourcemap) = transform_js_code(
            code,
            None,
            Default::default(),
            HashMap::from([("NODE_ENV".into(), "production".into())]),
        );
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
g_define('test', function(module, exports, require) {
    if ("production" === "production") 1;
});
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_js_wrapper() {
        let code = r#"
const Foo = "foo";
        "#
        .trim();
        let (code, _sourcemap) =
            transform_js_code(code, None, Default::default(), Default::default());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
g_define('test', function(module, exports, require) {
    const Foo = "foo";
});
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css() {
        let code = r#"
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) = transform_css_code(code, None, Default::default());
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
g_define('test.css', function(module, exports, require) {
    let css = `.foo {
  color: red;
}
`;
    let style = document.createElement('style');
    style.innerHTML = css;
    document.head.appendChild(style);
});
        "#
            .trim()
        );
    }

    #[test]
    fn test_transform_css_import() {
        let code = r#"
@import "./foo.css";
.foo { color: red; }
        "#
        .trim();
        let (code, _cm) =
            transform_css_code(code, None, HashMap::from([("1".into(), "bar.css".into())]));
        println!(">> CODE\n{}", code);
        assert_eq!(
            code,
            r#"
g_define('test.css', function(module, exports, require) {
    require("bar.css");
    let css = `.foo {
  color: red;
}
`;
    let style = document.createElement('style');
    style.innerHTML = css;
    document.head.appendChild(style);
});
        "#
            .trim()
        );
    }

    fn transform_js_code(
        origin: &str,
        path: Option<&str>,
        dep_map: HashMap<String, String>,
        env_map: HashMap<String, String>,
    ) -> (String, String) {
        let path = if path.is_none() {
            "test.tsx"
        } else {
            path.unwrap()
        };
        let (cm, mut ast) = build_js_ast(path, origin);
        let env_map = build_env_map(env_map);
        super::transform_js(&mut ast, "test", path, dep_map, env_map);
        let (code, _sourcemap) = js_ast_to_code(&ast, &cm);
        // let code = code.replace("\"use strict\";", "");
        let code = code.trim().to_string();
        (code, _sourcemap)
    }

    fn transform_css_code(
        content: &str,
        path: Option<&str>,
        dep_map: HashMap<String, String>,
    ) -> (String, String) {
        let path = if path.is_none() {
            "test.tsx"
        } else {
            path.unwrap()
        };
        let (_cm, mut ast) = build_css_ast(path, content);
        let (ast, cm) = transform_css(&mut ast, "test.css", path, dep_map);
        let (code, _sourcemap) = js_ast_to_code(&ast, &cm);
        let code = code.trim().to_string();
        (code, _sourcemap)
    }
}
