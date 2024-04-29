use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use mako_core::anyhow::{anyhow, Result};
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_ecma_ast::{Expr, KeyValueProp, Prop, PropName, PropOrSpread, Str};
use nanoid::nanoid;

use crate::compiler::{Compiler, Context};
use crate::generate::chunk::{Chunk, ChunkType};
use crate::generate::chunk_pot::{get_css_chunk_filename, ChunkPot, CHUNK_FILE_NAME_HASH_LENGTH};
use crate::generate::transform::transform_css_generate;
use crate::module::{ModuleAst, ModuleId};
use crate::utils::thread_pool;

#[derive(Clone)]
pub enum ChunkFileType {
    JS,
    Css,
}

#[derive(Clone)]
pub struct ChunkFile {
    pub raw_hash: u64,
    pub content: Vec<u8>,
    pub source_map: Option<Vec<u8>>,
    pub hash: Option<String>,
    pub file_name: String,
    pub chunk_id: String,
    pub file_type: ChunkFileType,
}

impl ChunkFile {
    pub fn disk_name(&self) -> String {
        if let Some(hash) = &self.hash {
            hash_file_name(&self.file_name, hash)
        } else {
            self.file_name.clone()
        }
    }

    pub fn source_map_disk_name(&self) -> String {
        format!("{}.map", self.disk_name())
    }

    pub fn source_map_name(&self) -> String {
        format!("{}.map", self.file_name)
    }
}

type ChunksHashPlaceholder = HashMap<String, String>;
type ChunksHashReplacer = HashMap<String, String>;

impl Compiler {
    pub fn generate_chunk_files(&self, hmr_hash: u64) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();

        let (entry_chunks, normal_chunks): (Vec<&Chunk>, Vec<&Chunk>) = chunks
            .into_iter()
            .partition(|chunk| match chunk.chunk_type {
                ChunkType::Entry(_, _, false) | ChunkType::Worker(_) => true,
                ChunkType::Entry(_, _, true) => false,
                _ => false,
            });

        let (entry_chunk_files_with_placeholder, normal_chunk_files) = thread_pool::join(
            || self.generate_entry_chunk_files(entry_chunks, hmr_hash),
            || self.generate_normal_chunk_files(normal_chunks),
        );

        let normal_chunk_files = normal_chunk_files?;

        let mut entry_chunk_files_with_placeholder = entry_chunk_files_with_placeholder?;

        if self.context.config.hash {
            let (js_chunks_hash_replacer, css_chunks_hash_replacer) =
                normal_chunk_files.iter().fold(
                    (ChunksHashReplacer::new(), ChunksHashReplacer::new()),
                    |(mut acc_js, mut acc_css), chunk_file| {
                        match chunk_file.file_type {
                            ChunkFileType::JS => {
                                acc_js.insert(chunk_file.chunk_id.clone(), chunk_file.disk_name());
                            }
                            ChunkFileType::Css => {
                                acc_css.insert(chunk_file.chunk_id.clone(), chunk_file.disk_name());
                            }
                        };
                        (acc_js, acc_css)
                    },
                );

            entry_chunk_files_with_placeholder
                .par_iter_mut()
                .try_for_each(
                    |(chunk_files, js_chunks_hash_placeholder, css_chunks_hash_placeholder)| -> Result<()>{
                        replace_chunks_placeholder(
                            chunk_files,
                            js_chunks_hash_placeholder,
                            &js_chunks_hash_replacer,
                        )?;
                        replace_chunks_placeholder(
                            chunk_files,
                            css_chunks_hash_placeholder,
                            &css_chunks_hash_replacer,
                        )?;
                        Ok(())
                    },
                )?;
        }

        let entry_chunk_files = entry_chunk_files_with_placeholder
            .into_iter()
            .flat_map(|e| e.0)
            .collect();

        Ok([entry_chunk_files, normal_chunk_files].concat())
    }

    fn generate_entry_chunk_files(
        &self,
        chunks: Vec<&Chunk>,
        hmr_hash: u64,
    ) -> Result<Vec<(Vec<ChunkFile>, ChunksHashPlaceholder, ChunksHashPlaceholder)>> {
        let chunk_file_results: Vec<_> = chunks
            .par_iter()
            .map(|chunk| {
                let context = self.context.clone();
                let module_graph = context.module_graph.read().unwrap();
                let chunk_graph = self.context.chunk_graph.read().unwrap();

                let (js_chunks_hash_placeholder, css_chunks_hash_placeholder) = chunk_graph
                    .installable_descendants_chunk(&chunk.id)
                    .iter()
                    .fold(
                        (ChunksHashPlaceholder::new(), ChunksHashPlaceholder::new()),
                        |(mut acc_js, mut acc_css), descendant_chunk_id| {
                            let descendant_chunk = chunk_graph.chunk(descendant_chunk_id).unwrap();
                            // TODO: maybe we can split chunks to chunk pots before generate, because normal chunks will be
                            // split here and fn generate_normal_chunk_files twice
                            let chunk_pot =
                                ChunkPot::from(descendant_chunk, &module_graph, &context);

                            if self.context.config.hash {
                                let placeholder = nanoid!(CHUNK_FILE_NAME_HASH_LENGTH);

                                let js_filename = chunk_pot.js_name;

                                if chunk_pot.stylesheet.is_some() {
                                    let css_filename = get_css_chunk_filename(&js_filename);
                                    acc_css.insert(
                                        descendant_chunk_id.id.clone(),
                                        hash_file_name(&css_filename, &placeholder),
                                    );
                                }

                                acc_js.insert(
                                    descendant_chunk_id.id.clone(),
                                    hash_file_name(&js_filename, &placeholder),
                                );
                            } else {
                                let js_filename = chunk_pot.js_name;

                                if chunk_pot.stylesheet.is_some() {
                                    let css_filename = get_css_chunk_filename(&js_filename);
                                    acc_css.insert(descendant_chunk_id.id.clone(), css_filename);
                                }

                                acc_js.insert(descendant_chunk_id.id.clone(), js_filename);
                            }
                            (acc_js, acc_css)
                        },
                    );

                let chunk_files = {
                    let chunk_pot = ChunkPot::from(chunk, &module_graph, &context);
                    chunk_pot
                        .to_entry_chunk_files(
                            &context,
                            &js_chunks_hash_placeholder,
                            &css_chunks_hash_placeholder,
                            chunk,
                            hmr_hash,
                        )
                        .map(|chunk_files| {
                            (
                                chunk_files,
                                js_chunks_hash_placeholder,
                                css_chunks_hash_placeholder,
                            )
                        })
                };

                chunk_files
            })
            .collect();

        let (chunk_files, errors) = chunk_file_results.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut chunk_files, mut errors), result| {
                match result {
                    Ok(cf) => chunk_files.push(cf),
                    Err(e) => errors.push(e),
                }
                (chunk_files, errors)
            },
        );

        if !errors.is_empty() {
            return Err(anyhow!(errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(", ")));
        }

        Ok(chunk_files)
    }

    fn generate_normal_chunk_files(&self, chunks: Vec<&Chunk>) -> Result<Vec<ChunkFile>> {
        let chunk_file_results: Vec<_> = chunks
            .par_iter()
            .map(|chunk| {
                let context = self.context.clone();
                let chunk_id = chunk.id.clone();
                let chunk_graph = context.chunk_graph.read().unwrap();
                let module_graph = context.module_graph.read().unwrap();
                let chunk = chunk_graph.chunk(&chunk_id).unwrap();

                let chunk_files = ChunkPot::from(chunk, &module_graph, &context)
                    .to_normal_chunk_files(chunk, &context);

                chunk_files
            })
            .collect();

        let (chunk_files, errors) = chunk_file_results.into_iter().fold(
            (Vec::new(), Vec::new()),
            |(mut chunk_files, mut err_msgs), result| {
                match result {
                    Ok(cfs) => chunk_files.extend(cfs),
                    Err(e) => err_msgs.push(e),
                }
                (chunk_files, err_msgs)
            },
        );

        if !errors.is_empty() {
            return Err(anyhow!(errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(", ")));
        }

        Ok(chunk_files)
    }
}

fn replace_chunks_placeholder(
    chunk_files: &mut [ChunkFile],
    chunks_hash_placeholder: &ChunksHashPlaceholder,
    chunks_hash_replacer: &ChunksHashReplacer,
) -> Result<()> {
    chunks_hash_placeholder.iter().try_for_each(
        |(chunk_id, placeholder)| match chunks_hash_replacer.get(chunk_id) {
            Some(replacer) => {
                chunk_files
                    .iter_mut()
                    .filter(|cf| matches!(cf.file_type, ChunkFileType::JS))
                    .try_for_each(|cf| {
                        let position = cf
                            .content
                            .windows(placeholder.len())
                            .position(|w| w == placeholder.as_bytes());

                        position.map_or(
                            {
                                Err(anyhow!(
                                    "Generate \"{}\" failed, placeholder \"{}\" not existed in chunk file.",
                                    chunk_id,
                                    placeholder,
                                ))
                            },
                            |pos| {
                                cf.content.splice(
                                    pos..pos + replacer.len(),
                                    replacer.as_bytes().to_vec(),
                                );
                                Ok(())
                            },
                        )
                    })?;
                Ok(())
            }
            _ => Err(anyhow!(
                "Generate \"{}\" failed, replacer not found for placeholder \"{}\".",
                chunk_id,
                placeholder
            )),
        },
    )
}

pub fn build_props(key_str: &str, value: Box<Expr>) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: PropName::Str(Str {
            span: DUMMY_SP,
            value: key_str.into(),
            raw: None,
        }),
        value,
    })))
}

pub fn modules_to_js_stmts(
    module_ids: &IndexSet<ModuleId>,
    module_graph: &std::sync::RwLockReadGuard<crate::module_graph::ModuleGraph>,
    context: &Arc<Context>,
) -> Result<(Vec<PropOrSpread>, Option<Stylesheet>)> {
    let mut js_stmts = vec![];
    let mut merged_css_modules: Vec<(String, Stylesheet)> = vec![];

    let module_ids: Vec<_> = module_ids.iter().collect();

    for module_id in module_ids {
        let module = module_graph.get_module(module_id).unwrap();
        let ast = module.info.as_ref().unwrap();
        let ast = &ast.ast;

        let fn_expr = module.to_module_fn_expr()?;

        js_stmts.push(build_props(
            module.id.generate(context).as_str(),
            fn_expr.into(),
        ));

        if let ModuleAst::Css(ast) = ast {
            // only apply the last css module if chunk depend on it multiple times
            // make sure the rules order is correct
            if let Some(index) = merged_css_modules
                .iter()
                .position(|(id, _)| id.eq(&module.id.id))
            {
                merged_css_modules.remove(index);
            }
            merged_css_modules.push((module.id.id.clone(), ast.ast.clone()));
        }
    }
    if !merged_css_modules.is_empty() {
        let mut merged_css_ast = Stylesheet {
            span: DUMMY_SP,
            rules: vec![],
        };

        for (_, ast) in merged_css_modules {
            merged_css_ast.rules.extend(ast.rules);
        }

        transform_css_generate(&mut merged_css_ast, context);
        Ok((js_stmts, Some(merged_css_ast)))
    } else {
        Ok((js_stmts, None))
    }
}

fn hash_file_name(file_name: &String, hash: &String) -> String {
    let path = Path::new(&file_name);
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let file_extension = path.extension().unwrap().to_str().unwrap();

    format!("{}.{}.{}", file_stem, hash, file_extension)
}
