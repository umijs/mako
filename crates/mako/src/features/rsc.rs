use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::serde::Serialize;
use mako_core::swc_ecma_ast::{Expr, ExprStmt, Lit, Module, ModuleItem, Stmt, Str};

use crate::ast::css_ast::CssAst;
use crate::ast::file::File;
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::Config;
use crate::module::ModuleAst;
use crate::parse::ParseError;

#[derive(Serialize, Debug, Clone)]
pub struct RscClientInfo {
    pub path: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct RscCssModules {
    pub path: String,
}

pub struct Rsc {}

impl Rsc {
    pub fn is_client(ast: &JsAst) -> Result<bool> {
        contains_directive(&ast.ast, "use client")
    }

    pub fn is_server(ast: &JsAst) -> Result<bool> {
        contains_directive(&ast.ast, "use server")
    }

    pub fn generate_client(file: &File, tpl: &str, context: Arc<Context>) -> Result<ModuleAst> {
        let content = tpl.replace("{{path}}", file.relative_path.to_str().unwrap());
        Ok(ModuleAst::Script(
            JsAst::build(file.path.to_str().unwrap(), &content, context.clone()).unwrap(),
        ))
    }

    pub fn emit_client(file: &File, context: Arc<Context>) {
        let mut info = context.stats_info.lock().unwrap();
        info.rsc_client_components.push(RscClientInfo {
            path: file.relative_path.to_string_lossy().to_string(),
        });
    }

    pub fn emit_css(file: &File, context: Arc<Context>) {
        let mut info = context.stats_info.lock().unwrap();
        info.rsc_css_modules.push(RscCssModules {
            path: file.relative_path.to_string_lossy().to_string(),
        });
    }

    pub fn generate_empty_css(file: &File, context: Arc<Context>) -> Result<ModuleAst> {
        Ok(ModuleAst::Css(
            CssAst::build(file.path.to_str().unwrap(), "", context.clone(), false).unwrap(),
        ))
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
