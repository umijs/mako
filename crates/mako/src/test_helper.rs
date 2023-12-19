use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use mako_core::swc_common::sync::Lrc;
use mako_core::swc_common::SourceMap;
use mako_core::swc_ecma_ast::Module as SwcModule;
use mako_core::swc_ecma_codegen::text_writer::JsWriter;
use mako_core::swc_ecma_codegen::Emitter;
use mako_core::swc_ecma_visit::{VisitMut, VisitMutWith};
use mako_core::tracing_subscriber::{fmt, EnvFilter};

use crate::ast::build_js_ast;
use crate::compiler::{self, Compiler};
use crate::config::{Config, Mode};
use crate::module::{Module, ModuleId, ModuleInfo};

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

#[allow(dead_code)]
pub fn create_mock_module(path: PathBuf, code: &str) -> Module {
    setup_logger();

    let ast = build_js_ast(path.to_str().unwrap(), code, &Arc::new(Default::default())).unwrap();
    let module_id = ModuleId::from_path(path.clone());
    let info = ModuleInfo {
        ast: crate::module::ModuleAst::Script(ast),
        path: path.to_string_lossy().to_string(),
        external: None,
        raw: code.to_string(),
        raw_hash: 0,
        resolved_resource: None,
        missing_deps: HashMap::new(),
        ignored_deps: vec![],
        top_level_await: false,
        is_async: false,
        source_map_chain: vec![],
    };
    Module::new(module_id, false, Some(info))
}

pub fn get_module(compiler: &Compiler, path: &str) -> Module {
    let module_graph = compiler.context.module_graph.read().unwrap();
    let cwd_path = &compiler.context.root;
    let module_id = ModuleId::from(cwd_path.join(path));
    let module = module_graph.get_module(&module_id).unwrap();
    module.clone()
}

#[allow(dead_code)]
pub fn setup_compiler(base: &str, cleanup: bool) -> Compiler {
    setup_logger();
    let current_dir = std::env::current_dir().unwrap();
    let root = current_dir.join(base);
    if !root.parent().unwrap().exists() {
        fs::create_dir_all(root.parent().unwrap()).unwrap();
    }
    if cleanup {
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir_all(&root).unwrap();
    }
    let mut config = Config::new(&root, None, None).unwrap();
    config.hmr = false;
    config.minify = false;
    config.mode = Mode::Production;

    compiler::Compiler::new(config, root, Default::default(), None).unwrap()
}

pub fn setup_logger() {
    let _result = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        // .with_max_level(Level::DEBUG)
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        // .without_time()
        .try_init();
}

pub fn transform_ast_with(
    module: &mut SwcModule,
    visitor: &mut dyn VisitMut,
    cm: &Lrc<SourceMap>,
) -> String {
    module.visit_mut_with(visitor);
    emit_js(module, cm)
}

fn emit_js(module: &SwcModule, cm: &Arc<SourceMap>) -> String {
    let mut buf = Vec::new();

    {
        let writer = Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None));
        let mut emitter = Emitter {
            cfg: Default::default(),
            comments: None,
            cm: cm.clone(),
            wr: writer,
        };
        // This may return an error if it fails to write
        emitter.emit_module(module).unwrap();
    }

    String::from_utf8(buf).unwrap().trim().to_string()
}
