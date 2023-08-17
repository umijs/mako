use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use cached::proc_macro::cached;
use indexmap::IndexSet;
use rayon::prelude::*;
use serde::Serialize;
use tracing::debug;

use crate::ast::{css_ast_to_code, js_ast_to_code};
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode, OutputMode};
use crate::generate_chunks::OutputAst;
use crate::minify::minify_js;
use crate::module::{ModuleAst, ModuleId};
use crate::stats::{create_stats_info, log_assets, write_stats};
use crate::update::UpdateResult;

#[derive(Clone)]
pub struct EmitFile {
    pub filename: String,
    pub content: String,
    pub chunk_id: String,
}

impl Compiler {
    pub fn generate_with_plugin_driver(&self) -> Result<()> {
        self.context.plugin_driver.generate(&self.context)?;
        Ok(())
    }

    pub fn generate(&self) -> Result<()> {
        if self.context.config.output.mode == OutputMode::MinifishPrebuild {
            return self.generate_with_plugin_driver();
        }

        debug!("generate");
        let t_generate = Instant::now();
        let t_tree_shaking = Instant::now();
        if matches!(self.context.config.mode, Mode::Production) {
            debug!("tree_shaking");
            self.tree_shaking();
        }
        let t_tree_shaking = t_tree_shaking.elapsed();
        let t_group_chunks = Instant::now();
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        // 为啥单独提前 transform modules？
        // 因为放 chunks 的循环里，一个 module 可能存在于多个 chunk 里，可能会被编译多遍
        let t_transform_modules = Instant::now();
        debug!("transform all modules");
        self.transform_all()?;
        let t_transform_modules = t_transform_modules.elapsed();

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path)?;
        }

        // generate chunks
        let t_generate_chunks = Instant::now();
        debug!("generate chunks");
        let mut chunk_asts = self.generate_chunks_ast()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        // minify
        let t_minify = Instant::now();
        debug!("minify");
        if self.context.config.minify {
            chunk_asts
                .par_iter_mut()
                .try_for_each(|file| -> Result<()> {
                    if matches!(self.context.config.mode, Mode::Production) {
                        match &mut file.ast {
                            ModuleAst::Script(ast) => {
                                minify_js(ast, &self.context)?;
                            }
                            ModuleAst::Css(ast) => {
                                swc_css_minifier::minify(ast, Default::default());
                            }
                            _ => (),
                        }
                    }
                    Ok(())
                })?;
        }
        let t_minify = t_minify.elapsed();

        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        chunk_asts.par_iter().try_for_each(|file| -> Result<()> {
            for file in get_chunk_emit_files(file, &self.context)? {
                self.write_to_dist_with_stats(file);
            }

            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        // write assets
        let t_write_assets = Instant::now();
        debug!("write assets");
        // why {} block? unlock assets_info
        {
            let assets_info = &(*self.context.assets_info.lock().unwrap());
            for (k, v) in assets_info {
                let asset_path = &self.context.root.join(k);
                let asset_output_path = &config.output.path.join(v);
                if asset_path.exists() {
                    fs::copy(asset_path, asset_output_path)?;
                } else {
                    panic!("asset not found: {}", asset_path.display());
                }
            }
        }
        let t_write_assets = t_write_assets.elapsed();

        // copy
        let t_copy = Instant::now();
        debug!("copy");
        self.copy()?;
        let t_copy = t_copy.elapsed();

        // generate stats
        let stats = create_stats_info(0, self);
        if self.context.config.stats {
            write_stats(&stats, self);
        }

        // build_success hook
        self.context
            .plugin_driver
            .build_success(&stats, &self.context)?;

        // log assets
        log_assets(self);

        debug!("generate done in {}ms", t_generate.elapsed().as_millis());
        debug!("  - tree shaking: {}ms", t_tree_shaking.as_millis());
        debug!("  - group chunks: {}ms", t_group_chunks.as_millis());
        debug!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
        debug!("  - generate chunks: {}ms", t_generate_chunks.as_millis());
        debug!("  - minify: {}ms", t_minify.as_millis());
        debug!(
            "  - ast to code and write: {}ms",
            t_ast_to_code_and_write.as_millis()
        );
        debug!("  - write assets: {}ms", t_write_assets.as_millis());
        debug!("  - copy: {}ms", t_copy.as_millis());

        Ok(())
    }

    pub fn emit_dev_chunks(&self) -> Result<()> {
        debug!("generate(hmr-fullbuild)");

        let t_generate = Instant::now();

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path)?;
        }

        // generate chunks
        let t_generate_chunks = Instant::now();
        let chunk_asts = self.generate_chunks_ast()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        chunk_asts.par_iter().try_for_each(|file| -> Result<()> {
            for file in get_chunk_emit_files(file, &self.context)? {
                self.write_to_dist_with_stats(file);
            }

            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        // write assets
        let t_write_assets = Instant::now();
        debug!("write assets");
        let assets_info = &(*self.context.assets_info.lock().unwrap());
        for (k, v) in assets_info {
            let asset_path = &self.context.root.join(k);
            let asset_output_path = &config.output.path.join(v);
            if asset_path.exists() {
                fs::copy(asset_path, asset_output_path)?;
            } else {
                panic!("asset not found: {}", asset_path.display());
            }
        }
        let t_write_assets = t_write_assets.elapsed();

        // copy
        let t_copy = Instant::now();
        debug!("copy");
        self.copy()?;
        let t_copy = t_copy.elapsed();

        let t_generate = t_generate.elapsed();

        debug!(
            "generate(hmr-fullbuild) done in {}ms",
            t_generate.as_millis()
        );
        debug!("  - generate chunks: {}ms", t_generate_chunks.as_millis());
        debug!(
            "  - ast to code and write: {}ms",
            t_ast_to_code_and_write.as_millis()
        );
        debug!("  - write assets: {}ms", t_write_assets.as_millis());
        debug!("  - copy: {}ms", t_copy.as_millis());

        Ok(())
    }

    // TODO: 集成到 fn generate 里
    pub fn generate_hot_update_chunks(
        &self,
        updated_modules: UpdateResult,
        last_full_hash: u64,
    ) -> Result<u64> {
        debug!("generate_hot_update_chunks start");

        let last_chunk_names: HashSet<String> = {
            let chunk_graph = self.context.chunk_graph.read().unwrap();
            chunk_graph.chunk_names()
        };

        debug!("hot-update:generate");

        let t_generate = Instant::now();
        let t_group_chunks = Instant::now();
        // TODO 不需要重新构建 graph
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        let t_transform_modules = Instant::now();
        self.transform_for_change(&updated_modules)?;
        let t_transform_modules = t_transform_modules.elapsed();

        let t_calculate_hash = Instant::now();
        let current_full_hash = self.full_hash();
        let t_calculate_hash = t_calculate_hash.elapsed();

        debug!(
            "{} {} {}",
            current_full_hash,
            if current_full_hash == last_full_hash {
                "equals"
            } else {
                "not equals"
            },
            last_full_hash
        );

        if current_full_hash == last_full_hash {
            return Ok(current_full_hash);
        }

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path).unwrap();
        }

        let (current_chunks, modified_chunks) = {
            let cg = self.context.chunk_graph.read().unwrap();

            let chunk_names = cg.chunk_names();

            let modified_chunks: Vec<String> = cg
                .get_chunks()
                .iter()
                .filter(|c| {
                    updated_modules
                        .modified
                        .iter()
                        .any(|m_id| c.has_module(m_id))
                })
                .map(|c| c.filename())
                .collect();

            (chunk_names, modified_chunks)
        };

        let removed_chunks: Vec<String> = last_chunk_names
            .difference(&current_chunks)
            .cloned()
            .collect();

        let t_generate_hmr_chunk = Instant::now();
        let cg = self.context.chunk_graph.read().unwrap();
        for chunk_name in &modified_chunks {
            if let Some(chunk) = cg.get_chunk_by_name(chunk_name) {
                let modified_ids: IndexSet<ModuleId> =
                    IndexSet::from_iter(updated_modules.modified.iter().cloned());
                let (code, ..) =
                    self.generate_hmr_chunk(chunk, &modified_ids, current_full_hash)?;
                // TODO the final format should be {name}.{full_hash}.hot-update.{ext}
                self.write_to_dist(to_hot_update_chunk_name(chunk_name, last_full_hash), code);
            }
        }
        let t_generate_hmr_chunk = t_generate_hmr_chunk.elapsed();

        self.write_to_dist(
            format!("{}.hot-update.json", last_full_hash),
            serde_json::to_string(&HotUpdateManifest {
                removed_chunks,
                modified_chunks,
            })
            .unwrap(),
        );

        debug!(
            "generate(hmr) done in {}ms",
            t_generate.elapsed().as_millis()
        );
        debug!("  - group chunks: {}ms", t_group_chunks.as_millis());
        debug!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
        debug!("  - calculate hash: {}ms", t_calculate_hash.as_millis());
        debug!(
            "  - generate hmr chunk: {}ms",
            t_generate_hmr_chunk.as_millis()
        );
        debug!("  - next full hash: {}", current_full_hash);

        Ok(current_full_hash)
    }

    pub fn write_to_dist<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(
        &self,
        filename: P,
        content: C,
    ) {
        let to = self.context.config.output.path.join(filename);

        std::fs::write(to, content).unwrap();
    }
    // 写入产物前记录 content 大小
    pub fn write_to_dist_with_stats(&self, file: EmitFile) {
        let to: PathBuf = self.context.config.output.path.join(file.filename.clone());
        let size = file.content.len() as u64;
        self.context.stats_info.lock().unwrap().add_assets(
            size,
            file.filename,
            file.chunk_id,
            to.clone(),
        );
        fs::write(to, file.content).unwrap();
    }
}

fn to_hot_update_chunk_name(chunk_name: &String, hash: u64) -> String {
    match chunk_name.rsplit_once('.') {
        None => {
            format!("{chunk_name}.{hash}.hot-update")
        }
        Some((left, ext)) => {
            format!("{left}.{hash}.hot-update.{ext}")
        }
    }
}

#[cached(
    result = true,
    key = "String",
    convert = r#"{ format!("{}-{}", file.module_hash, file.path) }"#
)]
fn get_chunk_emit_files(file: &OutputAst, context: &Arc<Context>) -> Result<Vec<EmitFile>> {
    let mut files = vec![];
    match &file.ast {
        ModuleAst::Script(ast) => {
            // ast to code
            let (js_code, js_sourcemap) = js_ast_to_code(&ast.ast, context, &file.path)?;
            // generate code and sourcemap files
            files.push(EmitFile {
                filename: file.path.clone(),
                content: js_code,
                chunk_id: file.chunk_id.clone(),
            });
            if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
                files.push(EmitFile {
                    filename: format!("{}.map", file.path.clone()),
                    content: js_sourcemap,
                    chunk_id: "".to_string(),
                });
            }
        }
        ModuleAst::Css(ast) => {
            // ast to code
            let (css_code, css_sourcemap) = css_ast_to_code(ast, context, &file.path);
            files.push(EmitFile {
                filename: file.path.clone(),
                content: css_code,
                chunk_id: file.chunk_id.clone(),
            });
            if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
                files.push(EmitFile {
                    filename: format!("{}.map", file.path.clone()),
                    content: css_sourcemap,
                    chunk_id: "".to_string(),
                });
            }
        }
        _ => (),
    }

    Ok(files)
}

#[derive(Serialize)]
struct HotUpdateManifest {
    #[serde(rename(serialize = "c"))]
    modified_chunks: Vec<String>,

    #[serde(rename(serialize = "r"))]
    removed_chunks: Vec<String>,
    // TODO
    // #[serde(rename(serialize = "c"))]
    // removed_modules: Vec<String>,
}
