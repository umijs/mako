use std::{fs, time::Instant};

use tracing::info;

use crate::compiler::Compiler;

impl Compiler {
    // TODO:
    // - 特殊处理 react，目前会同时包含 dev 和 prod 两个版本，虽然只会用到一个
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
            if self.context.config.sourcemap {
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
}
