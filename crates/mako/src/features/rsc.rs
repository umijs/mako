use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde::Serialize;
use swc_core::ecma::ast::{Expr, ExprStmt, Lit, Module, ModuleItem, Stmt, Str};

use crate::ast::css_ast::CssAst;
use crate::ast::file::File;
use crate::ast::js_ast::JsAst;
use crate::build::parse::ParseError;
use crate::compiler::Context;
use crate::config::{Config, LogServerComponent};
use crate::module::{ModuleAst, ModuleId};

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RscClientInfo {
    pub path: String,
    pub module_id: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RscCssModules {
    pub path: String,
    pub module_id: String,
    pub modules: bool,
}

pub struct Rsc {}

impl Rsc {
    pub fn parse_js(file: &File, ast: &JsAst, context: Arc<Context>) -> Result<Option<ModuleAst>> {
        if let Some(rsc_server) = context.config.rsc_server.as_ref() {
            if Rsc::is_client(ast)? {
                Rsc::emit_client(file, context.clone());
                return Ok(Some(Self::generate_client(
                    file,
                    &rsc_server.client_component_tpl,
                    context.clone(),
                )));
            }
        }
        if let Some(rsc_client) = &context.config.rsc_client {
            let is_server = Rsc::is_server(ast)?;
            if is_server && matches!(rsc_client.log_server_component, LogServerComponent::Error) {
                return Err(anyhow!(ParseError::UnsupportedServerAction {
                    path: file.path.to_string_lossy().to_string(),
                }));
            }
        }
        Ok(None)
    }

    fn is_client(ast: &JsAst) -> Result<bool> {
        contains_directive(&ast.ast, "use client")
    }

    fn is_server(ast: &JsAst) -> Result<bool> {
        contains_directive(&ast.ast, "use server")
    }

    fn generate_client(file: &File, tpl: &str, context: Arc<Context>) -> ModuleAst {
        let id = ModuleId::new(file.path.to_string_lossy().to_string()).generate(&context);
        let path = file.relative_path.to_string_lossy().to_string();
        let content = tpl
            .replace("{{path}}", path.as_str())
            .replace("{{id}}", id.as_str());
        ModuleAst::Script(
            JsAst::build(file.path.to_str().unwrap(), &content, context.clone()).unwrap(),
        )
    }

    fn emit_client(file: &File, context: Arc<Context>) {
        let stats_info = &context.stats_info;
        let module_id = ModuleId::from_path(file.path.clone()).generate(&context);
        stats_info.add_rsc_client_component(RscClientInfo {
            path: file.relative_path.to_string_lossy().to_string(),
            module_id,
        });
    }

    pub fn parse_css(file: &File, context: Arc<Context>) -> Result<Option<ModuleAst>> {
        if context
            .config
            .rsc_server
            .as_ref()
            .is_some_and(|rsc_server| rsc_server.emit_css)
        {
            Rsc::emit_css(file, context.clone());
            return Ok(Some(Rsc::generate_empty_css(file, context.clone())));
        }
        Ok(None)
    }

    fn emit_css(file: &File, context: Arc<Context>) {
        let stats_info = &context.stats_info;
        let module_id = ModuleId::from_path(file.path.clone()).generate(&context);
        stats_info.add_rsc_css_module(RscCssModules {
            path: file.relative_path.to_string_lossy().to_string(),
            module_id,
            modules: file.is_css() && file.has_param("modules"),
        });
    }

    fn generate_empty_css(file: &File, context: Arc<Context>) -> ModuleAst {
        ModuleAst::Css(
            CssAst::build(file.path.to_str().unwrap(), "", context.clone(), false).unwrap(),
        )
    }

    pub fn generate_resolve_conditions(config: &Config, conditions: Vec<String>) -> Vec<String> {
        let mut conditions = conditions;
        if config.rsc_server.is_some() {
            conditions.insert(0, "react-server".to_string())
        }
        conditions
    }
}

fn contains_directive(ast: &Module, directive: &str) -> Result<bool> {
    let mut is_directive = true;
    let mut is_target_directive = false;
    let mut error: Option<ParseError> = None;
    ast.body.iter().for_each(|stmt| {
        if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            expr: box Expr::Lit(Lit::Str(Str { value, .. })),
            ..
        })) = stmt
        {
            if value == directive {
                if is_directive {
                    is_target_directive = true;
                } else {
                    error = Some(ParseError::DirectiveNotOnTop {
                        directive: directive.to_string(),
                    });
                }
            }
        } else {
            is_directive = false;
        }
    });
    if let Some(error) = error {
        return Err(anyhow!(error));
    }
    Ok(is_target_directive)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_client() {
        assert!(!Rsc::is_client(&build_ast(r#""#)).unwrap());
        assert!(Rsc::is_client(&build_ast(r#""use client""#)).unwrap());
        assert!(Rsc::is_client(&build_ast(r#"'use client'"#)).unwrap());
        assert!(Rsc::is_client(&build_ast(r#"/*1*/"use client";"#)).unwrap());
        assert!(Rsc::is_client(&build_ast(r#""use strict";"use client";"#)).unwrap());
    }

    #[test]
    fn test_is_client_not_on_top() {
        assert!(Rsc::is_client(&build_ast(r#"1;"use client";"#)).is_err());
    }

    #[test]
    fn test_is_server() {
        assert!(Rsc::is_server(&build_ast(r#""use server""#)).unwrap());
    }

    fn build_ast(content: &str) -> JsAst {
        JsAst::build("test.ts", content, Default::default()).unwrap()
    }
}
