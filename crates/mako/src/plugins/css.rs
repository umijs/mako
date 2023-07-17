use std::sync::Arc;

use anyhow::Result;

use crate::ast::{build_css_ast, build_js_ast};
use crate::compiler::Context;
use crate::css_modules::{compile_css_modules, generate_code_for_css_modules, is_css_modules_path};
use crate::load::{read_content, Content};
use crate::module::ModuleAst;
use crate::parse::compile_css_compat;
use crate::plugin::{Plugin, PluginLoadParam, PluginParseParam};

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
            // return Ok(Some(ModuleAst::Css(param.request.clone())));
            let mut ast = build_css_ast(&param.request.path, content, context)?;
            let is_modules = param.request.has_query("modules");
            // parse css module as js
            if is_css_modules_path(&param.request.path) && !is_modules {
                let code = generate_code_for_css_modules(&param.request.path, &mut ast);
                let js_ast = build_js_ast(&param.request.path, &code, context)?;
                return Ok(Some(ModuleAst::Script(js_ast)));
            } else {
                // TODO: move to transform step
                // compile css compat
                compile_css_compat(&mut ast);
                // for mako css module, compile it and parse it as css
                if is_modules {
                    compile_css_modules(&param.request.path, &mut ast);
                }
                return Ok(Some(ModuleAst::Css(ast)));
            }
        }
        Ok(None)
    }
}
