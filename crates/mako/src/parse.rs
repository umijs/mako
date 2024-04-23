use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_css_visit::VisitMutWith as CSSVisitMutWith;
use mako_core::thiserror::Error;
use mako_core::tracing::debug;

use crate::analyze_deps::AnalyzeDeps;
use crate::ast::css_ast::CssAst;
use crate::ast::file::{Content, File};
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::features::rsc::Rsc;
use crate::module::ModuleAst;
use crate::plugin::PluginParseParam;
use crate::transform::Transform;
use crate::visitors::css_imports::CSSImports;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Unsupported content: {path:?}")]
    UnsupportedContent { path: String },
    #[error("Inline CSS missing deps: {path:?}")]
    InlineCSSMissingDeps { path: String },
    #[error(
        "Import module with `use server` from client components as server action is not supported yet: {path:?}"
    )]
    UnsupportedServerAction { path: String },
    #[error("The `\"{directive:?}\"` directive must be put at the top of the file.")]
    DirectiveNotOnTop { directive: String },
}

pub struct Parse {}

impl Parse {
    pub fn parse(file: &File, context: Arc<Context>) -> Result<ModuleAst> {
        mako_core::mako_profile_function!(file.path.to_string_lossy());

        // plugin first
        let ast = context
            .plugin_driver
            .parse(&PluginParseParam { file }, &context)?;
        if let Some(ast) = ast {
            return Ok(ast);
        }

        // js
        if let Some(Content::Js(_)) = &file.content {
            debug!("parse js: {:?}", file.path);
            let ast = JsAst::new(file, context.clone())?;
            if let Some(ast) = Rsc::parse_js(file, &ast, context.clone())? {
                return Ok(ast);
            }
            return Ok(ModuleAst::Script(ast));
        }

        // css
        if let Some(Content::Css(_)) = &file.content {
            debug!("parse css: {:?}", file.path);
            let is_modules = file.has_param("modules");
            let is_asmodule = file.has_param("asmodule");
            let css_modules = is_modules || is_asmodule;
            let mut ast = CssAst::new(file, context.clone(), css_modules)?;
            // ?asmodule
            if is_asmodule {
                let mut file = file.clone();
                file.set_content(Content::Js(CssAst::generate_css_modules_exports(
                    &file.pathname.to_string_lossy(),
                    &mut ast.ast,
                    context.config.css_modules_export_only_locales,
                )));
                let ast = JsAst::new(&file, context)?;
                return Ok(ModuleAst::Script(ast));
            } else {
                // when inline_css is enabled
                // we need to go through the css-related process first
                // and then hand it over to js for processing
                if context.config.inline_css.is_some() {
                    let mut ast = ModuleAst::Css(ast);
                    // transform
                    Transform::transform(&mut ast, file, context.clone())?;
                    // analyze_deps
                    // TODO: do not need to resolve here
                    let deps = AnalyzeDeps::analyze_deps(&ast, file, context.clone())?;
                    if !deps.missing_deps.is_empty() {
                        return Err(anyhow!(ParseError::InlineCSSMissingDeps {
                            path: file.path.to_string_lossy().to_string(),
                        }));
                    }
                    let deps = deps
                        .resolved_deps
                        .iter()
                        .map(|dep| {
                            format!("import '{}';", dep.resolver_resource.get_resolved_path())
                        })
                        .collect::<Vec<String>>()
                        .join("\n");
                    let ast = ast.as_css_mut();
                    // transform (remove @imports)
                    // TODO: Render::transform(&mut ast, &file, context.clone())?;
                    let mut css_handler = CSSImports {};
                    ast.ast.visit_mut_with(&mut css_handler);
                    // ast to code
                    let code = ast.generate(context.clone())?.code;
                    let mut file = file.clone();
                    file.set_content(Content::Js(format!(
                        r#"
import {{ moduleToDom }} from 'virtual:inline_css:runtime';
{}
moduleToDom(`
{}
`);
                    "#,
                        deps, code
                    )));
                    let ast = JsAst::new(&file, context.clone())?;
                    return Ok(ModuleAst::Script(ast));
                } else {
                    if let Some(ast) = Rsc::parse_css(file, context.clone())? {
                        return Ok(ast);
                    }
                    return Ok(ModuleAst::Css(ast));
                }
            }
        }

        Err(anyhow!(ParseError::UnsupportedContent {
            path: file.path.to_string_lossy().to_string(),
        }))
    }
}
