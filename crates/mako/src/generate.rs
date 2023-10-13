use std::collections::HashSet;
use std::fs;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cached::proc_macro::cached;
use mako_core::anyhow::{anyhow, Result};
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::serde::Serialize;
use mako_core::tracing::debug;

use crate::ast::base64_encode;
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, OutputMode, TreeShakeStrategy};
use crate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::module::ModuleId;
use crate::stats::{create_stats_info, print_stats, write_stats};
use crate::update::UpdateResult;

#[derive(Clone)]
pub struct EmitFile {
    pub filename: String,
    pub content: String,
    pub chunk_id: String,
    pub hashname: String,
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
        debug!("tree_shaking");
        // Disable tree shaking in watch mode temporarily
        // ref: https://github.com/umijs/mako/issues/396
        if !self.context.args.watch {
            match self.context.config.tree_shake {
                TreeShakeStrategy::Basic => {
                    let mut module_graph = self.context.module_graph.write().unwrap();

                    mako_core::mako_profile_scope!("tree shake");
                    self.context
                        .plugin_driver
                        .optimize_module_graph(module_graph.deref_mut())?;
                    let t_tree_shaking = t_tree_shaking.elapsed();
                    println!("basic optimize in {}ms.", t_tree_shaking.as_millis());
                }
                TreeShakeStrategy::Advanced => {
                    mako_core::mako_profile_scope!("advanced tree shake");
                    let shaking_module_ids = self.tree_shaking();
                    let t_tree_shaking = t_tree_shaking.elapsed();
                    println!(
                        "{} modules removed in {}ms.",
                        shaking_module_ids.len(),
                        t_tree_shaking.as_millis()
                    );
                }
                TreeShakeStrategy::None => {}
            }
        }
        let t_tree_shaking = t_tree_shaking.elapsed();
        let t_group_chunks = Instant::now();
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        let t_optimize_chunks = Instant::now();
        self.optimize_chunk();
        let t_optimize_chunks = t_optimize_chunks.elapsed();

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

        let (t_generate_chunks, t_ast_to_code_and_write) = self.write_chunk_files()?;

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
                    return Err(anyhow!("asset not found: {}", asset_path.display()));
                }
            }
        }

        let t_write_assets = t_write_assets.elapsed();

        // generate stats
        let stats = create_stats_info(0, self);
        if self.context.config.stats && !self.context.args.watch {
            write_stats(&stats, self);
        }

        // build_success hook
        self.context
            .plugin_driver
            .build_success(&stats, &self.context)?;

        // print stats
        if !self.context.args.watch {
            print_stats(self);
        }

        debug!("generate done in {}ms", t_generate.elapsed().as_millis());
        debug!("  - tree shaking: {}ms", t_tree_shaking.as_millis());
        debug!("  - group chunks: {}ms", t_group_chunks.as_millis());
        debug!("  - optimize chunks: {}ms", t_optimize_chunks.as_millis());
        debug!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
        debug!("  - generate chunks: {}ms", t_generate_chunks.as_millis());
        debug!(
            "  - ast to code and write: {}ms",
            t_ast_to_code_and_write.as_millis()
        );
        debug!("  - write assets: {}ms", t_write_assets.as_millis());

        Ok(())
    }

    fn write_chunk_files(&self) -> Result<(Duration, Duration)> {
        // generate chunks
        let t_generate_chunks = Instant::now();
        debug!("generate chunks");
        let chunk_files = self.generate_chunk_files()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        let t_ast_to_code_and_write = if self.context.args.watch {
            self.generate_chunk_mem_file(&chunk_files)?
        } else {
            self.generate_chunk_disk_file(&chunk_files)?
        };

        Ok((t_generate_chunks, t_ast_to_code_and_write))
    }

    fn generate_chunk_disk_file(&self, chunk_files: &Vec<ChunkFile>) -> Result<Duration> {
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        chunk_files.par_iter().try_for_each(|file| -> Result<()> {
            self.emit_chunk_file(file);
            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        Ok(t_ast_to_code_and_write)
    }

    fn generate_chunk_mem_file(&self, chunk_files: &Vec<ChunkFile>) -> Result<Duration> {
        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        chunk_files.par_iter().try_for_each(|file| -> Result<()> {
            write_dev_chunk_file(&self.context, file)?;

            if self.context.config.write_to_disk {
                self.emit_chunk_file(file);
            }

            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        Ok(t_ast_to_code_and_write)
    }

    pub fn emit_chunk_file(&self, chunk_file: &ChunkFile) {
        let to: PathBuf = self.context.config.output.path.join(chunk_file.disk_name());

        match self.context.config.devtool {
            DevtoolConfig::SourceMap => {
                {
                    let source_map = &chunk_file.source_map;

                    let size = source_map.len() as u64;
                    self.context.stats_info.lock().unwrap().add_assets(
                        size,
                        chunk_file.source_map_name(),
                        chunk_file.chunk_id.clone(),
                        to.clone(),
                        chunk_file.source_map_disk_name(),
                    );
                    fs::write(
                        self.context
                            .config
                            .output
                            .path
                            .join(chunk_file.source_map_disk_name()),
                        source_map,
                    )
                    .unwrap();
                }

                let source_map_url_line = match chunk_file.file_type {
                    ChunkFileType::JS => {
                        format!(
                            "\n//# sourceMappingURL={}",
                            chunk_file.source_map_disk_name()
                        )
                    }
                    ChunkFileType::Css => {
                        format!(
                            "\n/*# sourceMappingURL={}*/",
                            chunk_file.source_map_disk_name()
                        )
                    }
                };

                let mut code = Vec::new();

                code.extend_from_slice(&chunk_file.content);
                code.extend_from_slice(source_map_url_line.as_bytes());

                let size = code.len() as u64;
                self.context.stats_info.lock().unwrap().add_assets(
                    size,
                    chunk_file.file_name.clone(),
                    chunk_file.chunk_id.clone(),
                    to.clone(),
                    chunk_file.disk_name(),
                );
                fs::write(to, &code).unwrap();
            }
            DevtoolConfig::InlineSourceMap => {
                let source_map = &chunk_file.source_map;

                let mut code = Vec::new();

                code.extend_from_slice(&chunk_file.content);
                code.extend_from_slice(
                    format!(
                        "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                        base64_encode(source_map)
                    )
                    .as_bytes(),
                );

                let size = code.len() as u64;
                self.context.stats_info.lock().unwrap().add_assets(
                    size,
                    chunk_file.file_name.clone(),
                    chunk_file.chunk_id.clone(),
                    to.clone(),
                    chunk_file.disk_name(),
                );
                fs::write(to, code).unwrap();
            }
            DevtoolConfig::None => {
                self.context.stats_info.lock().unwrap().add_assets(
                    chunk_file.content.len() as u64,
                    chunk_file.file_name.clone(),
                    chunk_file.chunk_id.clone(),
                    to.clone(),
                    chunk_file.disk_name(),
                );

                fs::write(to, &chunk_file.content).unwrap();
            }
        }
    }

    pub fn emit_dev_chunks(&self) -> Result<()> {
        mako_core::mako_profile_function!("emit_dev_chunks");

        debug!("generate(hmr-fullbuild)");

        let t_generate = Instant::now();

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path)?;
        }

        // generate chunks
        let t_generate_chunks = Instant::now();
        let chunk_files = self.generate_chunk_files()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        // ast to code and sourcemap, then write
        debug!("ast to code and write");
        let t_ast_to_code_and_write = self.generate_chunk_mem_file(&chunk_files)?;

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

        let t_optimize_chunks = Instant::now();
        self.optimize_chunk();
        let t_optimize_chunks = t_optimize_chunks.elapsed();

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
            let filename = to_hot_update_chunk_name(chunk_name, last_full_hash);

            if let Some(chunk) = cg.get_chunk_by_name(chunk_name) {
                let modified_ids: IndexSet<ModuleId> =
                    IndexSet::from_iter(updated_modules.modified.iter().cloned());
                let (code, sourcemap) =
                    self.generate_hmr_chunk(chunk, &filename, &modified_ids, current_full_hash)?;
                // TODO the final format should be {name}.{full_hash}.hot-update.{ext}
                self.write_to_dist(&filename, code);
                self.write_to_dist(format!("{}.map", &filename), sourcemap);
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
        debug!("  - optimize chunks: {}ms", t_optimize_chunks.as_millis());
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
}

#[cached(result = true, key = "u64", convert = "{chunk.raw_hash}")]
fn write_dev_chunk_file(context: &Arc<Context>, chunk: &ChunkFile) -> Result<()> {
    context.write_static_content(&chunk.disk_name(), chunk.content.clone())
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
