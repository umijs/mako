use std::collections::HashSet;
use std::fs;
use std::time::Instant;

use serde::Serialize;
use tracing::info;

use crate::compiler::Compiler;
use crate::config::DevtoolConfig;
use crate::update::UpdateResult;

impl Compiler {
    pub fn generate(&self) {
        info!("generate");
        let t_generate = Instant::now();
        let t_group_chunks = Instant::now();
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

        // generate chunks
        let t_generate_chunks = Instant::now();
        let output_files = self.generate_chunks();
        let t_generate_chunks = t_generate_chunks.elapsed();

        // write chunks to files
        output_files.iter().for_each(|file| {
            let output = &config.output.path.join(&file.path);
            fs::write(output, &file.content).unwrap();
            // generate separate sourcemap file
            if matches!(self.context.config.devtool, DevtoolConfig::SourceMap) {
                fs::write(format!("{}.map", output.display()), &file.sourcemap).unwrap();
            }
        });

        // write assets
        let assets_info = &(*self.context.assets_info.lock().unwrap());
        for (k, v) in assets_info {
            let asset_path = &self.context.root.join(k);
            let asset_output_path = &config.output.path.join(v);
            if asset_path.exists() {
                fs::copy(asset_path, asset_output_path).unwrap();
            } else {
                panic!("asset not found: {}", asset_path.display());
            }
        }

        // copy
        self.copy();

        info!("generate done in {}ms", t_generate.elapsed().as_millis());
        info!("  - group chunks: {}ms", t_group_chunks.as_millis());
        info!(
            "  - transform modules: {}ms",
            t_transform_modules.as_millis()
        );
        info!("  - generate chunks: {}ms", t_generate_chunks.as_millis());
    }

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
        self.copy();

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
