use std::collections::HashSet;
use std::fs;
use std::time::Instant;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tracing::info;

use crate::ast::js_ast_to_code;
use crate::compiler::Compiler;
use crate::config::{DevtoolConfig, Mode};
use crate::minify::minify_js;
use crate::update::UpdateResult;

impl Compiler {
    pub fn generate(&self) -> Result<()> {
        info!("generate");
        let t_generate = Instant::now();
        let t_group_chunks = Instant::now();
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        // 为啥单独提前 transform modules？
        // 因为放 chunks 的循环里，一个 module 可能存在于多个 chunk 里，可能会被编译多遍
        let t_transform_modules = Instant::now();
        info!("transform all modules");
        self.transform_all();
        let t_transform_modules = t_transform_modules.elapsed();

        // ensure output dir exists
        let config = &self.context.config;
        if !config.output.path.exists() {
            fs::create_dir_all(&config.output.path)?;
        }

        // generate chunks
        // TODO: 并行
        let t_generate_chunks = Instant::now();
        info!("generate chunks");
        let mut chunk_asts = self.generate_chunks_ast()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        // minify
        let t_minify = Instant::now();
        info!("minify");
        chunk_asts
            .par_iter_mut()
            .try_for_each(|file| -> Result<()> {
                if matches!(self.context.config.mode, Mode::Production) {
                    file.js_ast = minify_js(file.js_ast.clone(), &self.context)?;
                }
                Ok(())
            })?;
        let t_minify = t_minify.elapsed();

        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        info!("ast to code and write");
        chunk_asts.par_iter().try_for_each(|file| -> Result<()> {
            // ast to code
            let (js_code, js_sourcemap) = js_ast_to_code(&file.js_ast, &self.context, &file.path)?;
            // generate code and sourcemap files
            let output = &config.output.path.join(&file.path);
            fs::write(output, js_code).unwrap();
            if matches!(self.context.config.devtool, DevtoolConfig::SourceMap) {
                fs::write(format!("{}.map", output.display()), js_sourcemap).unwrap();
            }
            Ok(())
        })?;
        let t_ast_to_code_and_write = t_ast_to_code_and_write.elapsed();

        // write assets
        let t_write_assets = Instant::now();
        info!("write assets");
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
        info!("copy");
        self.copy()?;
        let t_copy = t_copy.elapsed();

        info!("generate done in {}ms", t_generate.elapsed().as_millis());
        info!("  - group chunks: {}ms", t_group_chunks.as_millis());
        info!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
        info!("  - generate chunks: {}ms", t_generate_chunks.as_millis());
        info!("  - minify: {}ms", t_minify.as_millis());
        info!(
            "  - ast to code and write: {}ms",
            t_ast_to_code_and_write.as_millis()
        );
        info!("  - write assets: {}ms", t_write_assets.as_millis());
        info!("  - copy: {}ms", t_copy.as_millis());

        Ok(())
    }

    // TODO: 集成到 fn generate 里
    pub fn generate_hot_update_chunks(&self, updated_modules: UpdateResult) {
        let last_chunk_names: HashSet<String> = {
            let chunk_graph = self.context.chunk_graph.read().unwrap();
            chunk_graph.chunk_names()
        };

        info!("generate");

        let t_generate = Instant::now();
        let t_group_chunks = Instant::now();
        // TODO 不需要重新构建 graph
        self.group_chunk();
        let t_group_chunks = t_group_chunks.elapsed();

        // 为啥单独提前 transform modules？

        // 因为放 chunks 的循环里，一个 module 可能存在于多个 chunk 里，可能会被编译多遍，
        let t_transform_modules = Instant::now();
        self.transform_all();
        let t_transform_modules = t_transform_modules.elapsed();

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

        let cg = self.context.chunk_graph.read().unwrap();
        for chunk_name in &modified_chunks {
            if let Some(chunk) = cg.get_chunk_by_name(chunk_name) {
                let (code, _) = self.generate_hmr_chunk(chunk, &updated_modules.modified);

                // TODO the final format should be {name}.{full_hash}.hot-update.{ext}
                self.write_to_dist(to_hot_update_chunk_name(chunk_name), code);
            }
        }

        self.write_to_dist(
            "hot-update.json",
            serde_json::to_string(&HotUpdateManifest {
                removed_chunks,
                modified_chunks,
            })
            .unwrap(),
        );

        // copy
        self.copy().unwrap();

        info!(
            "generate(hmr) done in {}ms",
            t_generate.elapsed().as_millis()
        );
        info!("  - group chunks: {}ms", t_group_chunks.as_millis());
        info!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
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

fn to_hot_update_chunk_name(chunk_name: &String) -> String {
    match chunk_name.rsplit_once('.') {
        None => {
            format!("{chunk_name}.hot-update")
        }
        Some((left, ext)) => {
            format!("{left}.hot-update.{ext}")
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
