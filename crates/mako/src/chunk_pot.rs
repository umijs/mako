mod ast_impl;
mod str_impl;
pub mod util;
use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use mako_core::anyhow::Result;
use mako_core::indexmap::IndexSet;
use mako_core::swc_css_ast::Stylesheet;
use mako_core::ternary;

use crate::chunk::{Chunk, ChunkType};
pub use crate::chunk_pot::util::CHUNK_FILE_NAME_HASH_LENGTH;
use crate::chunk_pot::util::{hash_hashmap, hash_vec};
use crate::compiler::Context;
use crate::config::Mode;
use crate::generate_chunks::ChunkFile;
use crate::module::{Module, ModuleAst, ModuleId};
use crate::module_graph::ModuleGraph;

pub struct ChunkPot<'a> {
    pub chunk_id: String,
    pub chunk_type: ChunkType,
    pub js_name: String,
    pub module_map: HashMap<String, (&'a Module, u64)>,
    pub js_hash: u64,
    pub stylesheet: Option<CssModules<'a>>,
}

impl<'cp> ChunkPot<'cp> {
    pub fn from<'a: 'cp>(
        chunk: &'a Chunk,
        mg: &'a ModuleGraph,
        context: &'cp Arc<Context>,
    ) -> Self {
        let (js_modules, stylesheet) = ChunkPot::split_modules(chunk.get_modules(), mg, context);

        ChunkPot {
            js_name: chunk.filename(),
            chunk_type: chunk.chunk_type.clone(),
            chunk_id: chunk.id.id.clone(),
            module_map: js_modules.module_map,
            js_hash: js_modules.raw_hash,
            stylesheet,
        }
    }

    pub fn to_normal_chunk_files(
        &self,
        chunk: &Chunk,
        context: &Arc<Context>,
    ) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!(&self.chunk_id);

        let mut files = vec![];

        if self.module_map.is_empty() {
            return Ok(files);
        }

        let js_chunk_file = ternary!(
            self.use_chunk_parallel(context),
            ternary!(
                context.args.watch,
                str_impl::render_normal_js_chunk,
                str_impl::render_normal_js_chunk_no_cache
            ),
            ternary!(
                context.args.watch,
                ast_impl::render_normal_js_chunk,
                ast_impl::render_normal_js_chunk_no_cache
            )
        )(self, context)?;

        files.push(js_chunk_file);

        if self.stylesheet.is_some() {
            let css_chunk_file = ternary!(
                context.args.watch,
                ast_impl::render_css_chunk,
                ast_impl::render_css_chunk_no_cache
            )(self, chunk, context)?;
            files.push(css_chunk_file);
        }

        Ok(files)
    }

    pub fn to_entry_chunk_files(
        &self,
        context: &Arc<Context>,
        js_map: &HashMap<String, String>,
        css_map: &HashMap<String, String>,
        chunk: &Chunk,
        hmr_hash: u64,
    ) -> Result<Vec<ChunkFile>> {
        mako_core::mako_profile_function!();

        let mut files = vec![];

        if self.stylesheet.is_some() {
            let css_chunk_file = ast_impl::render_css_chunk(self, chunk, context)?;

            let mut css_map = css_map.clone();
            css_map.insert(css_chunk_file.chunk_id.clone(), css_chunk_file.disk_name());

            files.push(css_chunk_file);
            files.push(if self.use_chunk_parallel(context) {
                str_impl::render_entry_js_chunk(self, js_map, &css_map, chunk, context, hmr_hash)?
            } else {
                ast_impl::render_entry_js_chunk(self, js_map, &css_map, chunk, context, hmr_hash)?
            });
        } else {
            mako_core::mako_profile_scope!("EntryDevJsChunk", &self.chunk_id);

            files.push(if self.use_chunk_parallel(context) {
                str_impl::render_entry_js_chunk(self, js_map, css_map, chunk, context, hmr_hash)?
            } else {
                ast_impl::render_entry_js_chunk(self, js_map, css_map, chunk, context, hmr_hash)?
            });
        }

        Ok(files)
    }

    fn use_chunk_parallel(&self, context: &Arc<Context>) -> bool {
        // parallel emit chunk when in watch mode
        context.config.chunk_parallel
            && context.args.watch
            && matches!(context.config.mode, Mode::Development)
    }

    fn split_modules<'a>(
        module_ids: &'a IndexSet<ModuleId>,
        module_graph: &'a ModuleGraph,
        context: &'a Arc<Context>,
    ) -> (JsModules<'a>, Option<CssModules<'a>>) {
        mako_core::mako_profile_function!(module_ids.len().to_string());
        let mut module_map: HashMap<String, (&Module, u64)> = Default::default();
        let mut merged_css_modules: Vec<(String, &Stylesheet)> = vec![];

        let mut module_raw_hash_map: HashMap<String, u64> = Default::default();
        let mut css_raw_hashes = vec![];

        let module_ids: Vec<_> = module_ids.iter().collect();

        for module_id in module_ids {
            let module = module_graph.get_module(module_id).unwrap();

            if module.info.is_none() {
                continue;
            }

            let module_info = module.info.as_ref().unwrap();
            let ast = &module_info.ast;

            if let ModuleAst::Script(_) = ast {
                module_raw_hash_map.insert(module.id.id.clone(), module_info.raw_hash);
                module_map.insert(module.id.generate(context), (module, module_info.raw_hash));
            }

            if let ModuleAst::Css(ast) = ast {
                // only apply the last css module if chunk depend on it multiple times
                // make sure the rules order is correct
                if let Some(index) = merged_css_modules
                    .iter()
                    .position(|(id, _)| id.eq(&module.id.id))
                {
                    merged_css_modules.remove(index);
                }
                merged_css_modules.push((module.id.id.clone(), &ast.ast));
                css_raw_hashes.push(module_info.raw_hash);
            }
        }

        let raw_hash = hash_hashmap(&module_raw_hash_map);

        if !merged_css_modules.is_empty() {
            mako_core::mako_profile_scope!("iter_chunk_css_modules");

            let mut stylesheets = vec![];

            for (_, ast) in merged_css_modules {
                stylesheets.push(ast);
            }

            let css_raw_hash = hash_vec(&css_raw_hashes);

            (
                JsModules {
                    module_map,
                    raw_hash,
                },
                Some(CssModules {
                    stylesheets,
                    raw_hash: css_raw_hash,
                }),
            )
        } else {
            (
                JsModules {
                    module_map,
                    raw_hash,
                },
                None,
            )
        }
    }
}

struct JsModules<'a> {
    pub module_map: HashMap<String, (&'a Module, u64)>,
    raw_hash: u64,
}

pub struct CssModules<'a> {
    stylesheets: Vec<&'a Stylesheet>,
    raw_hash: u64,
}

pub fn get_css_chunk_filename(js_chunk_filename: &str) -> String {
    format!(
        "{}.css",
        js_chunk_filename.strip_suffix(".js").unwrap_or("")
    )
}
