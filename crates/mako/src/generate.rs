use std::collections::HashSet;
use std::{fs, time::Instant};

use serde::Serialize;
use tracing::info;

use crate::update::UpdateResult;
use crate::{compiler::Compiler, config::DevtoolConfig};

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
        let _output_files = self.generate_chunks();
        let t_generate_chunks = t_generate_chunks.elapsed();

        // write chunks to files
        self.context
            .chunk_graph
            .read()
            .unwrap()
            .get_chunks()
            .iter()
            .for_each(|chunk| {
                let output = &config.output.path.join(chunk.filename());
                fs::write(output, &chunk.content.as_ref().unwrap().clone()).unwrap();
                if matches!(self.context.config.devtool, DevtoolConfig::SourceMap) {
                    fs::write(
                        format!("{}.map", output.display()),
                        &chunk.source_map.as_ref().unwrap().clone(),
                    )
                    .unwrap();
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

    // - 特殊处理 react，目前会同时包含 dev 和 prod 两个版本，虽然只会用到一个
    pub fn generate_with_update(&self, updated_modules: UpdateResult) {
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

        // generate chunks
        let t_generate_chunks = Instant::now();
        self.generate_chunks();
        let t_generate_chunks = t_generate_chunks.elapsed();

        let (current_chunks, modified_chunks) = {
            let cg = self.context.chunk_graph.read().unwrap();

            let chunk_names = cg.chunk_names();

            println!("xxx");
            dbg!(&chunk_names);

            let modified_chunks: Vec<String> = cg
                .get_chunks()
                .iter()
                .filter(|c| {
                    println!("ffff -> {}", c.filename());
                    updated_modules
                        .modified
                        .iter()
                        .any(|m_id| c.contains_modules(m_id))
                })
                .map(|c| c.filename())
                .collect();

            (chunk_names, modified_chunks)
        };

        let created_chunks: Vec<String> = current_chunks
            .difference(&last_chunk_names)
            .cloned()
            .collect();

        let removed_chunks: Vec<String> = last_chunk_names
            .difference(&current_chunks)
            .cloned()
            .collect();

        for chunk_name in &modified_chunks {
            let cg = self.context.chunk_graph.read().unwrap();

            if let Some(chunk) = cg.get_chunk_by_name(chunk_name) {
                let (code, _) = self.generate_hmr_chunk(chunk, &updated_modules.modified);

                self.write_to_dist("lazy.tsx-async.hot-update.js", code);
            }
        }

        self.write_to_dist(
            "hot-update.json",
            serde_json::to_string(&HotUpdateManifest {
                removed_chunks,
                created_chunks,
                modified_chunks,
            })
            .unwrap(),
        );

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

    fn write_to_dist<P: AsRef<std::path::Path>, C: AsRef<[u8]>>(&self, filename: P, content: C) {
        let to = self.context.config.output.path.join(filename);

        std::fs::write(to, content).unwrap();
    }
}

#[derive(Serialize)]
struct HotUpdateManifest {
    #[serde(rename(serialize = "c"))]
    created_chunks: Vec<String>,
    #[serde(rename(serialize = "r"))]
    removed_chunks: Vec<String>,
    #[serde(rename(serialize = "m"))]
    modified_chunks: Vec<String>,
}
