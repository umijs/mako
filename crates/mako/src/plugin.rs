use std::any::Any;
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::build::FileRequest;
use crate::compiler::Context;
use crate::config::Config;
use crate::load::Content;
use crate::module::{Dependency, ModuleAst};
use crate::stats::StatsJsonMap;

#[derive(Debug)]
pub struct PluginLoadParam {
    pub path: String,
    pub is_entry: bool,
    pub ext_name: String,
}

pub struct PluginParseParam<'a> {
    pub request: &'a FileRequest,
    pub content: &'a Content,
}

pub struct PluginDepAnalyzeParam<'a> {
    pub ast: &'a ModuleAst,
    pub deps: Vec<Dependency>,
}

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &str;

    fn modify_config(&self, _config: &mut Config) -> Result<()> {
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

    fn analyze_deps(&self, _ast: &mut PluginDepAnalyzeParam) -> Result<()> {
        Ok(())
    }

    fn generate(&self, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
    }

    fn build_success(&self, _stats: &StatsJsonMap, _context: &Arc<Context>) -> Result<Option<()>> {
        Ok(None)
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

    pub fn modify_config(&self, config: &mut Config) -> Result<()> {
        for plugin in &self.plugins {
            plugin.modify_config(config)?;
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

    pub fn analyze_deps(&self, param: &mut PluginDepAnalyzeParam) -> Result<()> {
        for plugin in &self.plugins {
            plugin.analyze_deps(param)?;
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
}
