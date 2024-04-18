use std::sync::Arc;

use mako_core::anyhow::Result;
use mako_core::swc_ecma_ast::{Expr, ExprStmt, Lit, Module, ModuleItem, Stmt, Str};

use super::client_info::{RscClientInfo, RscCssModules};
use crate::ast::css_ast::CssAst;
use crate::ast::file::File;
use crate::ast::js_ast::JsAst;
use crate::compiler::Context;
use crate::config::Config;
use crate::module::ModuleAst;

pub struct Rsc {}

impl Rsc {
    pub fn is_client(ast: &JsAst) -> bool {
        contains_str_stmt(&ast.ast, "use client")
    }

    pub fn is_server(ast: &JsAst) -> bool {
        contains_str_stmt(&ast.ast, "use server")
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

fn contains_str_stmt(ast: &Module, target: &str) -> bool {
    ast.body.iter().any(|stmt| {
        if let ModuleItem::Stmt(Stmt::Expr(ExprStmt {
            expr: box Expr::Lit(Lit::Str(Str { value, .. })),
            ..
        })) = stmt
        {
            return value == target;
        }
        false
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_client() {
        assert!(!Rsc::is_client(&build_ast(r#""#)));
        assert!(Rsc::is_client(&build_ast(r#""use client""#)));
        assert!(Rsc::is_client(&build_ast(r#"'use client'"#)));
        assert!(Rsc::is_client(&build_ast(r#"1;"use client";"#)));
        assert!(Rsc::is_client(&build_ast(r#"/*1*/"use client";"#)));
    }

    #[test]
    fn test_is_server() {
        assert!(!Rsc::is_server(&build_ast(r#""#)));
        assert!(Rsc::is_server(&build_ast(r#""use server""#)));
        assert!(Rsc::is_server(&build_ast(r#"'use server'"#)));
        assert!(Rsc::is_server(&build_ast(r#"1;"use server";"#)));
        assert!(Rsc::is_server(&build_ast(r#"/*1*/"use server";"#)));
    }

    fn build_ast(content: &str) -> JsAst {
        JsAst::build("test.ts", content, Default::default()).unwrap()
    }
}
