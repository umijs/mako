#![feature(box_patterns)]

use compiler::Compiler;

pub mod build;
pub mod chunk;
pub mod chunk_graph;
pub mod compiler;
pub mod config;
pub mod context;
pub mod generate;
pub mod module;
pub mod module_graph;
pub mod update;
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
    "externals": {{
    }},
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

    // compiler_origin::run_compiler(config);
    let mut compiler = Compiler::new(config);
    compiler.run();

    println!("✅ DONE");
}
