use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use std::vec;

use mako_core::anyhow::{anyhow, Result};
use mako_core::indexmap::IndexSet;
use mako_core::rayon::prelude::*;
use mako_core::swc_common::DUMMY_SP;
use mako_core::swc_css_ast::Stylesheet;
use mako_core::swc_ecma_ast::{Expr, KeyValueProp, Prop, PropName, PropOrSpread, Str};
use mako_core::tracing::debug;

use crate::chunk::{Chunk, ChunkType};
use crate::chunk_pot::ChunkPot;
use crate::compiler::{Compiler, Context};
use crate::module::{ModuleAst, ModuleId};
use crate::transform_in_generate::transform_css_generate;

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

impl Compiler {
    pub fn generate_chunk_files(&self, hmr_hash: u64) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();

        let module_graph = self.context.module_graph.read().unwrap();
        let chunk_graph = self.context.chunk_graph.read().unwrap();

        let chunks = chunk_graph.get_chunks();

        let t = Instant::now();
        let non_entry_chunk_files = self.generate_non_entry_chunk_files()?;
        debug!(
            "generate_non_entry_chunk_files {}ms",
            t.elapsed().as_millis()
        );

        let (js_chunk_map, css_chunk_map) = Self::chunk_maps(&non_entry_chunk_files);

        let mut all_chunk_files = {
            mako_core::mako_profile_scope!("collect_entry_chunks");
            chunks
                .iter()
                .filter(|chunk| {
                    matches!(
                        chunk.chunk_type,
                        ChunkType::Entry(_, _, _) | ChunkType::Worker(_)
                    )
                })
                .map(|&chunk| {
                    let mut pot = ChunkPot::from(chunk, &module_graph, &self.context)?;
                    let mut js_chunk_map = js_chunk_map.clone();
                    let mut css_chunk_map = css_chunk_map.clone();
                    let t = Instant::now();
                    let installable_chunks = chunk_graph
                        .installable_descendants_chunk(&chunk.id)
                        .into_iter()
                        .map(|c| c.id)
                        .collect::<Vec<_>>();

                    js_chunk_map.retain(|k, _| installable_chunks.contains(k));
                    css_chunk_map.retain(|k, _| installable_chunks.contains(k));

                    let chunk_file = self.generate_entry_chunk_files(
                        &mut pot,
                        &js_chunk_map,
                        &css_chunk_map,
                        chunk,
                        hmr_hash,
                    );
                    debug!("generate_entry_chunk_files {}ms", t.elapsed().as_millis());
                    chunk_file
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };

        all_chunk_files.extend(non_entry_chunk_files);

        Ok(all_chunk_files)
    }

    fn generate_entry_chunk_files(
        &self,
        pot: &mut ChunkPot,
        js_map: &HashMap<String, String>,
        css_map: &HashMap<String, String>,
        chunk: &Chunk,
        hmr_hash: u64,
    ) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();
        if matches!(chunk.chunk_type, ChunkType::Entry(_, _, true)) {
            // generate shared chunk as normal chunk
            pot.to_normal_chunk_files(chunk, &self.context)
        } else {
            pot.to_entry_chunk_files(&self.context, js_map, css_map, chunk, hmr_hash)
        }
    }

    fn generate_non_entry_chunk_files(&self) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();
        let chunk_graph = self.context.chunk_graph.read().unwrap();
        let chunks = chunk_graph.get_chunks();

        let chunk_file_results: Vec<_> = chunks
            .par_iter()
            .filter_map(|chunk| match chunk.chunk_type {
                ChunkType::Entry(_, _, _) | ChunkType::Worker(_) => None,
                _ => {
                    let context = self.context.clone();
                    let chunk_id = chunk.id.clone();
                    let chunk_graph = context.chunk_graph.read().unwrap();
                    let module_graph = context.module_graph.read().unwrap();
                    let chunk = chunk_graph.chunk(&chunk_id).unwrap();
                    let chunk_files = ChunkPot::from(chunk, &module_graph, &context)
                        .map_or_else(Err, |pot| pot.to_normal_chunk_files(chunk, &context));

                    Some(chunk_files)
                }
            })
            .collect();

        let mut chunk_files = vec![];
        let mut errors = vec![];

        chunk_file_results.into_iter().for_each(|cfs| match cfs {
            Ok(cfs) => {
                chunk_files.extend(cfs);
            }
            Err(e) => {
                errors.push(e.to_string());
            }
        });

        if !errors.is_empty() {
            return Err(anyhow!(errors.join(", ")));
        }
        Ok(chunk_files)
    }

    fn chunk_maps(
        non_entry_chunk_files: &[ChunkFile],
    ) -> (HashMap<String, String>, HashMap<String, String>) {
        let mut js_chunk_map: HashMap<String, String> = HashMap::new();
        let mut css_chunk_map: HashMap<String, String> = HashMap::new();

        for f in non_entry_chunk_files.iter() {
            match f.file_type {
                ChunkFileType::JS => {
                    js_chunk_map.insert(f.chunk_id.clone(), f.disk_name());
                }
                ChunkFileType::Css => {
                    css_chunk_map.insert(f.chunk_id.clone(), f.disk_name());
                }
            }
        }

        (js_chunk_map, css_chunk_map)
    }
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
