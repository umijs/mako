use std::any::Any;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
use swc_core::common::errors::Handler;
use swc_core::common::Mark;
use swc_core::ecma::ast::Module;

use crate::ast::file::{Content, File};
use crate::compiler::{Args, Compiler, Context};
use crate::config::Config;
use crate::generate::chunk_graph::ChunkGraph;
use crate::generate::generate_chunks::ChunkFile;
use crate::module::{Dependency, ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;
use crate::resolve::ResolverResource;
use crate::stats::StatsJsonMap;

#[derive(Debug)]
pub struct PluginLoadParam<'a> {
    pub file: &'a File,
}

#[derive(Debug)]
pub struct PluginResolveIdParams {
    pub is_entry: bool,
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginGenerateEndParams {
    pub is_first_compile: bool,
    pub time: i64,
    pub stats: StatsJsonMap,
}

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;

    fn enforce(&self) -> Option<&str> {
        None
    }

    fn modify_config(&self, _config: &mut Config, _root: &Path, _args: &Args) -> Result<()> {
        Ok(())
    }

    fn load(&self, _param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        Ok(None)
    }

    fn load_transform(
        &self,
        _content: &mut Content,
        _path: &str,
        _context: &Arc<Context>,
    ) -> Result<Option<Content>> {
        Ok(None)
    }

    fn resolve_id(
        &self,
        _source: &str,
        _importer: &str,
        _params: &PluginResolveIdParams,
        _context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
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

    fn after_build(&self, _context: &Arc<Context>, _compiler: &Compiler) -> Result<()> {
        Ok(())
    }

    fn after_generate_chunk_files(
        &self,
        _chunk_files: &[ChunkFile],
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn build_success(&self, _stats: &StatsJsonMap, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn build_start(&self, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn generate_begin(&self, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn generate_end(
        &self,
        _params: &PluginGenerateEndParams,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn write_bundle(&self, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn watch_changes(&self, _id: &str, _event: &str, _context: &Arc<Context>) -> Result<()> {
        Ok(())
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn hmr_runtime_updates(&self, _context: &Arc<Context>) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn optimize_module_graph(
        &self,
        _module_graph: &mut ModuleGraph,
        _context: &Arc<Context>,
    ) -> Result<()> {
        Ok(())
    }

    fn before_optimize_chunk(&self, _context: &Arc<Context>) -> Result<()> {
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

    fn after_update(&self, _compiler: &Compiler) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct PluginDriver {
    plugins: Vec<Arc<dyn Plugin>>,
}

#[derive(Debug)]
pub struct NextBuildParam<'a> {
    pub current_module: &'a ModuleId,
    pub next_file: &'a File,
    pub resource: &'a ResolverResource,
}

impl PluginDriver {
    pub fn new(plugins: Vec<Arc<dyn Plugin>>) -> Self {
        Self { plugins }
    }

    pub fn next_build(&self, param: &NextBuildParam) -> bool {
        self.plugins.iter().all(|p| p.next_build(param))
    }

    pub fn after_build(&self, context: &Arc<Context>, c: &Compiler) -> Result<()> {
        for p in &self.plugins {
            p.after_build(context, c)?
        }

        Ok(())
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

    pub fn resolve_id(
        &self,
        source: &str,
        importer: &str,
        params: &PluginResolveIdParams,
        context: &Arc<Context>,
    ) -> Result<Option<ResolverResource>> {
        for plugin in &self.plugins {
            let ret = plugin.resolve_id(source, importer, params, context)?;
            if ret.is_some() {
                return Ok(ret);
            }
        }
        Ok(None)
    }

    pub fn before_generate(&self, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.generate_begin(context)?;
        }
        Ok(())
    }

    pub(crate) fn after_generate_chunk_files(
        &self,
        chunk_files: &[ChunkFile],
        context: &Arc<Context>,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin.after_generate_chunk_files(chunk_files, context)?;
        }

        Ok(())
    }

    pub fn build_start(&self, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.build_start(context)?;
        }
        Ok(())
    }

    pub fn generate_end(
        &self,
        params: &PluginGenerateEndParams,
        context: &Arc<Context>,
    ) -> Result<()> {
        for plugin in &self.plugins {
            plugin.generate_end(params, context)?;
        }
        Ok(())
    }

    pub fn write_bundle(&self, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.write_bundle(context)?;
        }
        Ok(())
    }

    pub fn watch_changes(&self, id: &str, event: &str, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.watch_changes(id, event, context)?;
        }
        Ok(())
    }

    pub fn generate_begin(&self, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.generate_begin(context)?;
        }
        Ok(())
    }

    pub fn build_success(&self, stats: &StatsJsonMap, context: &Arc<Context>) -> Result<()> {
        for plugin in &self.plugins {
            plugin.build_success(stats, context)?;
        }
        Ok(())
    }

    pub fn runtime_plugins_code(&self, context: &Arc<Context>) -> Result<String> {
        let mut plugins = Vec::new();
        for plugin in &self.plugins {
            plugins.extend(plugin.runtime_plugins(context)?);
        }
        Ok(plugins.join("\n"))
    }

    pub fn hmr_runtime_update_code(&self, context: &Arc<Context>) -> Result<String> {
        let mut plugins = Vec::new();
        for plugin in &self.plugins {
            plugins.extend(plugin.hmr_runtime_updates(context)?);
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

    pub fn before_optimize_chunk(&self, context: &Arc<Context>) -> Result<()> {
        for p in &self.plugins {
            p.before_optimize_chunk(context)?;
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

    pub fn load_transform(
        &self,
        content: &mut Content,
        path: &str,
        context: &Arc<Context>,
    ) -> Result<Content> {
        for plugin in &self.plugins {
            if let Some(transformed) = plugin.load_transform(content, path, context)? {
                *content = transformed;
            }
        }
        Ok(content.clone())
    }

    pub fn after_update(&self, compiler: &Compiler) -> Result<()> {
        for plugin in &self.plugins {
            plugin.after_update(compiler)?;
        }
        Ok(())
    }
}
