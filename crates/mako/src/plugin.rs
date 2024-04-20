use std::any::Any;
use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::swc_common::errors::Handler;
use mako_core::swc_common::Mark;
use mako_core::swc_ecma_ast::Module;

use crate::ast::file::{Content, File};
use crate::chunk_graph::ChunkGraph;
use crate::compiler::{Args, Context};
use crate::config::Config;
use crate::module::{Dependency, ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;
use crate::stats::StatsJsonMap;

#[derive(Debug)]
pub struct PluginLoadParam<'a> {
    pub file: &'a File,
}

pub struct PluginParseParam<'a> {
    pub file: &'a File,
}

pub struct PluginTransformJsParam<'a> {
    pub handler: &'a Handler,
    pub path: &'a str,
    pub top_level_mark: Mark,
    pub unresolved_mark: Mark,
}

#[derive(Clone)]
pub struct PluginGenerateEndParams {
    pub is_first_compile: bool,
    pub time: u64,
    pub stats: PluginGenerateStats,
}

#[derive(Clone)]
pub struct PluginGenerateStats {
    pub start_time: u64,
    pub end_time: u64,
}

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;

    fn modify_config(&self, _config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        Ok(())
    }

    fn load(&self, _param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        Ok(None)
    }

    fn next_build(&self, _next_build_param: &NextBuildParam) -> bool {
        true
    }

    fn parse(
        &self,
        _param: &PluginParseParam,
        _context: &Arc<Context>,
    ) -> Result<Option<ModuleAst>> {
        Ok(None)
    }

    #[allow(dead_code)]
    fn transform_js(
        &self,
        _param: &PluginTransformJsParam,
        _ast: &mut Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn after_generate_transform_js(
        &self,
        _param: &PluginTransformJsParam,
        _ast: &mut Module,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
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

    fn build_start(&self, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
    }

    fn generate_end(
        &self,
        _params: &PluginGenerateEndParams,
        _context: &Arc<Context>,
    ) -> Result<Option<()>> {
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

    fn optimize_chunk(
        &self,
        _chunk_graph: &mut ChunkGraph,
        _module_graph: &mut ModuleGraph,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn before_write_fs(&self, _path: &Path, _content: &[u8]) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct PluginDriver {
    plugins: Vec<Arc<dyn Plugin>>,
}

pub struct NextBuildParam<'a> {
    pub current_module: &'a ModuleId,
    pub next_file: &'a File,
}

impl PluginDriver {
    pub fn next_build(&self, param: &NextBuildParam) -> bool {
        self.plugins.iter().all(|p| p.next_build(param))
    }
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

    #[allow(dead_code)]
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

    pub fn after_generate_transform_js(
        &self,
        param: &PluginTransformJsParam,
        ast: &mut Module,
        context: &Arc<Context>,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin.after_generate_transform_js(param, ast, context)?;
        }
        Ok(())
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn build_start(&self, context: &Arc<Context>) -> Result<Option<()>> {
        for plugin in &self.plugins {
            plugin.build_start(context)?;
        }
        Ok(None)
    }

    pub fn generate_end(
        &self,
        param: &PluginGenerateEndParams,
        context: &Arc<Context>,
    ) -> Result<Option<()>> {
        for plugin in &self.plugins {
            plugin.generate_end(param, context)?;
        }
        Ok(None)
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

    pub fn optimize_chunk(
        &self,
        chunk_graph: &mut ChunkGraph,
        module_graph: &mut ModuleGraph,
        context: &Arc<Context>,
    ) -> Result<()> {
        for p in &self.plugins {
            p.optimize_chunk(chunk_graph, module_graph, context)?;
        }

        Ok(())
    }

    pub fn before_write_fs<P: AsRef<Path>, C: AsRef<[u8]>>(
        &self,
        path: P,
        content: C,
    ) -> Result<()> {
        for p in &self.plugins {
            p.before_write_fs(path.as_ref(), content.as_ref())?;
        }

        Ok(())
    }
}
