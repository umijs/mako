use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::ast::build_js_ast;
use crate::chunk_graph::ChunkGraph;
use crate::compiler::{Context, Meta};
use crate::module::{Module, ModuleId, ModuleInfo};
use crate::module_graph::ModuleGraph;

#[macro_export]
macro_rules! assert_display_snapshot {
    ($value:expr) => {{
        let cwd = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let value = format!("{}", $value).replace(&cwd, "<CWD>");
        insta::assert_snapshot!(insta::_macro_support::AutoName, value, stringify!($value));
    }};
}

#[macro_export]
macro_rules! assert_debug_snapshot {
    ($value:expr) => {{
        let cwd = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let value = format!("{:#?}", $value).replace(&cwd, "<CWD>");
        insta::assert_snapshot!(insta::_macro_support::AutoName, value, stringify!($value));
    }};
}

pub fn create_mock_module(path: PathBuf, code: &str) -> Module {
    let root = PathBuf::from("/path/to/root");
    let ast = build_js_ast(
        path.to_str().unwrap(),
        code,
        &Arc::new(Context {
            config: Default::default(),
            root,
            module_graph: RwLock::new(ModuleGraph::new()),
            chunk_graph: RwLock::new(ChunkGraph::new()),
            assets_info: Mutex::new(HashMap::new()),
            meta: Meta::new(),
        }),
    )
    .unwrap();
    let module_id = ModuleId::from_path(path.clone());
    let info = ModuleInfo {
        ast: crate::module::ModuleAst::Script(ast),
        path: path.to_string_lossy().to_string(),
        external: None,
    };
    Module::new(module_id, false, Some(info))
}
