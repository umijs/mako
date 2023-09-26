use std::collections::HashSet;
use std::fs;
use std::ops::DerefMut;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Result};
use cached::proc_macro::cached;
use indexmap::IndexSet;
use rayon::prelude::*;
use serde::Serialize;
use swc_ecma_ast::{
    BindingIdent, CallExpr, Callee, Decl, Expr, ExprOrSpread, ModuleItem, ObjectLit, Pat, Stmt,
    VarDecl,
};
use tracing::debug;

use crate::ast::{build_js_ast, css_ast_to_code, js_ast_to_code};
use crate::chunk::ChunkType;
use crate::compiler::{Compiler, Context};
use crate::config::{DevtoolConfig, Mode, OutputMode, TreeShakeStrategy};
use crate::generate_chunks::{compile_runtime_entry, modules_to_js_stmts, OutputAst};
use crate::load::file_content_hash;
use crate::minify::{minify_css, minify_js};
use crate::module::{ModuleAst, ModuleId};
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
                    puffin::profile_scope!("tree shake");
                    self.context
                        .plugin_driver
                        .optimize_module_graph(module_graph.deref_mut())?;
                    let t_tree_shaking = t_tree_shaking.elapsed();
                    println!("basic optimize in {}ms.", t_tree_shaking.as_millis());
                }
                TreeShakeStrategy::Advanced => {
                    puffin::profile_scope!("advanced tree shake");
                    let shaking_module_ids = self.tree_shaking();
                    let t_tree_shaking = t_tree_shaking.elapsed();
                    println!(
                        "{} modules removed in {}ms.",
                        shaking_module_ids.len(),
                        t_tree_shaking.as_millis()
                    );
                }
                TreeShakeStrategy::None => {
                    // do nothing
                }
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

        // generate chunks
        let t_generate_chunks = Instant::now();
        debug!("generate chunks");
        let mut chunk_asts = self.generate_chunks_ast()?;
        let t_generate_chunks = t_generate_chunks.elapsed();

        // minify
        let t_minify = Instant::now();
        debug!("minify");
        if self.context.config.minify && matches!(self.context.config.mode, Mode::Production) {
            chunk_asts
                .par_iter_mut()
                .try_for_each(|file| -> Result<()> {
                    match &mut file.ast {
                        ModuleAst::Script(ast) => {
                            minify_js(ast, &self.context)?;
                        }
                        ModuleAst::Css(ast) => {
                            minify_css(ast, &self.context)?;
                        }
                        _ => (),
                    }
                    Ok(())
                })?;
        }
        let t_minify = t_minify.elapsed();

        // ast to code and sourcemap, then write
        let t_ast_to_code_and_write = Instant::now();
        debug!("ast to code and write");
        {
            puffin::profile_scope!("ast to code");
            let mut files = chunk_asts
                .par_iter()
                .map(|file| -> Result<_> { get_chunk_emit_files(file, &self.context) })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();

            if self.context.config.hash {
                let cg = self.context.chunk_graph.read().unwrap();
                let module_graph = self.context.module_graph.read().unwrap();

                let mut entries = cg.get_chunks();
                entries.retain(|&ch| matches!(ch.chunk_type, ChunkType::Entry(_, _)));

                let mut css_chunks: Vec<String> = vec![];
                let mut js_chunks: Vec<String> = vec!["chunksIdToUrlMap = {};".to_string()];

                for f in &files {
                    if f.filename.ends_with(".js") {
                        js_chunks.push(format!(
                            "chunksIdToUrlMap[\"{}\"] = `{}`;",
                            f.chunk_id, f.hashname
                        ))
                    }

                    if f.filename.ends_with(".css") {
                        let str = format!(
                            "cssChunksIdToUrlMap[\"{}\"] = `{}`;",
                            f.chunk_id, f.hashname
                        );
                        css_chunks.push(str);
                    }
                }

                dbg!(&css_chunks);
                dbg!(&js_chunks);

                dbg!(entries.len());

                let full_hash = self.full_hash();

                for chunk in entries {
                    if let ChunkType::Entry(module_id, _) = &chunk.chunk_type {
                        let module_ids = chunk.get_modules();

                        let stmts_res =
                            modules_to_js_stmts(module_ids, &module_graph, &self.context);

                        if stmts_res.is_err() {
                            return Err(anyhow!(
                                "Chunk {} failed to generate js ast {:?}",
                                chunk.id.id,
                                stmts_res.err().unwrap()
                            ));
                        }

                        let (js_stmts, _merged_css_ast) = stmts_res.unwrap();

                        let module_generated_id = module_id.generate(&self.context);

                        let chunks_ids = cg
                            .sync_dependencies_chunk(chunk)
                            .into_iter()
                            .map(|chunk| chunk.generate(&self.context))
                            .collect::<Vec<String>>();

                        let chunks_map_str = js_chunks.join("\n");
                        let css_chunks_map_str = format!(
                            "{}\n{}\n",
                            format!(
                                "installedChunks['{}'] = 0;\n",
                                chunk.id.generate(&self.context),
                            ),
                            css_chunks.join("\n")
                        );

                        let code = format!(
                            "{}\n{}",
                            chunks_map_str,
                            compile_runtime_entry(
                                self.context
                                    .assets_info
                                    .lock()
                                    .unwrap()
                                    .values()
                                    .any(|info| info.ends_with(".wasm")),
                                self.context
                                    .module_graph
                                    .read()
                                    .unwrap()
                                    .modules()
                                    .iter()
                                    .any(|module| module.info.as_ref().unwrap().is_async),
                            )
                        )
                        .replace("_%full_hash%_", &full_hash.to_string())
                        .replace(
                            "// __inject_runtime_code__",
                            &self
                                .context
                                .plugin_driver
                                .runtime_plugins_code(&self.context)?,
                        )
                        .replace("// __CSS_CHUNKS_URL_MAP", &css_chunks_map_str.to_string())
                        .replace("_%main%_", &module_generated_id);

                        let content = if !chunks_ids.is_empty() {
                            let ensures = chunks_ids
                                .into_iter()
                                .map(|id| format!("requireModule.ensure(\"{}\")", id))
                                .collect::<Vec<String>>()
                                .join(", ");

                            code.replace(
                                "// __BEFORE_ENTRY",
                                format!("Promise.all([{}]).then(()=>{{", ensures).as_str(),
                            )
                            .replace("// __AFTER_ENTRY", "});")
                        } else {
                            code
                        };

                        let mut js_ast = build_js_ast(
                            "mako_internal_runtime_entry.js",
                            content.as_str(),
                            &self.context,
                        )
                        .unwrap();

                        for stmt in &mut js_ast.ast.body {
                            // const runtime = createRuntime({ }, 'main');
                            if let ModuleItem::Stmt(Stmt::Decl(Decl::Var(box VarDecl {
                                decls,
                                ..
                            }))) = stmt
                            {
                                if decls.len() != 1 {
                                    continue;
                                }
                                let decl = &mut decls[0];
                                if let Pat::Ident(BindingIdent { id, .. }) = &decl.name {
                                    if id.sym.to_string() != "runtime" {
                                        continue;
                                    }
                                }
                                if let Some(box Expr::Call(CallExpr {
                                    args,
                                    callee: Callee::Expr(box Expr::Ident(ident)),
                                    ..
                                })) = &mut decl.init
                                {
                                    if args.len() != 2 || ident.sym.to_string() != "createRuntime" {
                                        continue;
                                    }
                                    if let ExprOrSpread {
                                        expr: box Expr::Object(ObjectLit { props, .. }),
                                        ..
                                    } = &mut args[0]
                                    {
                                        props.extend(js_stmts);
                                        break;
                                    }
                                }
                            }
                        }
                        let filename = chunk.filename();
                        let map_filename = format!("{}.map", filename);

                        let (js_code, js_sourcemap) =
                            js_ast_to_code(&js_ast.ast, &self.context, &filename)?;

                        let generated_chunk_id = chunk.id.generate(&self.context);

                        for f in &mut files {
                            if f.chunk_id == generated_chunk_id && f.filename.ends_with(".js") {
                                f.content = js_code.clone();
                            }

                            if matches!(self.context.config.devtool, DevtoolConfig::SourceMap)
                                && f.filename == map_filename
                            {
                                f.content = js_sourcemap.clone()
                            }
                        }
                    }
                }
            }

            for f in files {
                self.write_to_dist_with_stats(f);
            }
        }

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
        debug!("  - minify: {}ms", t_minify.as_millis());
        debug!(
            "  - ast to code and write: {}ms",
            t_ast_to_code_and_write.as_millis()
        );
        debug!("  - write assets: {}ms", t_write_assets.as_millis());

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
    // 写入产物前记录 content 大小, 并加上 hash 值
    pub fn write_to_dist_with_stats(&self, file: EmitFile) {
        let to: PathBuf = self.context.config.output.path.join(file.hashname.clone());
        let size = file.content.len() as u64;
        self.context.stats_info.lock().unwrap().add_assets(
            size,
            file.filename,
            file.chunk_id,
            to.clone(),
            file.hashname,
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
    convert = r#"{ format!("{}-{}", file.ast_module_hash, file.path) }"#
)]

// 需要在这里提前记录 js 和 map 的 hash，因为 map 是不单独计算 hash 值的，继承的是 js 的 hash 值
fn get_chunk_emit_files(file: &OutputAst, context: &Arc<Context>) -> Result<Vec<EmitFile>> {
    let mut files = vec![];
    match &file.ast {
        ModuleAst::Script(ast) => {
            // ast to code
            let (js_code, js_sourcemap) = js_ast_to_code(&ast.ast, context, &file.path)?;

            // 计算 hash 值
            let hashname = if context.config.hash {
                let h = file_content_hash(&js_code);
                hash_file_name(file.path.clone(), h)
            } else {
                file.path.clone()
            };
            // generate code and sourcemap files
            files.push(EmitFile {
                filename: file.path.clone(),
                content: js_code,
                chunk_id: file.chunk_id.clone(),
                hashname: hashname.clone(),
            });
            if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
                files.push(EmitFile {
                    filename: format!("{}.map", file.path.clone()),
                    hashname: format!("{}.map", hashname),
                    content: js_sourcemap,
                    chunk_id: "".to_string(),
                });
            }
        }
        ModuleAst::Css(ast) => {
            // ast to code
            let (css_code, css_sourcemap) = css_ast_to_code(ast, context, &file.path);
            // 计算 hash 值
            let hashed_name = if context.config.hash {
                let h = file_content_hash(&css_code);

                hash_file_name(file.path.clone(), h)
            } else {
                file.path.clone()
            };
            files.push(EmitFile {
                filename: file.path.clone(),
                hashname: hashed_name.clone(),
                content: css_code,
                chunk_id: file.chunk_id.clone(),
            });
            if matches!(context.config.devtool, DevtoolConfig::SourceMap) {
                files.push(EmitFile {
                    filename: format!("{}.map", file.path.clone()),
                    hashname: format!("{}.map", hashed_name),
                    content: css_sourcemap,
                    chunk_id: "".to_string(),
                });
            }
        }
        _ => (),
    }

    Ok(files)
}

#[allow(dead_code)]
fn hash_file_name(file_name: String, hash: String) -> String {
    let path = Path::new(&file_name);
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let file_extension = path.extension().unwrap().to_str().unwrap();

    format!("{}.{}.{}", file_stem, hash, file_extension)
}

fn postfix_hash(file_name: &String, hash: u64) -> String {
    let path = Path::new(file_name);
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let file_extension = path.extension().unwrap().to_str().unwrap();
    let hash = &format!("{:08x}", hash)[0..8];

    format!("{}.{}.{}", file_stem, hash, file_extension)
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
