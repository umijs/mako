#![feature(box_patterns)]

use compiler::Compiler;

use crate::plugins::node_polyfill::get_node_builtins;

pub mod build;
pub mod chunk;
pub mod chunk_graph;
pub mod compiler;
pub mod config;
pub mod context;
pub mod generate;
pub mod module;
pub mod module_graph;
pub mod plugin;
pub mod plugins;
pub mod utils;

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() <= 1 {
        panic!("Please specify the root directory of the project");
    }

    // config
    let root = std::env::current_dir()
        .unwrap()
        .join(&args[1])
        .to_string_lossy()
        .to_string();
    let mut config = config::Config::from_literal_str(
        format!(
            r#"
{{
    "externals": {{}},
    "root": "{}",
    "entry": {{ "index": "index.tsx" }}
}}
    "#,
            root
        )
        .as_str(),
    )
    .unwrap();
    config.normalize();

    // TODO: move this to plugin
    // node polyfills
    let builtins = get_node_builtins();
    for name in builtins.iter() {
        config.externals.insert(name.to_string(), name.to_string());
    }

    // compiler_origin::run_compiler(config);
    let mut compiler = Compiler::new(config);
    compiler.run();

    println!("✅ DONE");
}
