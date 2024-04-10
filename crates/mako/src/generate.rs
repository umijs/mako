use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use mako_core::anyhow::{anyhow, Result};
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::serde::Serialize;
use mako_core::tracing::debug;

use crate::ast::utils::base64_encode;
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, OutputMode, TreeShakingStrategy};
use crate::generate_chunks::{ChunkFile, ChunkFileType};
use crate::module::{Dependency, ModuleId};
use crate::stats::{create_stats_info, print_stats, write_stats};
use crate::update::UpdateResult;
use crate::visitors::async_module::mark_async;

#[derive(Clone)]
pub struct EmitFile {
    pub filename: String,
    pub content: String,
    pub chunk_id: String,
    pub hashname: String,
}

impl Compiler {
    fn generate_with_plugin_driver(&self) -> Result<()> {
        self.context.plugin_driver.generate(&self.context)?;

        let stats = create_stats_info(0, self);

        self.context
            .plugin_driver
            .build_success(&stats, &self.context)?;
        Ok(())
    }

    fn mark_async(&self) -> HashMap<ModuleId, Vec<Dependency>> {
        let module_ids = {
            let module_graph = self.context.module_graph.read().unwrap();
            let (mut module_ids, _) = module_graph.toposort();
            // start from the leaf nodes, so reverser the sort
            module_ids.reverse();
            drop(module_graph);
            module_ids
        };
        mark_async(&module_ids, &self.context)
    }

    pub fn generate(&self) -> Result<()> {
        debug!("generate");
        let t_generate = Instant::now();

        debug!("tree_shaking");
        let t_tree_shaking = Instant::now();

        let async_dep_map = self.mark_async();

        // Disable tree shaking in watch mode temporarily
        // ref: https://github.com/umijs/mako/issues/396
        if !self.context.args.watch {
            match self.context.config._tree_shaking {
                Some(TreeShakingStrategy::Basic) => {
                    let mut module_graph = self.context.module_graph.write().unwrap();

                    mako_core::mako_profile_scope!("tree shake");
                    self.context
                        .plugin_driver
                        .optimize_module_graph(module_graph.deref_mut(), &self.context)?;
                    let t_tree_shaking = t_tree_shaking.elapsed();
                    println!("basic optimize in {}ms.", t_tree_shaking.as_millis());
                }
                Some(TreeShakingStrategy::Advanced) => {
                    // waiting @heden8 to come back
                }
                None => {}
            }
        }
        let t_tree_shaking = t_tree_shaking.elapsed();

        // TODO: improve this hardcode
        if self.context.config.output.mode == OutputMode::Bundless {
            return self.generate_with_plugin_driver();
        }

        let t_group_chunks = Instant::now();
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        let t_optimize_chunks = Instant::now();
        self.optimize_chunk();
        let t_optimize_chunks = t_optimize_chunks.elapsed();

        {
            let mut module_graph = self.context.module_graph.write().unwrap();
            let mut chunk_graph = self.context.chunk_graph.write().unwrap();

            self.context.plugin_driver.optimize_chunk(
                &mut chunk_graph,
                &mut module_graph,
                &self.context,
            )?;
        }

        // 为啥单独提前 transform modules？
        // 因为放 chunks 的循环里，一个 module 可能存在于多个 chunk 里，可能会被编译多遍
        let t_transform_modules = Instant::now();
        debug!("transform all modules");
        self.transform_all(async_dep_map)?;
        let t_transform_modules = t_transform_modules.elapsed();

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path)?;
        }

        let full_hash = self.full_hash();
        let (t_generate_chunks, t_ast_to_code_and_write) = self.write_chunk_files(full_hash)?;

        // write assets
        if config.emit_assets {
            let t_write_assets = Instant::now();
            debug!("write assets");
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
            debug!("  - write assets: {}ms", t_write_assets.as_millis());
        }

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

        Ok(())
    }

    fn write_chunk_files(&self, full_hash: u64) -> Result<(Duration, Duration)> {
        // generate chunks
        let t_generate_chunks = Instant::now();
        debug!("generate chunks");
        let chunk_files = self.generate_chunk_files(full_hash)?;
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
        mako_core::mako_profile_function!();
        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        chunk_files.par_iter().try_for_each(|file| -> Result<()> {
            write_dev_chunk_file(&self.context, file)?;
            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        Ok(t_ast_to_code_and_write)
    }

    pub fn emit_chunk_file(&self, chunk_file: &ChunkFile) {
        emit_chunk_file(&self.context, chunk_file);
    }

    pub fn emit_dev_chunks(&self, hmr_hash: u64) -> Result<()> {
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
        let chunk_files = self.generate_chunk_files(hmr_hash)?;
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
        last_snapshot_hash: u64,
        last_hmr_hash: u64,
    ) -> Result<(u64, u64)> {
        debug!("generate_hot_update_chunks start");

        let last_chunk_names: HashSet<String> = {
            let chunk_graph = self.context.chunk_graph.read().unwrap();
            chunk_graph.chunk_names()
        };

        debug!("hot-update:generate");

        let t_generate = Instant::now();
        let t_group_chunks = Instant::now();
        let group_result = self.group_hot_update_chunk(&updated_modules);
        let t_group_chunks = t_group_chunks.elapsed();

        let t_optimize_chunks = Instant::now();
        self.optimize_hot_update_chunk(&group_result);
        let t_optimize_chunks = t_optimize_chunks.elapsed();

        let t_transform_modules = Instant::now();
        self.transform_for_change(&updated_modules)?;
        let t_transform_modules = t_transform_modules.elapsed();

        let t_calculate_hash = Instant::now();
        let current_snapshot_hash = self.full_hash();
        let current_hmr_hash = last_hmr_hash.wrapping_add(current_snapshot_hash);
        let t_calculate_hash = t_calculate_hash.elapsed();

        debug!(
            "{} {} {}",
            current_snapshot_hash,
            if current_snapshot_hash == last_snapshot_hash {
                "equals"
            } else {
                "not equals"
            },
            last_snapshot_hash
        );

        if current_snapshot_hash == last_snapshot_hash {
            return Ok((current_snapshot_hash, current_hmr_hash));
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
                    let is_modified = updated_modules
                        .modified
                        .iter()
                        .any(|m_id| c.has_module(m_id));
                    let is_added = updated_modules.added.iter().any(|m_id| c.has_module(m_id));
                    is_modified || is_added
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
            let filename = to_hot_update_chunk_name(chunk_name, last_hmr_hash);

            if let Some(chunk) = cg.get_chunk_by_name(chunk_name) {
                let modified_ids: IndexSet<ModuleId> =
                    IndexSet::from_iter(updated_modules.modified.iter().cloned());
                let added_ids: IndexSet<ModuleId> =
                    IndexSet::from_iter(updated_modules.added.iter().cloned());
                let merged_ids: IndexSet<ModuleId> =
                    modified_ids.union(&added_ids).cloned().collect();
                let (code, sourcemap) =
                    self.generate_hmr_chunk(chunk, &filename, &merged_ids, current_hmr_hash)?;
                // TODO the final format should be {name}.{full_hash}.hot-update.{ext}
                self.write_to_dist(&filename, code);
                self.write_to_dist(format!("{}.map", &filename), sourcemap);
            }
        }
        let t_generate_hmr_chunk = t_generate_hmr_chunk.elapsed();

        self.write_to_dist(
            format!("{}.hot-update.json", last_hmr_hash),
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
        debug!("  - next full hash: {}", current_snapshot_hash);

        Ok((current_snapshot_hash, current_hmr_hash))
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

fn write_dev_chunk_file(context: &Arc<Context>, chunk: &ChunkFile) -> Result<()> {
    mako_core::mako_profile_function!();

    if let Some(source_map) = &chunk.source_map {
        context.write_static_content(
            chunk.source_map_disk_name(),
            source_map.clone(),
            chunk.raw_hash,
        )?;

        let source_map_url_line = match chunk.file_type {
            ChunkFileType::JS => {
                format!("\n//# sourceMappingURL={}", chunk.source_map_disk_name())
            }
            ChunkFileType::Css => {
                format!("\n/*# sourceMappingURL={}*/", chunk.source_map_disk_name())
            }
        };

        let mut code = Vec::new();

        code.extend_from_slice(&chunk.content);
        code.extend_from_slice(source_map_url_line.as_bytes());

        context.write_static_content(chunk.disk_name(), code, chunk.raw_hash)?;
    } else {
        context.write_static_content(chunk.disk_name(), chunk.content.clone(), chunk.raw_hash)?;
    }

    Ok(())
}

fn emit_chunk_file(context: &Arc<Context>, chunk_file: &ChunkFile) {
    mako_core::mako_profile_function!(&chunk_file.file_name);

    let to: PathBuf = context.config.output.path.join(chunk_file.disk_name());

    match context.config.devtool {
        Some(DevtoolConfig::SourceMap) => {
            let mut code = Vec::new();
            code.extend_from_slice(&chunk_file.content);

            if let Some(source_map) = &chunk_file.source_map {
                let size = source_map.len() as u64;
                context.stats_info.lock().unwrap().add_assets(
                    size,
                    chunk_file.source_map_name(),
                    chunk_file.chunk_id.clone(),
                    to.clone(),
                    chunk_file.source_map_disk_name(),
                );
                fs::write(
                    context
                        .config
                        .output
                        .path
                        .join(chunk_file.source_map_disk_name()),
                    source_map,
                )
                .unwrap();

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
                code.extend_from_slice(source_map_url_line.as_bytes());
            }

            let size = code.len() as u64;
            context.stats_info.lock().unwrap().add_assets(
                size,
                chunk_file.file_name.clone(),
                chunk_file.chunk_id.clone(),
                to.clone(),
                chunk_file.disk_name(),
            );
            fs::write(to, &code).unwrap();
        }
        Some(DevtoolConfig::InlineSourceMap) => {
            let mut code = Vec::new();
            code.extend_from_slice(&chunk_file.content);

            if let Some(source_map) = &chunk_file.source_map {
                code.extend_from_slice(
                    format!(
                        "\n//# sourceMappingURL=data:application/json;charset=utf-8;base64,{}",
                        base64_encode(source_map)
                    )
                    .as_bytes(),
                );
            }

            let size = code.len() as u64;
            context.stats_info.lock().unwrap().add_assets(
                size,
                chunk_file.file_name.clone(),
                chunk_file.chunk_id.clone(),
                to.clone(),
                chunk_file.disk_name(),
            );
            fs::write(to, code).unwrap();
        }
        None => {
            context.stats_info.lock().unwrap().add_assets(
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
