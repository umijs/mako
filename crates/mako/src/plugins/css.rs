use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::base64::engine::{general_purpose, Engine};
use mako_core::swc_css_ast::{AtRule, AtRulePrelude, ImportHref, Rule, Str, Stylesheet, UrlValue};
use mako_core::swc_css_modules::{compile, CssClassName, TransformConfig, TransformResult};
use mako_core::swc_css_visit::{Visit, VisitMutWith, VisitWith};
use mako_core::{md5, swc_atoms, swc_common, swc_css_compat};

use crate::ast::{build_css_ast, build_js_ast};
use crate::compiler::Context;
use crate::load::{read_content, Content};
use crate::module::{Dependency, ModuleAst, ResolveType};
use crate::plugin::{Plugin, PluginDepAnalyzeParam, PluginLoadParam, PluginParseParam};

pub struct CSSPlugin {}

impl Plugin for CSSPlugin {
    fn name(&self) -> &str {
        "css"
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        if matches!(param.ext_name.as_str(), "css") {
            return Ok(Some(Content::Css(read_content(param.path.as_str())?)));
        }
        Ok(None)
    }

    fn parse(&self, param: &PluginParseParam, context: &Arc<Context>) -> Result<Option<ModuleAst>> {
        if let Content::Css(content) = param.content {
            let has_modules_query = param.request.has_query("modules");
            let has_asmodule_query = param.request.has_query("asmodule");
            let mut ast = build_css_ast(
                &param.request.path,
                content,
                context,
                has_asmodule_query || has_modules_query,
            )?;
            import_url_to_href(&mut ast);
            // parse css module as js
            if has_asmodule_query {
                let code = generate_code_for_css_modules(&param.request.path, &mut ast);
                let js_ast = build_js_ast(&param.request.path, &code, context)?;
                return Ok(Some(ModuleAst::Script(js_ast)));
            } else {
                // TODO: move to transform step
                // compile css compat
                compile_css_compat(&mut ast);
                // for mako css module, compile it and parse it as css
                if has_modules_query {
                    compile_css_modules(&param.request.path, &mut ast);
                }
                return Ok(Some(ModuleAst::Css(ast)));
            }
        }
        Ok(None)
    }

    fn analyze_deps(
        &self,
        param: &mut PluginDepAnalyzeParam,
        _context: &Arc<Context>,
    ) -> Result<Option<Vec<Dependency>>> {
        if let ModuleAst::Css(ast) = param.ast {
            let mut visitor = DepCollectVisitor::new();
            ast.visit_with(&mut visitor);
            Ok(Some(visitor.dependencies))
        } else {
            Ok(None)
        }
    }
}

fn compile_css_compat(ast: &mut Stylesheet) {
    ast.visit_mut_with(&mut swc_css_compat::compiler::Compiler::new(
        swc_css_compat::compiler::Config {
            process: swc_css_compat::feature::Features::NESTING,
        },
    ));
}

struct CssModuleRename {
    pub path: String,
}

impl TransformConfig for CssModuleRename {
    fn new_name_for(&self, local: &swc_atoms::JsWord) -> swc_atoms::JsWord {
        let name = local.to_string();
        let new_name = ident_name(&self.path, &name);
        new_name.into()
    }
}

fn ident_name(path: &str, name: &str) -> String {
    let source = format!("{}__{}", path, name);
    let digest = md5::compute(source);
    let hash = general_purpose::URL_SAFE.encode(digest.0);
    let hash_slice = hash[..8].to_string();
    format!("{}-{}", name, hash_slice)
}

fn compile_css_modules(path: &str, ast: &mut Stylesheet) -> TransformResult {
    compile(
        ast,
        CssModuleRename {
            path: path.to_string(),
        },
    )
}

fn generate_code_for_css_modules(path: &str, ast: &mut Stylesheet) -> String {
    let stylesheet = compile_css_modules(path, ast);

    let mut export_names = Vec::new();
    for (name, classes) in stylesheet.renamed.iter() {
        let mut after_transform_classes = Vec::new();
        for v in classes {
            match v {
                CssClassName::Local { name } => {
                    after_transform_classes.push(name.value.to_string());
                }
                CssClassName::Global { name } => {
                    // e.g. composes foo from global
                    after_transform_classes.push(name.value.to_string());
                }
                CssClassName::Import { name, from: _ } => {
                    // TODO: support composes from external files
                    after_transform_classes.push(name.value.to_string());
                }
            }
        }
        export_names.push((name, after_transform_classes));
    }
    let export_names = export_names
        .iter()
        .map(|(name, classes)| format!("\"{}\": `{}`", name, classes.join(" ").trim()))
        .collect::<Vec<String>>()
        .join(",");

    format!(
        r#"
import "{}?modules";
export default {{{}}}
"#,
        path, export_names
    )
}

// Why do this?
// 为了修复 @import url() 会把 css 当 asset 处理，返回 base64 的问题
// 把 @import url() 转成 @import 之后，所有 url() 就都是 rule 里的了
// e.g. @import url("foo") => @import "foo"
fn import_url_to_href(ast: &mut Stylesheet) {
    ast.rules.iter_mut().for_each(|rule| {
        if let Rule::AtRule(box AtRule {
            prelude: Some(box AtRulePrelude::ImportPrelude(preclude)),
            ..
        }) = rule
        {
            if let box ImportHref::Url(url) = &mut preclude.href {
                let href_string = url
                    .value
                    .as_ref()
                    .map(|box value| match value {
                        UrlValue::Str(str) => str.value.to_string(),
                        UrlValue::Raw(raw) => raw.value.to_string(),
                    })
                    .unwrap_or_default();
                preclude.href = Box::new(ImportHref::Str(Str {
                    span: url.span,
                    value: href_string.into(),
                    raw: None,
                }));
            }
        }
    });
}

pub fn is_url_ignored(url: &str) -> bool {
    let lower_url = url.to_lowercase();
    lower_url.starts_with("http://")
        || lower_url.starts_with("https://")
        || lower_url.starts_with("data:")
        || lower_url.starts_with("//")
}

pub fn handle_css_url(url: String) -> String {
    let mut url = url;
    // compatible with the legacy css-loader usage in webpack
    // ref: https://stackoverflow.com/a/39535907
    // @import "~foo" => "foo"
    if url.starts_with('~') {
        url = url[1..].to_string();
    }
    url
}

struct DepCollectVisitor {
    dependencies: Vec<Dependency>,
    order: usize,
}

impl DepCollectVisitor {
    fn new() -> Self {
        Self {
            dependencies: vec![],
            // start with 1
            // 0 for swc helpers
            order: 1,
        }
    }
    fn bind_dependency(
        &mut self,
        source: String,
        resolve_type: ResolveType,
        span: Option<swc_common::Span>,
    ) {
        self.dependencies.push(Dependency {
            source,
            order: self.order,
            resolve_type,
            span,
        });
        self.order += 1;
    }
    fn handle_css_url(&mut self, url: String) {
        if is_url_ignored(&url) {
            return;
        }
        let url = handle_css_url(url);
        self.bind_dependency(url, ResolveType::Css, None);
    }
}

impl Visit for DepCollectVisitor {
    fn visit_import_href(&mut self, n: &ImportHref) {
        match n {
            // e.g.
            // @import url(a.css)
            // @import url("a.css")
            ImportHref::Url(url) => {
                let src: Option<String> = url.value.as_ref().map(|box value| match value {
                    UrlValue::Str(str) => str.value.to_string(),
                    UrlValue::Raw(raw) => raw.value.to_string(),
                });
                if let Some(src) = src {
                    self.handle_css_url(src);
                }
            }
            // e.g.
            // @import "a.css"
            ImportHref::Str(src) => {
                let src = src.value.to_string();
                self.handle_css_url(src);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{ident_name, is_url_ignored};
    use crate::ast::build_css_ast;
    use crate::plugins::css::generate_code_for_css_modules;

    #[test]
    fn test_ident_name() {
        let result = ident_name("/test/path", "name");
        assert_eq!(result, "name-L9IOSlj5");
    }

    #[test]
    fn test_generate_code_for_css_modules() {
        let code = generate(
            r#"
.a {}
/* composes from global */
.b {
    composes: a from global;
}
/* composes from external files */
.c {
    composes: c from "./c.css";
}
        "#,
        );
        println!("{}", code);
        assert!(code.contains("\"a\": `a-"));
        assert!(code.contains("\"b\": `b-"));
        assert_eq!(
            code.trim(),
            r#"
import "/test/path?modules";
export default {"b": `b-KOXpblx_ a`,"c": `c-WTxpkVWA c`,"a": `a-hlnPCer-`}
        "#
            .trim()
        );
    }

    fn generate(code: &str) -> String {
        let path = "/test/path";
        let mut ast = build_css_ast(path, code, &Arc::new(Default::default()), true).unwrap();

        generate_code_for_css_modules(path, &mut ast)
    }

    #[test]
    fn test_is_url_ignored() {
        assert!(
            is_url_ignored(&String::from("http://abc")),
            "http should be ignored"
        );
        assert!(
            is_url_ignored(&String::from("https://abc")),
            "https should be ignored"
        );
        assert!(
            is_url_ignored(&String::from("HTTPS://abc")),
            "HTTPS should be ignored (support uppercase)"
        );
        assert!(
            is_url_ignored(&String::from("//abc")),
            "// prefixed url should be ignored"
        );
        assert!(
            is_url_ignored(&String::from("data:image")),
            "data should be ignored"
        );
        assert!(
            !is_url_ignored(&String::from("./abc")),
            "./ should not be ignored"
        );
    }
}
