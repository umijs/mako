use std::fs;

use mako_core::tracing_subscriber::{fmt, EnvFilter};

use crate::compiler::{self, Compiler};
use crate::config::{Config, Mode};
use crate::module::{Module, ModuleId};

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
    config.hmr = None;
    config.minify = false;
    config.mode = Mode::Production;
    config.optimization = None;

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
