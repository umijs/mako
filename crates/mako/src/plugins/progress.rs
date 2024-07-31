use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use crate::ast::file::Content;
use crate::compiler::{Args, Context};
use crate::config::Config;
use crate::plugin::{Plugin, PluginLoadParam};
pub struct ProgressPluginOptions {
    pub prefix: String,
    pub template: String,
    pub progress_chars: String,
}

pub struct ProgressPlugin {
    // TODO 支持接受回调函数
    pub options: ProgressPluginOptions,
    pub progress_bar: ProgressBar,
}

impl ProgressPlugin {
    pub fn new(options: ProgressPluginOptions) -> Self {
        let progress_bar =
            ProgressBar::with_draw_target(Some(100), ProgressDrawTarget::stdout_with_hz(100));
        let progress_bar_style = ProgressStyle::with_template(&options.template)
            .expect("TODO:")
            .progress_chars(&options.progress_chars);

        progress_bar.set_style(progress_bar_style);
        Self {
            options,
            progress_bar,
        }
    }

    pub fn handler(&self, percent: f32, msg: String, state_items: Vec<String>) {
        self.progress_bar
            .set_message(msg + " " + state_items.join(" ").as_str());
        self.progress_bar.set_position((percent * 100.0) as u64);
    }
}

impl Plugin for ProgressPlugin {
    fn name(&self) -> &str {
        "progress_plugin"
    }

    fn modify_config(
        &self,
        _config: &mut Config,
        _root: &Path,
        _args: &Args,
    ) -> anyhow::Result<()> {
        self.progress_bar.set_prefix(self.options.prefix.clone());
        self.handler(0.01, "setup".to_string(), vec!["modify config".to_string()]);
        Ok(())
    }

    fn load(&self, _param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        self.handler(0.02, "setup".to_string(), vec!["load".to_string()]);
        Ok(None)
    }

    fn next_build(&self, _next_build_param: &crate::plugin::NextBuildParam) -> bool {
        self.handler(0.03, "setup".to_string(), vec!["next build".to_string()]);
        true
    }

    fn parse(
        &self,
        _param: &crate::plugin::PluginParseParam,
        _context: &Arc<Context>,
    ) -> anyhow::Result<Option<crate::module::ModuleAst>> {
        self.handler(0.1, "setup".to_string(), vec!["parse".to_string()]);
        Ok(None)
    }

    fn transform_js(
        &self,
        param: &crate::plugin::PluginTransformJsParam,
        _ast: &mut swc_core::ecma::ast::Module,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.2,
            "transform js".to_string(),
            vec!["transform js".to_string(), param.path.into()],
        );
        Ok(())
    }

    fn after_generate_transform_js(
        &self,
        _param: &crate::plugin::PluginTransformJsParam,
        _ast: &mut swc_core::ecma::ast::Module,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.3,
            "transform js".to_string(),
            vec!["after generate transform js".to_string()],
        );
        Ok(())
    }

    fn before_resolve(
        &self,
        _deps: &mut Vec<crate::module::Dependency>,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.4,
            "resolve".to_string(),
            vec!["before resolve".to_string()],
        );
        Ok(())
    }

    fn after_build(
        &self,
        _context: &Arc<Context>,
        _compiler: &crate::compiler::Compiler,
    ) -> anyhow::Result<()> {
        self.handler(0.5, "build".to_string(), vec!["after build".to_string()]);
        Ok(())
    }

    fn after_generate_chunk_files(
        &self,
        _chunk_files: &[crate::generate::generate_chunks::ChunkFile],
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.6,
            "generate chunk files".to_string(),
            vec!["after generate chunk files".to_string()],
        );
        Ok(())
    }

    fn build_success(
        &self,
        _stats: &crate::stats::StatsJsonMap,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            1.0,
            "build success".to_string(),
            vec!["build success".to_string()],
        );
        Ok(())
    }

    fn build_start(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.handler(
            0.8,
            "build start".to_string(),
            vec!["build start".to_string()],
        );
        Ok(())
    }

    fn generate_begin(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.handler(
            0.9,
            "generate begin".to_string(),
            vec!["generate begin".to_string()],
        );
        Ok(())
    }

    fn generate_end(
        &self,
        _params: &crate::plugin::PluginGenerateEndParams,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.91,
            "generate end".to_string(),
            vec!["generate end".to_string()],
        );

        Ok(())
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        self.handler(
            0.95,
            "runtime plugins".to_string(),
            vec!["runtime plugins".to_string()],
        );
        Ok(Vec::new())
    }

    fn optimize_module_graph(
        &self,
        _module_graph: &mut crate::module_graph::ModuleGraph,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.96,
            "optimize module graph".to_string(),
            vec!["optimize module graph".to_string()],
        );
        Ok(())
    }

    fn before_optimize_chunk(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.handler(
            0.97,
            "before optimize chunk".to_string(),
            vec!["before optimize chunk".to_string()],
        );
        Ok(())
    }

    fn optimize_chunk(
        &self,
        _chunk_graph: &mut crate::generate::chunk_graph::ChunkGraph,
        _module_graph: &mut crate::module_graph::ModuleGraph,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.98,
            "optimize chunk".to_string(),
            vec!["optimize chunk".to_string()],
        );
        Ok(())
    }

    fn before_write_fs(&self, _path: &std::path::Path, _content: &[u8]) -> anyhow::Result<()> {
        self.handler(
            0.99,
            "before write fs".to_string(),
            vec!["before write fs".to_string()],
        );
        Ok(())
    }
}
