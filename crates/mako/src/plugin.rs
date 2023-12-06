use std::any::Any;
use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_common::errors::Handler;
use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::Module;

use crate::build::FileRequest;
use crate::compiler::{Args, Context};
use crate::config::Config;
use crate::load::Content;
use crate::module::{Dependency, ModuleAst};
use crate::module_graph::ModuleGraph;
use crate::stats::StatsJsonMap;

#[derive(Debug)]
pub struct PluginLoadParam<'a> {
    pub path: String,
    pub is_entry: bool,
    pub ext_name: Option<&'a str>,
    pub request: &'a FileRequest,
}

pub struct PluginParseParam<'a> {
    pub request: &'a FileRequest,
    pub content: &'a Content,
}

pub struct PluginCheckAstParam<'a> {
    pub ast: &'a ModuleAst,
}

pub struct PluginTransformJsParam<'a> {
    pub handler: &'a Handler,
    pub path: &'a str,
    pub top_level_mark: Mark,
    pub unresolved_mark: Mark,
}

pub struct PluginDepAnalyzeParam<'a> {
    pub ast: &'a ModuleAst,
}

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;

    fn modify_config(&self, _config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        Ok(())
    }

    fn load(&self, _param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        Ok(None)
    }

    fn parse(
        &self,
        _param: &PluginParseParam,
        _context: &Arc<Context>,
    ) -> Result<Option<ModuleAst>> {
        Ok(None)
    }

    fn check_ast(&self, _param: &PluginCheckAstParam, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn transform_js(
        &self,
        _param: &PluginTransformJsParam,
        _ast: &mut Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn analyze_deps(
        &self,
        _param: &mut PluginDepAnalyzeParam,
        _context: &Arc<Context>,
    ) -> Result<Option<Vec<Dependency>>> {
        Ok(None)
    }

    fn before_resolve(&self, _deps: &mut Vec<Dependency>, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn generate(&self, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
    }

    fn build_success(&self, _stats: &StatsJsonMap, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn optimize_module_graph(
        &self,
        _module_graph: &mut ModuleGraph,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct PluginDriver {
    plugins: Vec<Arc<dyn Plugin>>,
}
impl PluginDriver {
    pub fn new(plugins: Vec<Arc<dyn Plugin>>) -> Self {
        Self { plugins }
    }

    pub fn modify_config(&self, config: &mut Config, root: &Path, args: &Args) -> Result<()> {
        for plugin in &self.plugins {
            plugin.modify_config(config, root, args)?;
        }
        Ok(())
    }

    pub fn load(&self, param: &PluginLoadParam, context: &Arc<Context>) -> Result<Option<Content>> {
        for plugin in &self.plugins {
            let ret = plugin.load(param, context)?;
            if ret.is_some() {
                return Ok(ret);
            }
        }
        Ok(None)
    }

    pub fn parse(
        &self,
        param: &PluginParseParam,
        context: &Arc<Context>,
    ) -> Result<Option<ModuleAst>> {
        for plugin in &self.plugins {
            let ret = plugin.parse(param, context)?;
            if ret.is_some() {
                return Ok(ret);
            }
        }
        Ok(None)
    }

    pub fn check_ast(&self, param: &PluginCheckAstParam, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.check_ast(param, context)?;
        }
        Ok(())
    }

    pub fn transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut Module,
        context: &Arc<Context>,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin.transform_js(param, ast, context)?;
        }
        Ok(())
    }

    pub fn analyze_deps(
        &self,
        param: &mut PluginDepAnalyzeParam,
        context: &Arc<Context>,
    ) -> Result<Vec<Dependency>> {
        for plugin in &self.plugins {
            let ret = plugin.analyze_deps(param, context)?;
            if let Some(ret) = ret {
                return Ok(ret);
            }
        }
        Ok(vec![])
    }

    pub fn before_resolve(
        &self,
        param: &mut Vec<Dependency>,
        context: &Arc<Context>,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin.before_resolve(param, context)?;
        }
        Ok(())
    }

    pub fn generate(&self, context: &Arc<Context>) -> Result<Option<()>> {
        for plugin in &self.plugins {
            let ret = plugin.generate(context)?;
            if ret.is_some() {
                return Ok(Some(()));
            }
        }
        Err(anyhow!("None of the plugins generate content"))
    }

    pub fn build_success(
        &self,
        stats: &StatsJsonMap,
        context: &Arc<Context>,
    ) -> Result<Option<()>> {
        for plugin in &self.plugins {
            plugin.build_success(stats, context)?;
        }
        Ok(None)
    }

    pub fn runtime_plugins_code(&self, context: &Arc<Context>) -> Result<String> {
        let mut plugins = Vec::new();
        for plugin in &self.plugins {
            plugins.extend(plugin.runtime_plugins(context)?);
        }
        Ok(plugins.join("\n"))
    }

    pub fn optimize_module_graph(
        &self,
        module_graph: &mut ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<()> {
        for p in &self.plugins {
            p.optimize_module_graph(module_graph, context)?;
        }

        Ok(())
    }
}
