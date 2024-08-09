use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use parking_lot::Mutex;

use crate::ast::file::Content;
use crate::compiler::Context;
use crate::plugin::{Plugin, PluginLoadParam};

/**
 * 插件执行顺序 3 ~ 7 会重复执行
 * 1. modify_config
 * 2. build_start
 * 3. load
 * 4. parse
 * 5. transform_js
 * 6. before_resolve
 * 7. next_build
 * 8. after_build
 * 9. generate_begin
 * 10. optimize_module_graph
 * 11. before_optimize_chunk
 * 12. optimize_chunk
 * 13. runtime_plugins
 * 14. after_generate_chunk_files
 * 15. build_success
 * after_generate_transform_js、before_write_fs 仅 mode bundless 执行
 */

pub struct ProgressPluginOptions {
    pub prefix: String,
    pub template: String,
    pub progress_chars: String,
}

pub struct ProgressPlugin {
    options: ProgressPluginOptions,
    progress_bar: Mutex<Option<ProgressBar>>,
    module_count: Arc<Mutex<u32>>,
    first_build: Mutex<bool>,
    percent: Arc<Mutex<f32>>,
}

impl ProgressPlugin {
    pub fn new(options: ProgressPluginOptions) -> Self {
        Self {
            options,
            progress_bar: Mutex::new(None),
            module_count: Arc::new(Mutex::new(0)),
            first_build: Mutex::new(true),
            percent: Arc::new(Mutex::new(0.1)),
        }
    }

    pub fn init_progress_bar(&self) {
        let progress_bar =
            ProgressBar::with_draw_target(Some(100), ProgressDrawTarget::stdout_with_hz(100));
        let progress_bar_style = ProgressStyle::with_template(&self.options.template)
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .progress_chars(&self.options.progress_chars);
        progress_bar.set_style(progress_bar_style);
        progress_bar.enable_steady_tick(Duration::from_millis(200));
        progress_bar.set_prefix(self.options.prefix.clone());
        progress_bar.reset();
        *self.progress_bar.lock() = Some(progress_bar);
    }

    pub fn handler(&self, percent: f32, msg: String, state_items: Vec<String>) {
        self.progress_bar
            .lock()
            .as_ref()
            .unwrap()
            .set_message(msg + " " + state_items.join(" ").as_str());
        self.progress_bar
            .lock()
            .as_ref()
            .unwrap()
            .set_position((percent * 100.0) as u64);
    }

    pub fn increment_module_count(&self) {
        let mut count = self.module_count.lock();
        *count += 1;
    }

    pub fn reset_module_count(&self) {
        let mut count = self.module_count.lock();
        *count = 0;
    }

    pub fn get_module_count(&self) -> u32 {
        let count = self.module_count.lock();
        *count
    }

    pub fn increment_percent(&self) {
        let mut percent = self.percent.lock();
        *percent += 0.02;
    }

    pub fn set_percent(&self, val: f32) {
        let mut percent = self.percent.lock();
        *percent = val;
    }

    pub fn get_percent(&self) -> f32 {
        let percent = self.percent.lock();
        *percent
    }
}

impl Plugin for ProgressPlugin {
    fn name(&self) -> &str {
        "progress_plugin"
    }

    fn build_start(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.init_progress_bar();
        self.handler(
            0.1,
            "build start".to_string(),
            vec!["build start".to_string()],
        );
        self.reset_module_count();
        Ok(())
    }

    fn load(&self, param: &PluginLoadParam, _context: &Arc<Context>) -> Result<Option<Content>> {
        self.increment_module_count();
        let count = self.get_module_count() as f32;
        let path: String = param.file.path.to_string_lossy().to_string();
        let first_build = self.first_build.lock();

        if *first_build {
            self.set_percent(0.2);
        } else {
            self.increment_percent();
        }

        self.handler(
            self.get_percent().min(0.6),
            "load".to_string(),
            vec![format!("transform ({count}) {path}")],
        );

        Ok(None)
    }

    fn parse(
        &self,
        _param: &crate::plugin::PluginParseParam,
        _context: &Arc<Context>,
    ) -> anyhow::Result<Option<crate::module::ModuleAst>> {
        let first_build = self.first_build.lock();
        if *first_build {
            self.increment_percent();
            self.handler(
                self.get_percent(),
                "parse".to_string(),
                vec!["parse".to_string()],
            );
        }
        Ok(None)
    }

    fn transform_js(
        &self,
        _param: &crate::plugin::PluginTransformJsParam,
        _ast: &mut swc_core::ecma::ast::Module,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        let first_build = self.first_build.lock();
        if *first_build {
            self.increment_percent();
            self.handler(
                self.get_percent(),
                "transform_js".to_string(),
                vec!["transform_js".to_string()],
            );
        }
        Ok(())
    }

    fn before_resolve(
        &self,
        _deps: &mut Vec<crate::module::Dependency>,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        let first_build = self.first_build.lock();
        if *first_build {
            self.increment_percent();
            self.handler(
                self.get_percent(),
                "before resolve".to_string(),
                vec!["before resolve".to_string()],
            );
        }
        Ok(())
    }

    fn next_build(&self, _next_build_param: &crate::plugin::NextBuildParam) -> bool {
        let first_build = self.first_build.lock();
        if *first_build {
            self.increment_percent();
            self.handler(
                self.get_percent(),
                "next build".to_string(),
                vec!["next build".to_string()],
            );
        }
        true
    }

    fn after_build(
        &self,
        _context: &Arc<Context>,
        _compiler: &crate::compiler::Compiler,
    ) -> anyhow::Result<()> {
        let mut first_build = self.first_build.lock();
        *first_build = false;

        self.reset_module_count();

        self.handler(0.62, "build".to_string(), vec!["after build".to_string()]);
        Ok(())
    }

    fn generate_begin(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.handler(
            0.65,
            "generate begin".to_string(),
            vec!["generate begin".to_string()],
        );
        Ok(())
    }

    fn optimize_module_graph(
        &self,
        _module_graph: &mut crate::module_graph::ModuleGraph,
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.7,
            "optimize module graph".to_string(),
            vec!["optimize module graph".to_string()],
        );
        Ok(())
    }

    fn before_optimize_chunk(&self, _context: &Arc<Context>) -> anyhow::Result<()> {
        self.handler(
            0.8,
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
            0.85,
            "optimize chunk".to_string(),
            vec!["optimize chunk".to_string()],
        );
        Ok(())
    }

    fn runtime_plugins(&self, _context: &Arc<Context>) -> anyhow::Result<Vec<String>> {
        self.handler(
            0.9,
            "runtime plugins".to_string(),
            vec!["runtime plugins".to_string()],
        );
        Ok(Vec::new())
    }

    fn after_generate_chunk_files(
        &self,
        _chunk_files: &[crate::generate::generate_chunks::ChunkFile],
        _context: &Arc<Context>,
    ) -> anyhow::Result<()> {
        self.handler(
            0.95,
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
        self.progress_bar
            .lock()
            .as_ref()
            .unwrap()
            .finish_with_message("Compiled successfully".to_string());
        Ok(())
    }
}
