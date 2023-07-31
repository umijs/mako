use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use swc_common::sync::Lrc;
use swc_common::SourceMap;
use swc_ecma_ast::Module as SwcModule;
use swc_ecma_codegen::text_writer::JsWriter;
use swc_ecma_codegen::Emitter;
use swc_ecma_visit::{VisitMut, VisitMutWith};
use tracing_subscriber::EnvFilter;

use crate::ast::{build_js_ast, js_ast_to_code};
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
    let ast = build_js_ast(path.to_str().unwrap(), code, &Arc::new(Default::default())).unwrap();
    let module_id = ModuleId::from_path(path.clone());
    let info = ModuleInfo {
        ast: crate::module::ModuleAst::Script(ast),
        path: path.to_string_lossy().to_string(),
        external: None,
        raw_hash: 0,
        missing_deps: HashMap::new(),
    };
    Module::new(module_id, false, Some(info))
}

#[allow(dead_code)]
pub fn setup_compiler(base: &str, cleanup: bool) -> Compiler {
    let _result = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("mako=debug")),
        )
        // .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        // .without_time()
        .try_init();
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

    compiler::Compiler::new(config, root)
}

pub fn read_dist_file(compiler: &Compiler) -> String {
    let cwd_path = &compiler.context.root;

    fs::read_to_string(cwd_path.join("dist/index.js")).unwrap()
}

pub fn setup_files(compiler: &Compiler, extra_files: Vec<(String, String)>) {
    let cwd_path = &compiler.context.root;
    extra_files.into_iter().for_each(|(path, content)| {
        let output = cwd_path.join(path);
        fs::write(output, content).unwrap();
    });
}

pub fn module_to_jscode(compiler: &Compiler, module_id: &ModuleId) -> String {
    let module_graph = compiler.context.module_graph.read().unwrap();
    let module = module_graph.get_module(module_id).unwrap();
    let context = compiler.context.clone();
    let info = module.info.as_ref().unwrap();
    let ast = &info.ast;
    match ast {
        crate::module::ModuleAst::Script(ast) => {
            let (code, _) =
                js_ast_to_code(&ast.ast.clone(), &context, module.id.id.as_str()).unwrap();
            code
        }
        crate::module::ModuleAst::Css(_) => todo!(),
        crate::module::ModuleAst::None => todo!(),
    }
}

pub fn transform_ast_with(module: &mut SwcModule, visitor: &mut Box<dyn VisitMut>) -> String {
    module.visit_mut_with(visitor);
    emit_js(module)
}

fn emit_js(module: &SwcModule) -> String {
    let cm: Lrc<SourceMap> = Default::default();
    let mut buf = Vec::new();

    {
        let writer = Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None));
        let mut emitter = Emitter {
            cfg: Default::default(),
            comments: None,
            cm,
            wr: writer,
        };
        // This may return an error if it fails to write
        emitter.emit_module(module).unwrap();
    }

    String::from_utf8(buf).unwrap().trim().to_string()
}
